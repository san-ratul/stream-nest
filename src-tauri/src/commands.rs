use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose};
use librqbit::{
    AddTorrent,
    AddTorrentOptions,
    ApiError,
    api::{
        ApiAddTorrentResponse,
        TorrentDetailsResponse,
        TorrentIdOrHash,
        TorrentListResponse,
        TorrentStats,
    },
    http_api_types::{PeerStatsFilter, PeerStatsSnapshot},
    session_stats::snapshot::SessionStatsSnapshot,
};
use serde::Serialize;
use tauri::State;

use crate::engine::{EngineShared, EngineState};
use crate::storage::Db;

fn parse_peers(peers: Option<Vec<String>>) -> Result<Option<Vec<std::net::SocketAddr>>, String> {
    let Some(peers) = peers else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for p in peers {
        let addr: std::net::SocketAddr = p
            .parse()
            .map_err(|_| format!("invalid peer address: {p}"))?;
        out.push(addr);
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

#[derive(Serialize)]
pub struct StorageUsage {
    pub path: String,
    pub bytes: u64,
    pub files: u64,
}

fn dir_usage(path: &std::path::Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    if let Ok(entries) = walkdir(path) {
        for (size, _modified) in entries {
            bytes += size;
            files += 1;
        }
    }
    (bytes, files)
}

fn walkdir(path: &std::path::Path) -> std::io::Result<Vec<(u64, std::time::SystemTime)>> {
    let mut out = Vec::new();
    if path.is_file() {
        let meta = std::fs::metadata(path)?;
        out.push((meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)));
        return Ok(out);
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                out.extend(walkdir(&p)?);
            } else if let Ok(meta) = std::fs::metadata(&p) {
                out.push((meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)));
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn storage_usage(state: State<'_, EngineState>) -> CommandResult<StorageUsage> {
    let shared = shared(&state)?;
    let (bytes, files) = dir_usage(&shared.download_dir);
    Ok(StorageUsage {
        path: shared.download_dir.to_string_lossy().to_string(),
        bytes,
        files,
    })
}

#[tauri::command]
pub fn clear_subtitles(state: State<'_, EngineState>) -> CommandResult<u64> {
    let shared = shared(&state)?;
    let dir = shared
        .download_dir
        .parent()
        .map(|p| p.join("subtitles"))
        .unwrap_or_else(|| std::path::PathBuf::from("subtitles"));
    let mut removed = 0u64;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

#[tauri::command]
pub fn apply_engine_limits(
    engine: State<'_, EngineState>,
    db: State<'_, Db>,
    download_limit_kbps: i64,
    upload_limit_kbps: i64,
) -> CommandResult<()> {
    let shared = shared(&engine)?;
    let session = shared.api.session();
    let to_bps = |kbps: i64| {
        if kbps > 0 {
            Some(std::num::NonZeroU32::new((kbps * 1000) as u32).unwrap_or_else(|| std::num::NonZeroU32::new(1).unwrap()))
        } else {
            None
        }
    };
    session.ratelimits.set_download_bps(to_bps(download_limit_kbps));
    session.ratelimits.set_upload_bps(to_bps(upload_limit_kbps));
    Ok(())
}

fn read_setting(db: &Db, key: &str, default: i64) -> i64 {
    let conn = db.0.lock();
    conn.query_row("SELECT value FROM app_settings WHERE key = ?1", [key], |r| {
        let v: String = r.get(0)?;
        Ok(v.parse::<i64>().unwrap_or(default))
    })
    .unwrap_or(default)
}

fn read_connection_limit(db: &Db) -> i64 {
    read_setting(db, "connectionLimit", 100)
}

#[derive(Serialize)]
pub struct CommandError {
    pub message: String,
}

impl CommandError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(e: anyhow::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<ApiError> for CommandError {
    fn from(e: ApiError) -> Self {
        Self::new(e.to_string())
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

fn shared(state: &EngineState) -> CommandResult<Arc<EngineShared>> {
    state
        .shared()
        .ok_or_else(|| CommandError::new("torrent engine is not ready yet"))
}

#[derive(Serialize)]
pub struct EngineStatus {
    pub ready: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn engine_status(state: State<'_, EngineState>) -> EngineStatus {
    EngineStatus {
        ready: state.shared().is_some(),
        error: state.init_error.read().clone(),
    }
}

#[tauri::command]
pub async fn torrent_add_uri(
    state: State<'_, EngineState>,
    db: State<'_, Db>,
    uri: String,
    peers: Option<Vec<String>>,
) -> CommandResult<ApiAddTorrentResponse> {
    let initial_peers =
        parse_peers(peers).map_err(CommandError::new)?;
    let opts = AddTorrentOptions {
        overwrite: false,
        initial_peers,
        peer_limit: Some(read_connection_limit(&db) as usize),
        ..Default::default()
    };
    let shared = shared(&state)?;
    Ok(shared
        .api
        .api_add_torrent(AddTorrent::Url(uri.into()), Some(opts))
        .await?)
}

#[tauri::command]
pub async fn torrent_add_file_base64(
    state: State<'_, EngineState>,
    db: State<'_, Db>,
    contents: String,
    peers: Option<Vec<String>>,
) -> CommandResult<ApiAddTorrentResponse> {
    let bytes = general_purpose::STANDARD
        .decode(contents.as_bytes())
        .map_err(|e| CommandError::new(format!("invalid base64 torrent file: {e}")))?;
    let initial_peers =
        parse_peers(peers).map_err(CommandError::new)?;
    let opts = AddTorrentOptions {
        overwrite: false,
        initial_peers,
        peer_limit: Some(read_connection_limit(&db) as usize),
        ..Default::default()
    };
    let shared = shared(&state)?;
    Ok(shared
        .api
        .api_add_torrent(AddTorrent::TorrentFileBytes(bytes.into()), Some(opts))
        .await?)
}

#[tauri::command]
pub fn torrent_list(state: State<'_, EngineState>) -> CommandResult<TorrentListResponse> {
    let shared = shared(&state)?;
    Ok(shared
        .api
        .api_torrent_list_ext(librqbit::api::ApiTorrentListOpts { with_stats: true }))
}

#[tauri::command]
pub async fn torrent_details(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
) -> CommandResult<TorrentDetailsResponse> {
    let shared = shared(&state)?;
    Ok(shared.api.api_torrent_details(id)?)
}

#[tauri::command]
pub async fn torrent_stats(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
) -> CommandResult<TorrentStats> {
    let shared = shared(&state)?;
    Ok(shared.api.api_stats_v1(id)?)
}

#[tauri::command]
pub fn torrent_peer_stats(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
    filter: Option<PeerStatsFilter>,
) -> CommandResult<PeerStatsSnapshot> {
    let shared = shared(&state)?;
    Ok(shared
        .api
        .api_peer_stats(id, filter.unwrap_or_default())?)
}

#[tauri::command]
pub async fn torrent_pause(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
) -> CommandResult<()> {
    let shared = shared(&state)?;
    shared.api.api_torrent_action_pause(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn torrent_start(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
) -> CommandResult<()> {
    let shared = shared(&state)?;
    shared.api.api_torrent_action_start(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn torrent_forget(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
) -> CommandResult<()> {
    let shared = shared(&state)?;
    shared.api.api_torrent_action_forget(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn torrent_delete(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
) -> CommandResult<()> {
    let shared = shared(&state)?;
    shared.api.api_torrent_action_delete(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn torrent_update_only_files(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
    only_files: Vec<usize>,
) -> CommandResult<()> {
    let shared = shared(&state)?;
    shared
        .api
        .api_torrent_action_update_only_files(id, &only_files.into_iter().collect())
        .await?;
    Ok(())
}

#[tauri::command]
pub fn session_stats(state: State<'_, EngineState>) -> CommandResult<SessionStatsSnapshot> {
    let shared = shared(&state)?;
    Ok(shared.api.api_session_stats())
}

#[tauri::command]
pub fn stream_url(
    state: State<'_, EngineState>,
    id: TorrentIdOrHash,
    file_id: usize,
) -> CommandResult<String> {
    let shared = shared(&state)?;
    Ok(format!(
        "http://127.0.0.1:{}/{}/{}/{}",
        shared.streaming.port,
        shared.streaming.token,
        id,
        file_id
    ))
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn dev_read_test_torrent(name: String) -> CommandResult<String> {
    let dir = std::env::var("STREAMNEST_TEST_MEDIA")
        .unwrap_or_else(|_| "E:\\streamnest\\.testmedia".to_string());
    let path = std::path::Path::new(&dir)
        .join("torrents")
        .join(&name);
    let bytes = std::fs::read(&path).map_err(|e| CommandError::new(format!(
        "cannot read test torrent {}: {e}",
        path.display()
    )))?;
    Ok(general_purpose::STANDARD.encode(bytes))
}

const SUBTITLE_EXTS: [&str; 4] = ["srt", "ass", "vtt", "ssa"];
const SUBTITLE_MAX_BYTES: usize = 2 * 1024 * 1024;

pub fn sanitize_subtitle_name(name: &str) -> Result<String, String> {
    let safe = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "invalid subtitle file name".to_string())?;
    if safe.starts_with('.') {
        return Err("subtitle file name must not be hidden".to_string());
    }
    if safe.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0')) {
        return Err("subtitle file name contains path separators".to_string());
    }
    let ext = std::path::Path::new(safe)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !SUBTITLE_EXTS.contains(&ext.as_str()) {
        return Err(format!(
            "unsupported subtitle type .{ext} (allowed: srt, ass, vtt, ssa)"
        ));
    }
    Ok(safe.to_string())
}

#[tauri::command]
pub fn import_subtitle(
    state: State<'_, EngineState>,
    name: String,
    contents_base64: String,
) -> CommandResult<String> {
    let safe_name = sanitize_subtitle_name(&name).map_err(CommandError::new)?;

    let bytes = general_purpose::STANDARD
        .decode(contents_base64.as_bytes())
        .map_err(|e| CommandError::new(format!("invalid base64 subtitle: {e}")))?;
    if bytes.len() > SUBTITLE_MAX_BYTES {
        return Err(CommandError::new("subtitle file exceeds 2 MiB limit"));
    }

    let shared = shared(&state)?;
    let dir = shared
        .download_dir
        .parent()
        .map(|p| p.join("subtitles"))
        .unwrap_or_else(|| std::path::PathBuf::from("subtitles"));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(safe_name);
    std::fs::write(&path, &bytes)?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::sanitize_subtitle_name;

    #[test]
    fn accepts_plain_srt() {
        assert_eq!(sanitize_subtitle_name("movie.srt").unwrap(), "movie.srt");
        assert_eq!(sanitize_subtitle_name("MOVIE.SRT").unwrap(), "MOVIE.SRT");
    }

    #[test]
    fn strips_traversal_components() {
        let sanitized = sanitize_subtitle_name("../../etc/evil.srt").unwrap();
        assert_eq!(sanitized, "evil.srt");
        assert!(!sanitized.contains(".."));
    }

    #[test]
    fn strips_windows_traversal() {
        let sanitized = sanitize_subtitle_name("..\\..\\windows\\evil.ass").unwrap();
        assert_eq!(sanitized, "evil.ass");
        assert!(!sanitized.contains('\\'));
    }

    #[test]
    fn rejects_hidden_files() {
        assert!(sanitize_subtitle_name(".srt").is_err());
    }

    #[test]
    fn rejects_wrong_extension() {
        assert!(sanitize_subtitle_name("payload.txt").is_err());
        assert!(sanitize_subtitle_name("noext").is_err());
    }
}
