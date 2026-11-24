use std::collections::HashMap;
use std::time::Duration;
use std::{future::Future, pin::Pin, sync::Arc};

use serde::{Deserialize, Serialize};

/// Transport-level fetch result decoupled from reqwest so tests can mock HTTP.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub status: u16,
    pub body: String,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum FetchError {
    Network(String),
    Timeout(String),
}

/// Object-safe async fetcher (boxed futures; avoids an async-trait dependency).
pub trait HttpFetcher: Send + Sync {
    fn fetch<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>> + Send + 'a>>;
}

pub struct ReqwestFetcher {
    client: reqwest::Client,
}

impl ReqwestFetcher {
    pub fn new(timeout: Duration) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(timeout)
            .user_agent("StreamNest/0.2 (+https://github.com/san-ratul/stream-nest)")
            .build()?;
        Ok(Self { client })
    }
}

impl HttpFetcher for ReqwestFetcher {
    fn fetch<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>> + Send + 'a>> {
        Box::pin(async move {
            let resp = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| {
                    // Never leak full URLs (they carry the api_key) into error strings.
                    let e = if e.is_timeout() {
                        FetchError::Timeout("request exceeded configured timeout".to_string())
                    } else {
                        FetchError::Network(
                            e.without_url()
                                .status()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "connection failed".to_string()),
                        )
                    };
                    e
                })?;
            let status = resp.status().as_u16();
            let retry_after_seconds = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok());
            let body = resp.text().await.map_err(|e| {
                FetchError::Network(
                    e.without_url().to_string().split(';').next().unwrap_or("read failed").to_string(),
                )
            })?;
            Ok(FetchResponse { status, body, retry_after_seconds })
        })
    }
}

#[derive(Debug, Clone)]
pub enum TmdbError {
    Auth,
    RateLimited { retry_after_seconds: Option<u64> },
    NotFound,
    InvalidResponse(String),
    Network(String),
    Timeout,
}

impl std::fmt::Display for TmdbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Credential-related failures must not echo any request material.
            TmdbError::Auth => write!(f, "TMDB rejected the configured API key (auth failed)"),
            TmdbError::RateLimited { retry_after_seconds } => match retry_after_seconds {
                Some(s) => write!(f, "TMDB rate limit reached (retry after {s}s)"),
                None => write!(f, "TMDB rate limit reached"),
            },
            TmdbError::NotFound => write!(f, "TMDB resource not found"),
            TmdbError::InvalidResponse(m) => write!(f, "invalid TMDB response: {m}"),
            TmdbError::Network(m) => write!(f, "TMDB request failed: {m}"),
            TmdbError::Timeout => write!(f, "TMDB request timed out"),
        }
    }
}

impl std::error::Error for TmdbError {}

impl From<rusqlite::Error> for TmdbError {
    fn from(e: rusqlite::Error) -> Self {
        TmdbError::Network(format!("database error: {e}"))
    }
}

/// TMDB image configuration (documented /configuration endpoint).
#[derive(Debug, Clone, Serialize)]
pub struct ImageConfig {
    pub secure_base_url: String,
    pub poster_sizes: Vec<String>,
    pub backdrop_sizes: Vec<String>,
    pub still_sizes: Vec<String>,
    pub profile_sizes: Vec<String>,
}

impl Default for ImageConfig {
    fn default() -> Self {
        // Documented defaults, used until /configuration is fetched.
        Self {
            secure_base_url: "https://image.tmdb.org/t/p/".to_string(),
            poster_sizes: ["w154", "w342", "w500", "original"]
                .into_iter()
                .map(String::from)
                .collect(),
            backdrop_sizes: ["w780", "w1280", "original"]
                .into_iter()
                .map(String::from)
                .collect(),
            still_sizes: ["w185", "w300", "original"].into_iter().map(String::from).collect(),
            profile_sizes: ["w185", "h632", "original"].into_iter().map(String::from).collect(),
        }
    }
}

/// Generic paginated list wrapper.
#[derive(Debug, Deserialize)]
pub struct TmdbPage<T> {
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub results: Vec<T>,
    #[serde(default, rename = "total_pages")]
    pub total_pages: i64,
}

/// A movie or TV list result item. Field names cover both shapes.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MediaListItem {
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub original_title: Option<String>,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub popularity: Option<f64>,
    #[serde(default)]
    pub vote_average: Option<f64>,
    #[serde(default, rename = "vote_count")]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub original_language: Option<String>,
    #[serde(default)]
    pub adult: Option<bool>,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub genre_ids: Vec<i64>,
}

impl MediaListItem {
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.name.clone())
            .or_else(|| self.original_title.clone())
            .or_else(|| self.original_name.clone())
            .unwrap_or_else(|| format!("TMDB #{}", self.id))
    }

    pub fn date(&self) -> Option<String> {
        self.release_date.clone().or_else(|| self.first_air_date.clone())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GenreItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct GenreList {
    #[serde(default)]
    pub genres: Vec<GenreItem>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CastMember {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub character: Option<String>,
    #[serde(default)]
    pub profile_path: Option<String>,
    #[serde(default)]
    pub order: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CrewMember {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub job: Option<String>,
    #[serde(default)]
    pub profile_path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Credits {
    #[serde(default)]
    pub cast: Vec<CastMember>,
    #[serde(default)]
    pub crew: Vec<CrewMember>,
}

#[derive(Debug, Default, Deserialize)]
pub struct MovieDetails {
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub original_title: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub popularity: Option<f64>,
    #[serde(default)]
    pub vote_average: Option<f64>,
    #[serde(default, rename = "vote_count")]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub original_language: Option<String>,
    #[serde(default)]
    pub adult: Option<bool>,
    #[serde(default)]
    pub runtime: Option<i64>,
    #[serde(default)]
    pub genres: Vec<GenreItem>,
    #[serde(default)]
    pub credits: Option<Credits>,
}

impl MovieDetails {
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.original_title.clone())
            .unwrap_or_else(|| format!("TMDB #{}", self.id))
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SeasonSummary {
    pub id: i64,
    #[serde(default)]
    pub season_number: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default, rename = "episode_count")]
    pub episode_count: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EpisodeItem {
    pub id: i64,
    #[serde(default)]
    pub episode_number: i64,
    #[serde(default)]
    pub season_number: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub still_path: Option<String>,
    #[serde(default)]
    pub vote_average: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TvDetails {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub popularity: Option<f64>,
    #[serde(default)]
    pub vote_average: Option<f64>,
    #[serde(default, rename = "vote_count")]
    pub vote_count: Option<i64>,
    #[serde(default)]
    pub original_language: Option<String>,
    #[serde(default)]
    pub genres: Vec<GenreItem>,
    #[serde(default)]
    pub seasons: Vec<SeasonSummary>,
    #[serde(default)]
    pub credits: Option<Credits>,
    #[serde(default, rename = "number_of_seasons")]
    pub number_of_seasons: Option<i64>,
}

impl TvDetails {
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .or_else(|| self.original_name.clone())
            .unwrap_or_else(|| format!("TMDB #{}", self.id))
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct SeasonDetails {
    pub id: i64,
    #[serde(default)]
    pub season_number: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub episodes: Vec<EpisodeItem>,
}

#[derive(Debug, Deserialize)]
struct ConfigurationResponse {
    #[serde(default)]
    images: Option<ConfigurationImages>,
}

#[derive(Debug, Deserialize)]
struct ConfigurationImages {
    #[serde(default)]
    secure_base_url: Option<String>,
    #[serde(default)]
    poster_sizes: Vec<String>,
    #[serde(default)]
    backdrop_sizes: Vec<String>,
    #[serde(default)]
    still_sizes: Vec<String>,
    #[serde(default)]
    profile_sizes: Vec<String>,
}

const MAX_ATTEMPTS: u32 = 3;
const BACKOFFS_MS: [u64; MAX_ATTEMPTS as usize] = [500, 2000, 0];

/// TMDB API client. All requests carry the key as the documented `api_key`
/// query parameter; the key never appears in errors or logs.
pub struct TmdbClient {
    fetcher: Arc<dyn HttpFetcher>,
    api_key: String,
}

impl TmdbClient {
    pub fn new(fetcher: Arc<dyn HttpFetcher>, api_key: String) -> Self {
        Self { fetcher, api_key }
    }

    fn build_url(&self, path: &str, params: &[(&str, String)]) -> String {
        // Some endpoints (discover) embed query params in their path; keep the
        // separator valid in both cases.
        let sep = if path.contains('?') { "&" } else { "?" };
        let mut url = format!(
            "https://api.themoviedb.org/3/{path}{sep}api_key={}",
            self.api_key
        );
        if let Some((_, lang)) = params.iter().find(|(k, _)| *k == "language") {
            url.push_str(&format!("&language={lang}"));
        }
        for (k, v) in params {
            if *k == "language" {
                continue;
            }
            url.push_str(&format!("&{k}={}", urlencode(v)));
        }
        url
    }

    async fn get_raw(&self, path: &str, params: &[(&str, String)]) -> Result<FetchResponse, TmdbError> {
        let mut last_err: Option<TmdbError> = None;
        for attempt in 0..MAX_ATTEMPTS {
            let url = self.build_url(path, params);
            let result = self.fetcher.fetch(&url).await;
            // The URL embeds the credential: drop it from anything we keep.
            drop(url);
            match result {
                Ok(resp) => match resp.status {
                    200 => return Ok(resp),
                    401 | 403 => return Err(TmdbError::Auth),
                    404 => return Err(TmdbError::NotFound),
                    429 => {
                        let err = TmdbError::RateLimited {
                            retry_after_seconds: resp.retry_after_seconds,
                        };
                        if attempt + 1 < MAX_ATTEMPTS {
                            let wait = resp
                                .retry_after_seconds
                                .unwrap_or(BACKOFFS_MS[attempt as usize] / 1000)
                                .min(30);
                            tokio::time::sleep(Duration::from_secs(wait.max(1))).await;
                            last_err = Some(err);
                        } else {
                            return Err(err);
                        }
                    }
                    s if s >= 500 => {
                        if attempt + 1 < MAX_ATTEMPTS {
                            tokio::time::sleep(Duration::from_millis(BACKOFFS_MS[attempt as usize])).await;
                            last_err = Some(TmdbError::Network(format!("server error {s}")));
                        } else {
                            return Err(TmdbError::Network(format!("server error {s}")));
                        }
                    }
                    s => {
                        return Err(TmdbError::InvalidResponse(format!("unexpected status {s}")));
                    }
                },
                Err(FetchError::Timeout(m)) => {
                    if attempt + 1 < MAX_ATTEMPTS {
                        tokio::time::sleep(Duration::from_millis(BACKOFFS_MS[attempt as usize])).await;
                        last_err = Some(TmdbError::Timeout);
                        let _ = m;
                    } else {
                        return Err(TmdbError::Timeout);
                    }
                }
                Err(FetchError::Network(m)) => {
                    if attempt + 1 < MAX_ATTEMPTS {
                        tokio::time::sleep(Duration::from_millis(BACKOFFS_MS[attempt as usize])).await;
                        last_err = Some(TmdbError::Network(m));
                    } else {
                        return Err(TmdbError::Network(m));
                    }
                }
            }
        }
        Err(last_err.unwrap_or(TmdbError::Network("request failed".to_string())))
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T, TmdbError> {
        let resp = self.get_raw(path, params).await?;
        serde_json::from_str(&resp.body)
            .map_err(|e| TmdbError::InvalidResponse(short_parse_error(&e.to_string())))
    }

    pub async fn configuration(&self) -> Result<ImageConfig, TmdbError> {
        let cfg: ConfigurationResponse = self.get_json("configuration", &[]).await?;
        Ok(cfg
            .images
            .map(|i| ImageConfig {
                secure_base_url: i.secure_base_url.unwrap_or_else(|| {
                    "https://image.tmdb.org/t/p/".to_string()
                }),
                poster_sizes: i.poster_sizes,
                backdrop_sizes: i.backdrop_sizes,
                still_sizes: i.still_sizes,
                profile_sizes: i.profile_sizes,
            })
            .unwrap_or_default())
    }

    /// Fetch one page of a list-style endpoint (`path` may embed query parts).
    pub async fn list_page(
        &self,
        path: &str,
        page: u32,
    ) -> Result<TmdbPage<MediaListItem>, TmdbError> {
        self.get_json(path, &[("page", page.to_string())]).await
    }

    pub async fn movie_genres(&self) -> Result<Vec<GenreItem>, TmdbError> {
        let list: GenreList = self.get_json("genre/movie/list", &[]).await?;
        Ok(list.genres)
    }

    pub async fn tv_genres(&self) -> Result<Vec<GenreItem>, TmdbError> {
        let list: GenreList = self.get_json("genre/tv/list", &[]).await?;
        Ok(list.genres)
    }

    pub async fn movie_details(&self, id: i64) -> Result<MovieDetails, TmdbError> {
        self.get_json(&format!("movie/{id}"), &[("append_to_response", "credits".to_string())])
            .await
    }

    pub async fn tv_details(&self, id: i64) -> Result<TvDetails, TmdbError> {
        self.get_json(&format!("tv/{id}"), &[("append_to_response", "credits".to_string())])
            .await
    }

    pub async fn season_details(
        &self,
        tv_id: i64,
        season_number: i64,
    ) -> Result<SeasonDetails, TmdbError> {
        self.get_json(&format!("tv/{tv_id}/season/{season_number}"), &[])
            .await
    }

    pub async fn search_multi(&self, query: &str, page: u32) -> Result<TmdbPage<MediaListItem>, TmdbError> {
        self.get_json(
            "search/multi",
            &[
                ("query", query.to_string()),
                ("page", page.to_string()),
                ("include_adult", "false".to_string()),
            ],
        )
        .await
    }
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn short_parse_error(msg: &str) -> String {
    // serde errors can quote long fragments of the body; keep them bounded.
    let mut s = msg.replace('\n', " ");
    if s.len() > 200 {
        s.truncate(200);
    }
    s
}

pub fn best_image_size(sizes: &[String], preferred: &[&str]) -> String {
    for p in preferred {
        if sizes.iter().any(|s| s == p) {
            return (*p).to_string();
        }
    }
    sizes
        .iter()
        .find(|s| s.starts_with('w'))
        .cloned()
        .unwrap_or_else(|| "original".to_string())
}

/// Useful for building raw image URLs when not proxying.
pub fn image_url(cfg: &ImageConfig, kind: &str, size: &str, path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let base = cfg.secure_base_url.clone();
    Some(format!("{base}{kind}/{size}{path}"))
}

pub type ParamMap = HashMap<String, String>;

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// Static-response fetcher: maps URL substring to status/body.
    pub struct MockFetcher {
        pub routes: Vec<MockRoute>,
        pub calls: std::sync::Mutex<Vec<String>>,
    }

    pub struct MockRoute {
        pub matches: &'static str,
        pub status: u16,
        pub body: &'static str,
        pub retry_after: Option<u64>,
    }

    impl HttpFetcher for MockFetcher {
        fn fetch<'a>(
            &'a self,
            url: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>> + Send + 'a>> {
            self.calls.lock().unwrap().push(url.to_string());
            let hit = self
                .routes
                .iter()
                .find(|r| url.contains(r.matches))
                .map(|r| (r.status, r.body, r.retry_after));
            Box::pin(async move {
                match hit {
                    Some((status, body, retry)) => Ok(FetchResponse {
                        status,
                        body: body.to_string(),
                        retry_after_seconds: retry,
                    }),
                    None => Ok(FetchResponse {
                        status: 404,
                        body: "{\"status_message\":\"Not found\"}".to_string(),
                        retry_after_seconds: None,
                    }),
                }
            })
        }
    }

    pub fn mock(routes: Vec<MockRoute>) -> Arc<MockFetcher> {
        Arc::new(MockFetcher { routes, calls: std::sync::Mutex::new(Vec::new()) })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    const LIST_BODY: &str = r#"{
      "page": 1, "total_pages": 8,
      "results": [
        {"id": 1, "title": "Some Movie", "poster_path": "/abc.jpg", "vote_average": 7.1,
         "vote_count": 120, "release_date": "2024-05-01", "genre_ids": [28, 99]},
        {"id": 2, "name": "Some Show", "first_air_date": "2023-11-12"},
        {"id": 3}
      ]
    }"#;

    fn client_with(routes: Vec<MockRoute>) -> (TmdbClient, Arc<MockFetcher>) {
        let f = mock(routes);
        let client = TmdbClient::new(f.clone(), "test-key".to_string());
        (client, f)
    }

    #[tokio::test]
    async fn successful_list_parses_and_defaults_missing_fields() {
        let (client, f) = client_with(vec![MockRoute {
            matches: "movie/popular",
            status: 200,
            body: LIST_BODY,
            retry_after: None,
        }]);
        let page = client.list_page("movie/popular", 1).await.unwrap();
        assert_eq!(page.results.len(), 3);
        assert_eq!(page.results[0].display_title(), "Some Movie");
        assert_eq!(page.results[1].display_title(), "Some Show");
        assert_eq!(page.results[2].display_title(), "TMDB #3");
        assert_eq!(page.results[2].vote_average, None);
        let url = &f.calls.lock().unwrap()[0];
        assert!(url.contains("page=1"), "page param must be sent: {url}");
        assert!(url.contains("api_key=test-key"));
    }

    #[tokio::test]
    async fn auth_failure_maps_to_auth_error() {
        let (client, _) = client_with(vec![MockRoute {
            matches: "movie/popular",
            status: 401,
            body: r#"{"status_code":7,"status_message":"Invalid API key"}"#,
            retry_after: None,
        }]);
        let err = client.list_page("movie/popular", 1).await.unwrap_err();
        assert!(matches!(err, TmdbError::Auth));
        // The message must not leak the key material.
        assert!(!err.to_string().contains("test-key"));
    }

    #[tokio::test]
    async fn invalid_json_is_invalid_response() {
        let (client, _) = client_with(vec![MockRoute {
            matches: "movie/popular",
            status: 200,
            body: "not json at all {",
            retry_after: None,
        }]);
        let err = client.list_page("movie/popular", 1).await.unwrap_err();
        assert!(matches!(err, TmdbError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn rate_limit_maps_to_rate_limited_with_retry_after() {
        let (client, _) = client_with(vec![MockRoute {
            matches: "movie/popular",
            status: 429,
            body: "{}",
            retry_after: Some(1),
        }]);
        let err = client.list_page("movie/popular", 1).await.unwrap_err();
        assert!(matches!(
            err,
            TmdbError::RateLimited { retry_after_seconds: Some(1) }
        ));
    }

    #[tokio::test]
    async fn server_errors_are_retried_then_reported() {
        let (client, f) = client_with(vec![MockRoute {
            matches: "movie/popular",
            status: 503,
            body: "unavailable",
            retry_after: None,
        }]);
        let err = client.list_page("movie/popular", 1).await.unwrap_err();
        assert!(matches!(err, TmdbError::Network(_)));
        assert_eq!(f.calls.lock().unwrap().len(), MAX_ATTEMPTS as usize);
    }

    #[tokio::test]
    async fn pagination_uses_requested_page() {
        let (client, f) = client_with(vec![MockRoute {
            matches: "trending/movie/day",
            status: 200,
            body: LIST_BODY,
            retry_after: None,
        }]);
        client.list_page("trending/movie/day", 3).await.unwrap();
        assert!(f.calls.lock().unwrap()[0].contains("page=3"));
    }

    #[tokio::test]
    async fn configuration_defaults_when_images_missing() {
        let (client, _) = client_with(vec![MockRoute {
            matches: "configuration",
            status: 200,
            body: "{}",
            retry_after: None,
        }]);
        let cfg = client.configuration().await.unwrap();
        assert_eq!(cfg.secure_base_url, "https://image.tmdb.org/t/p/");
        assert!(!cfg.poster_sizes.is_empty());
    }

    #[tokio::test]
    async fn movie_details_parses_credits_and_runtime() {
        let (client, _) = client_with(vec![MockRoute {
            matches: "movie/42",
            status: 200,
            body: r#"{"id":42,"title":"Detail Movie","runtime":96,
                      "genres":[{"id":99,"name":"Documentary"}],
                      "credits":{"cast":[{"id":7,"name":"Ada","character":"Hero"}],
                                 "crew":[{"id":8,"name":"Bob","job":"Director"}]}}"#,
            retry_after: None,
        }]);
        let details = client.movie_details(42).await.unwrap();
        assert_eq!(details.runtime, Some(96));
        assert_eq!(details.credits.unwrap().cast[0].name.as_deref(), Some("Ada"));
    }

    #[test]
    fn urlencode_escapes_spaces() {
        assert_eq!(urlencode("big buck"), "big%20buck");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
    }
}
