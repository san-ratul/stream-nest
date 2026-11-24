//! Safe remote `.torrent` fetching: SSRF-protected, size- and redirect-bounded.
//! The fetcher never touches arbitrary hosts' private ranges, never disables TLS
//! verification, and validates the downloaded payload as bencode metainfo.

use std::net::{IpAddr, Ipv4Addr};
#[cfg(test)]
use std::net::Ipv6Addr;
use std::time::Duration;

use serde::Serialize;

const USER_AGENT: &str = "StreamNest/0.2 (+https://github.com/san-ratul/stream-nest)";

#[derive(Debug, Clone)]
pub struct FetchLimits {
    pub max_bytes: u64,
    pub max_redirects: usize,
    pub timeout: Duration,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024, // 10 MiB: torrents are tiny
            max_redirects: 5,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub enum UrlError {
    UnsupportedScheme,
    InvalidUrl(String),
    PrivateAddress(String),
    TooManyRedirects,
    TooLarge(u64),
    /// Reserved for transport-level timeout mapping by callers; reqwest surfaces
    /// timeouts as Network errors after the configured duration elapses.
    #[allow(dead_code)]
    Timeout,
    InvalidTorrent(String),
    Network(String),
}

impl std::fmt::Display for UrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrlError::UnsupportedScheme => {
                write!(f, "Only http:// and https:// torrent URLs are supported")
            }
            UrlError::InvalidUrl(m) => write!(f, "Invalid torrent URL: {m}"),
            UrlError::PrivateAddress(h) => {
                write!(f, "Refusing to fetch from a private or local address ({h})")
            }
            UrlError::TooManyRedirects => write!(f, "Too many redirects"),
            UrlError::TooLarge(n) => {
                write!(f, "Torrent response too large ({n} bytes; limit is configurable in Settings)")
            }
            UrlError::Timeout => write!(f, "Torrent URL request timed out"),
            UrlError::InvalidTorrent(m) => write!(f, "The URL did not return a valid torrent file: {m}"),
            UrlError::Network(m) => write!(f, "Torrent URL request failed: {m}"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FetchedTorrent {
    pub bytes: Vec<u8>,
    pub info_hash: String,
    pub name: Option<String>,
    pub file_count: usize,
    pub total_size: u64,
    pub final_url: String,
}

/// True for addresses that must never be fetched from: loopback, RFC1918,
/// link-local (incl. cloud metadata), CGNAT, benchmark, multicast, reserved,
/// unspecified, and IPv4-mapped IPv6 equivalents.
pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_ipv4(v4),
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_forbidden_ipv4(mapped);
            }
            let segs = v6.segments();
            v6.is_loopback()                                  // ::1
                || v6.is_unspecified()                        // ::
                || (segs[0] & 0xfe00) == 0xfc00               // fc00::/7 ULA
                || (segs[0] & 0xffc0) == 0xfe80               // fe80::/10 link-local
                || (segs[0] & 0xff00) == 0xff00               // multicast
                || segs[0] == 0x2002                          // 6to4 (embeds v4, skip cautiously)
                || (segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0 && segs[4] == 0
                    && segs[5] == 0xffff)                     // v4-mapped (belt & braces)
        }
    }
}

fn is_forbidden_ipv4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    let is_private = v4.is_loopback() || v4.is_private() || v4.is_link_local();
    let cgnat = o[0] == 100 && (o[1] & 0xC0) == 64; // 100.64.0.0/10
    let benchmark = o[0] == 198 && (o[1] == 18 || o[1] == 19); // 198.18.0.0/15
    let reserved = o[0] == 0 || o[0] >= 240 || (o[0] == 192 && o[1] == 0 && o[2] == 0);
    is_private || cgnat || benchmark || reserved
}

/// Extracts scheme + host for validation.
pub fn parse_target(url: &str) -> Result<(reqwest::Url,), UrlError> {
    let url = reqwest::Url::parse(url.trim()).map_err(|e| UrlError::InvalidUrl(e.to_string()))?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(UrlError::UnsupportedScheme),
    }
    let host = url
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| UrlError::InvalidUrl("missing host".to_string()))?;
    let _ = host;
    Ok((url,))
}

/// Resolves the host and rejects forbidden addresses. Returns allowed sockets.
pub async fn resolve_allowed(url: &reqwest::Url) -> Result<Vec<std::net::SocketAddr>, UrlError> {
    let host = url
        .host_str()
        .ok_or_else(|| UrlError::InvalidUrl("missing host".to_string()))?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| UrlError::InvalidUrl("no port".to_string()))?;
    let addrs = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| UrlError::Network(format!("DNS resolution failed: {e}")))?
        .collect::<Vec<_>>();
    if addrs.is_empty() {
        return Err(UrlError::Network("host resolved to no addresses".to_string()));
    }
    let allowed: Vec<std::net::SocketAddr> = addrs
        .into_iter()
        .filter(|a| !is_forbidden_ip(a.ip()))
        .collect();
    if allowed.is_empty() {
        return Err(UrlError::PrivateAddress(host));
    }
    Ok(allowed)
}

/// Resolves a (possibly relative) redirect target against the current URL.
pub fn follow_redirect(current: &reqwest::Url, location: &str) -> Result<reqwest::Url, UrlError> {
    let next = current
        .join(location)
        .map_err(|e| UrlError::InvalidUrl(format!("bad redirect target: {e}")))?;
    match next.scheme() {
        "http" | "https" => Ok(next),
        _ => Err(UrlError::UnsupportedScheme),
    }
}

fn client_for(url: &reqwest::Url, addr: std::net::SocketAddr, limits: &FetchLimits) -> Result<reqwest::Client, UrlError> {
    let host = url
        .host_str()
        .ok_or_else(|| UrlError::InvalidUrl("missing host".to_string()))?
        .to_string();
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(limits.timeout)
        .connect_timeout(Duration::from_secs(10))
        // Pin the connection to the DNS-validated address: defeats DNS rebinding
        // because the TLS/SNI host stays the original name but traffic goes to
        // the vetted IP only.
        .resolve(&host, addr)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| UrlError::Network(e.to_string()))
}

async fn read_bounded(resp: reqwest::Response, limits: &FetchLimits) -> Result<Vec<u8>, UrlError> {
    let declared = resp
        .content_length()
        .unwrap_or(0);
    if declared > limits.max_bytes {
        return Err(UrlError::TooLarge(declared));
    }
    let mut buf: Vec<u8> = Vec::with_capacity(declared.min(64 * 1024) as usize);
    let mut stream = resp;
    while let Some(chunk) = stream
        .chunk()
        .await
        .map_err(|e| UrlError::Network(e.to_string()))?
    {
        if (buf.len() as u64 + chunk.len() as u64) > limits.max_bytes {
            return Err(UrlError::TooLarge(buf.len() as u64));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// Fetches and validates a `.torrent` payload with full SSRF protection.
pub async fn fetch_torrent_file(
    start_url: &str,
    limits: &FetchLimits,
) -> Result<FetchedTorrent, UrlError> {
    let (mut url,) = parse_target(start_url)?;
    let mut hops = 0usize;
    loop {
        let addrs = resolve_allowed(&url).await?;
        let mut last_err: Option<UrlError> = None;
        let mut response = None;
        // Try each validated address (happy-eyeballs-lite).
        for addr in &addrs {
            match client_for(&url, *addr, limits) {
                Ok(client) => match client.get(url.clone()).send().await {
                    Ok(resp) => {
                        response = Some(resp);
                        break;
                    }
                    Err(e) => {
                        last_err = Some(UrlError::Network(
                            e.to_string().split(';').next().unwrap_or("request failed").to_string(),
                        ));
                    }
                },
                Err(e) => last_err = Some(e),
            }
        }
        let resp = response.ok_or_else(|| {
            last_err.unwrap_or_else(|| UrlError::Network("connection failed".to_string()))
        })?;

        let status = resp.status();
        if status.is_redirection() {
            if hops >= limits.max_redirects {
                return Err(UrlError::TooManyRedirects);
            }
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| UrlError::Network("redirect without location".to_string()))?
                .to_string();
            url = follow_redirect(&url, &location)?;
            hops += 1;
            continue;
        }
        if !status.is_success() {
            return Err(UrlError::Network(format!("server returned {status}")));
        }

        let bytes = read_bounded(resp, limits).await?;
        let torrent = validate_torrent(&bytes)?;
        return Ok(FetchedTorrent {
            bytes,
            info_hash: torrent.info_hash,
            name: torrent.name,
            file_count: torrent.file_count,
            total_size: torrent.total_size,
            final_url: url.to_string(),
        });
    }
}

pub struct TorrentMetaSummary {
    pub info_hash: String,
    pub name: Option<String>,
    pub file_count: usize,
    pub total_size: u64,
}

/// Structural validation + identity of a bencode metainfo payload.
pub fn validate_torrent(bytes: &[u8]) -> Result<TorrentMetaSummary, UrlError> {
    let parsed = librqbit::torrent_from_bytes(bytes)
        .map_err(|e| UrlError::InvalidTorrent(format!("not a valid torrent file ({e})")))?;
    let validated = parsed
        .info
        .data
        .validate()
        .map_err(|e| UrlError::InvalidTorrent(format!("invalid torrent structure ({e})")))?;
    let file_count = validated.iter_file_details().count();
    let total_size = validated.lengths().total_length();
    Ok(TorrentMetaSummary {
        info_hash: parsed.info_hash.as_string(),
        name: validated.name().map(|n| n.to_string()),
        file_count,
        total_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_ipv4_ranges() {
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(127, 5, 6, 7))));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 1, 1))));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(172, 31, 9, 9))));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254)))); // cloud metadata
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)))); // CGNAT
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1)))); // benchmark
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));
        assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255))));
    }

    #[test]
    fn public_ipv4_allowed() {
        assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1)))); // just outside 172.16/12
        assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(100, 128, 0, 1)))); // just outside CGNAT
    }

    #[test]
    fn forbidden_ipv6_ranges() {
        assert!(is_forbidden_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_forbidden_ip(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        assert!(is_forbidden_ip(IpAddr::V6("fe80::1".parse().unwrap())));
        assert!(is_forbidden_ip(IpAddr::V6("fc00::1".parse().unwrap())));
        assert!(is_forbidden_ip(IpAddr::V6("::ffff:10.0.0.1".parse().unwrap()))); // v4-mapped
        assert!(!is_forbidden_ip(IpAddr::V6("2606:4700::1".parse().unwrap())));
    }

    #[test]
    fn schemes_validated() {
        assert!(parse_target("https://example.com/a.torrent").is_ok());
        assert!(parse_target("http://example.com/a.torrent").is_ok());
        assert!(matches!(
            parse_target("ftp://example.com/a.torrent"),
            Err(UrlError::UnsupportedScheme)
        ));
        assert!(matches!(
            parse_target("file:///etc/passwd"),
            Err(UrlError::UnsupportedScheme)
        ));
        assert!(matches!(
            parse_target("http://127.0.0.1/x.torrent"),
            Err(UrlError::UnsupportedScheme)
        ) == false); // scheme is fine; the IP check rejects it at resolve time
        assert!(parse_target("not a url").is_err());
    }

    #[test]
    fn redirect_following_resolves_relative_and_rejects_schemes() {
        let base = reqwest::Url::parse("https://example.com/files/a.torrent").unwrap();
        let abs = follow_redirect(&base, "https://cdn.example.net/b.torrent").unwrap();
        assert_eq!(abs.host_str(), Some("cdn.example.net"));
        let rel = follow_redirect(&base, "../mirror/c.torrent").unwrap();
        assert_eq!(rel.path(), "/mirror/c.torrent");
        assert!(matches!(
            follow_redirect(&base, "ftp://evil/x"),
            Err(UrlError::UnsupportedScheme)
        ));
    }

    #[test]
    fn validate_torrent_accepts_and_rejects() {
        // Minimal handcrafted single-file metainfo; pieces must be a 20-byte multiple.
        let torrent = b"d4:infod6:lengthi4e4:name3:abc12:piece lengthi16384e6:pieces20:01234567890123456789ee";
        let summary = validate_torrent(torrent).expect("valid metainfo");
        assert_eq!(summary.file_count, 1);
        assert_eq!(summary.total_size, 4);
        assert_eq!(summary.name.as_deref(), Some("abc"));
        assert_eq!(summary.info_hash.len(), 40);

        assert!(validate_torrent(b"not bencode at all").is_err());
        assert!(validate_torrent(b"d1:ai1ee").is_err()); // no info dict
        // info dict missing the pieces field
        assert!(validate_torrent(b"d4:infod6:lengthi4e4:name3:abc12:piece lengthi16384ee4:name3:abce").is_err());
    }
}
