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
    uri: String,
    peers: Option<Vec<String>>,
) -> CommandResult<ApiAddTorrentResponse> {
    let initial_peers =
        parse_peers(peers).map_err(CommandError::new)?;
    let opts = AddTorrentOptions {
        overwrite: false,
        initial_peers,
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
