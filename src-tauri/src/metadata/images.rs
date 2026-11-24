use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use parking_lot::RwLock;

use super::tmdb::ImageConfig;

pub struct ImageServerInfo {
    pub port: u16,
    pub token: Arc<String>,
}

struct ImageState {
    token: String,
    port: u16,
    cache_dir: PathBuf,
    image_config: Arc<RwLock<ImageConfig>>,
    cache_limit_mb: u64,
}

/// Starts the local TMDB image proxy. Binds 127.0.0.1 only, random token path,
/// host-header check — same model as the streaming server.
/// The frontend never contacts image.tmdb.org directly (CSP stays tight).
pub async fn spawn(
    cache_dir: PathBuf,
    image_config: Arc<RwLock<ImageConfig>>,
    cache_limit_mb: u64,
) -> anyhow::Result<ImageServerInfo> {
    std::fs::create_dir_all(&cache_dir)?;
    let token = Arc::new(uuid::Uuid::new_v4().simple().to_string());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    let state = Arc::new(ImageState {
        token: (*token).clone(),
        port,
        cache_dir,
        image_config,
        cache_limit_mb,
    });

    let app = Router::new()
        .route("/{token}/tmdb/{kind}/{size}/{file}", get(serve_image))
        .with_state(state);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("image server terminated: {e}");
        }
    });

    log::info!("image proxy listening on 127.0.0.1:{port}");
    Ok(ImageServerInfo { port, token })
}

/// Local URL for a TMDB image path (poster/backdrop/still/profile file name like `/abc.jpg`).
pub fn local_url(info: &ImageServerInfo, kind: &str, size: &str, tmdb_path: &str) -> Option<String> {
    let file = tmdb_path.trim_start_matches('/');
    if !valid_file(file) || !valid_kind(kind) || !valid_size(size) {
        return None;
    }
    Some(format!(
        "http://127.0.0.1:{}/tmdb/{}/{}/{}",
        info.port, kind, size, file
    ))
}

fn valid_file(file: &str) -> bool {
    if file.is_empty() || file.len() > 128 {
        return false;
    }
    if file.contains("..") || file.contains('\\') || file.contains('/') {
        return false;
    }
    let ok_ext = file.ends_with(".jpg")
        || file.ends_with(".jpeg")
        || file.ends_with(".png")
        || file.ends_with(".webp")
        || file.ends_with(".svg");
    ok_ext
        && file.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn valid_size(size: &str) -> bool {
    !size.is_empty()
        && size.len() <= 12
        && size
            .chars()
            .all(|c| c.is_ascii_alphanumeric())
}

fn valid_kind(kind: &str) -> bool {
    matches!(kind, "poster" | "backdrop" | "still" | "profile")
}

fn content_type(file: &str) -> &'static str {
    if file.ends_with(".png") {
        "image/png"
    } else if file.ends_with(".webp") {
        "image/webp"
    } else if file.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "image/jpeg"
    }
}

fn cache_path(state: &ImageState, kind: &str, size: &str, file: &str) -> PathBuf {
    state.cache_dir.join(format!("{kind}_{size}_{file}"))
}

fn remote_url(state: &ImageState, size: &str, file: &str) -> String {
    // TMDB CDN serves all image kinds from /t/p/{size}/{file}; the kind segment
    // is only a local cache-namespace concern.
    let base = state.image_config.read().secure_base_url.clone();
    format!("{base}{size}/{file}")
}

async fn enforce_cache_limit(dir: PathBuf, limit_mb: u64) {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                files.push((entry.path(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)));
            }
        }
    }
    let limit_bytes = limit_mb.saturating_mul(1024 * 1024);
    let total: u64 = files.iter().map(|f| f.1).sum();
    if total <= limit_bytes {
        return;
    }
    files.sort_by_key(|f| f.2);
    let mut total = total;
    for (path, size, _) in files {
        if total <= limit_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

async fn serve_image(
    State(state): State<Arc<ImageState>>,
    Path((token, kind, size, file)): Path<(String, String, String, String)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if token != state.token {
        return Err(StatusCode::NOT_FOUND);
    }
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok());
    let host_ok = host
        .map(|h| h == format!("127.0.0.1:{}", state.port) || h == format!("localhost:{}", state.port))
        .unwrap_or(false);
    if !host_ok {
        return Err(StatusCode::FORBIDDEN);
    }
    if !valid_kind(&kind) || !valid_size(&size) || !valid_file(&file) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let local = cache_path(&state, &kind, &size, &file);
    if let Ok(bytes) = std::fs::read(&local) {
        // Touch for LRU (best-effort).
        let _ = std::fs::File::options().append(true).open(&local);
        let mut resp_headers = HeaderMap::new();
        if let Ok(v) = content_type(&file).parse() {
            resp_headers.insert(header::CONTENT_TYPE, v);
        }
        resp_headers.insert(header::CACHE_CONTROL, "max-age=86400".parse().unwrap());
        return Ok((StatusCode::OK, resp_headers, bytes));
    }

    let url = remote_url(&state, &size, &file);
    let resp = reqwest::get(&url).await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    if !resp.status().is_success() {
        return Err(StatusCode::NOT_FOUND);
    }
    let bytes = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let _ = std::fs::write(&local, &bytes);
    tokio::spawn(enforce_cache_limit(
        state.cache_dir.clone(),
        state.cache_limit_mb,
    ));

    let mut resp_headers = HeaderMap::new();
    if let Ok(v) = content_type(&file).parse() {
        resp_headers.insert(header::CONTENT_TYPE, v);
    }
    resp_headers.insert(header::CACHE_CONTROL, "max-age=86400".parse().unwrap());
    Ok((StatusCode::OK, resp_headers, bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_file_accepts_tmdb_style_names() {
        assert!(valid_file("abc123.jpg"));
        assert!(valid_file("AbC-1_2.webp"));
        assert!(valid_file("x.png"));
    }

    #[test]
    fn valid_file_rejects_traversal_and_paths() {
        assert!(!valid_file("..%2F..%2Fsecret.jpg"));
        assert!(!valid_file("../secret.jpg"));
        assert!(!valid_file("a/b.jpg"));
        assert!(!valid_file("a\\b.jpg"));
        assert!(!valid_file(""));
        assert!(!valid_file("noext"));
        assert!(!valid_file("x.txt"));
    }

    #[test]
    fn size_and_kind_validation() {
        assert!(valid_size("w500"));
        assert!(valid_size("original"));
        assert!(!valid_size(""));
        assert!(!valid_size("w500/.."));
        assert!(valid_kind("poster"));
        assert!(valid_kind("still"));
        assert!(!valid_kind("anything"));
    }

    #[test]
    fn local_url_builds_token_url_or_none() {
        let info = ImageServerInfo { port: 12345, token: Arc::new("tok".to_string()) };
        let url = local_url(&info, "poster", "w342", "/abc.jpg").unwrap();
        assert_eq!(url, "http://127.0.0.1:12345/tmdb/poster/w342/abc.jpg");
        assert!(local_url(&info, "poster", "w342", "/../etc.jpg").is_none());
        assert!(local_url(&info, "bogus", "w342", "/abc.jpg").is_none());
    }
}
