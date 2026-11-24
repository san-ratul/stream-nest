use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use librqbit::{AddTorrent, AddTorrentOptions, api::TorrentIdOrHash};

use crate::{
    commands::parse_peers,
    engine::{EngineShared, EngineState},
    metadata::video::{self, EpisodeFileMatch, VideoFileInfo},
    storage::Db,
};

const METADATA_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// One active playback session. The registry is informational: playback never
/// stops or reconfigures torrents, so user-managed downloads survive playback.
#[derive(Debug, Clone, Serialize)]
pub struct PlaybackSession {
    pub media_key: String,
    pub info_hash: String,
    pub file_id: usize,
    pub source_id: String,
    pub started_at: String,
}

#[derive(Default)]
pub struct PlaybackRegistry {
    sessions: Mutex<HashMap<String, PlaybackSession>>,
}

impl PlaybackRegistry {
    pub fn begin(
        &self,
        media_key: String,
        info_hash: String,
        file_id: usize,
        source_id: String,
    ) {
        self.sessions.lock().insert(
            media_key.clone(),
            PlaybackSession {
                media_key,
                info_hash,
                file_id,
                source_id,
                started_at: now_string(),
            },
        );
    }

    pub fn end(&self, media_key: &str) -> bool {
        self.sessions.lock().remove(media_key).is_some()
    }

    pub fn get(&self, media_key: &str) -> Option<PlaybackSession> {
        self.sessions.lock().get(media_key).cloned()
    }

    pub fn active(&self) -> Vec<PlaybackSession> {
        self.sessions.lock().values().cloned().collect()
    }
}

#[derive(Serialize, Clone)]
pub struct PreparedPlayback {
    pub info_hash: String,
    pub torrent_name: String,
    /// Playable video files (extras/sample/trailers filtered out).
    pub videos: Vec<VideoFileInfo>,
    pub total_files: usize,
    pub filtered_extras: usize,
}

pub enum SourceInput {
    Magnet { uri: String, peers: Vec<String> },
    TorrentBytes { bytes: Vec<u8>, peers: Vec<String> },
}

fn now_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch:{secs}")
}

/// Stage callbacks keep the prepare pipeline free of Tauri types so it stays
/// testable without linking the windowing runtime (test binaries have no
/// application manifest; importing comctl32 v6 APIs breaks their load).
type StageCallback<'a> = &'a (dyn Fn(&str) + Send + Sync);

fn emit_stage(cb: Option<StageCallback<'_>>, stage: &str) {
    if let Some(cb) = cb {
        cb(stage);
    }
}

/// Extracts the btih hash from a magnet URI (hex 40 or base32 32 chars) → hex.
pub fn magnet_info_hash(uri: &str) -> Result<String, String> {
    let uri = uri.trim();
    let rest = uri
        .strip_prefix("magnet:?")
        .ok_or_else(|| "This source is invalid: not a magnet URI".to_string())?;
    let xt = rest
        .split('&')
        .find_map(|p| p.strip_prefix("xt=urn:btih:"))
        .ok_or_else(|| "This source is invalid: missing btih hash".to_string())?;
    let hash = xt.split('&').next().unwrap_or("");
    if hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(hash.to_lowercase());
    }
    if hash.len() == 32 {
        // base32
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let mut bits: u64 = 0;
        let mut nbits: u32 = 0;
        let mut bytes = Vec::new();
        for c in hash.chars() {
            let val = ALPHABET
                .iter()
                .position(|&a| (a as char).eq_ignore_ascii_case(&c))
                .ok_or_else(|| "This source is invalid: bad base32 hash".to_string())?;
            bits = (bits << 5) | val as u64;
            nbits += 5;
            if nbits >= 8 {
                nbits -= 8;
                bytes.push(((bits >> nbits) & 0xFF) as u8);
            }
        }
        return Ok(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
    }
    Err("This source is invalid: unsupported btih encoding".to_string())
}

fn add_options(peer_limit: usize, peers: Option<Vec<std::net::SocketAddr>>) -> AddTorrentOptions {
    AddTorrentOptions {
        overwrite: false,
        initial_peers: peers,
        peer_limit: Some(peer_limit),
        ..Default::default()
    }
}

async fn wait_until_initialized(
    shared: &EngineShared,
    info_hash: &str,
) -> Result<(), String> {
    let id = TorrentIdOrHash::parse(info_hash)
        .map_err(|_| "This source is invalid: bad info hash".to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let stats = shared
            .api
            .api_stats_v1(id.clone())
            .map_err(|e| format!("Could not read torrent state: {e}"))?;
        match stats.state {
            librqbit::TorrentStatsState::Initializing { .. } => {}
            _ => return Ok(()),
        }
        if std::time::Instant::now() >= deadline {
            return Ok(()); // proceed anyway; the restriction attempt will surface its own error
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_metadata(
    shared: &EngineShared,
    info_hash: &str,
    stage_cb: Option<StageCallback<'_>>,
) -> Result<Vec<VideoFileInfo>, String> {
    let id = TorrentIdOrHash::parse(info_hash)
        .map_err(|_| "This source is invalid: bad info hash".to_string())?;
    emit_stage(stage_cb, "metadata");
    let deadline = std::time::Instant::now() + METADATA_TIMEOUT;
    loop {
        let details = shared
            .api
            .api_torrent_details(id.clone())
            .map_err(|e| format!("Could not retrieve source metadata: {e}"))?;
        if let Some(files) = &details.files {
            if !files.is_empty() {
                let videos: Vec<VideoFileInfo> = files
                    .iter()
                    .enumerate()
                    .map(|(i, f)| VideoFileInfo {
                        file_index: i,
                        path: f.components.join("/"),
                        name: f.name.clone(),
                        size_bytes: f.length,
                    })
                    .collect();
                return Ok(videos);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(
                "Could not retrieve source metadata (timeout). No peers may be available right now."
                    .to_string(),
            );
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Adds the torrent (reusing an already-managed one), waits for metadata and
/// classifies files. Newly added torrents restrict downloads to the chosen
/// video + subtitle files; user-managed torrents keep their selection.
async fn prepare_core(
    shared: &Arc<EngineShared>,
    db: &Db,
    input: SourceInput,
    stage_cb: Option<StageCallback<'_>>,
) -> Result<PreparedPlayback, String> {
    emit_stage(stage_cb, "connecting");

    let known_hash = {
        let list = shared.api.api_torrent_list_ext(Default::default());
        list.torrents.iter().map(|t| t.info_hash.clone()).collect::<Vec<_>>()
    };

    let peer_limit = read_connection_limit(db);
    let (add, peers, info_hash) = match input {
        SourceInput::Magnet { uri, peers } => {
            let hash = magnet_info_hash(&uri)?;
            (AddTorrent::Url(uri.into()), peers, hash)
        }
        SourceInput::TorrentBytes { bytes, peers } => {
            (AddTorrent::TorrentFileBytes(bytes.into()), peers, String::new())
        }
    };

    let initial_peers = parse_peers(Some(peers)).map_err(|e| format!("This source is invalid: {e}"))?;
    let res = shared
        .api
        .api_add_torrent(add, Some(add_options(peer_limit, initial_peers)))
        .await
        .map_err(|e| format!("This source could not be added to the torrent engine: {e}"))?;

    let info_hash = if info_hash.is_empty() {
        res.details.info_hash.clone()
    } else {
        info_hash
    };

    // Register in the local DB for the download manager (same as manual add).
    let _ = {
        let conn = db.0.lock();
        conn.execute(
            "INSERT INTO torrents(info_hash, name, state, output_folder) VALUES (?1, ?2, 'initializing', ?3)
             ON CONFLICT(info_hash) DO UPDATE SET name = excluded.name, output_folder = excluded.output_folder",
            rusqlite::params![
                info_hash,
                res.details.name.clone().unwrap_or_else(|| "unknown".to_string()),
                res.output_folder
            ],
        )
    };

    let reused = known_hash.iter().any(|h| *h == info_hash);
    let videos = wait_for_metadata(shared, &info_hash, stage_cb).await?;
    let total_files = videos.len();
    let playable = video::playable_videos(&videos);
    let filtered_extras = total_files.saturating_sub(playable.len());
    if playable.is_empty() {
        return Err("No playable video file found in this source".to_string());
    }

    if !reused {
        // Playback-owned fresh torrent: only fetch the video + subtitle files.
        // Existing user-managed torrents keep their selection untouched.
        // librqbit rejects update_only_files while initializing — wait it out
        // (hash check over not-yet-downloaded files completes almost instantly).
        wait_until_initialized(shared, &info_hash).await?;
        let keep: Vec<usize> = {
            let id = TorrentIdOrHash::parse(&info_hash)
                .map_err(|_| "This source is invalid: bad info hash".to_string())?;
            let details = shared.api.api_torrent_details(id.clone())
                .map_err(|e| format!("Could not read torrent files: {e}"))?;
            let mut keep: Vec<usize> = playable.iter().map(|v| v.file_index).collect();
            if let Some(files) = &details.files {
                for (i, f) in files.iter().enumerate() {
                    let path = f.components.join("/");
                    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
                    if matches!(ext.as_str(), "srt" | "ass" | "vtt" | "ssa") {
                        keep.push(i);
                    }
                }
            }
            keep.sort_unstable();
            keep.dedup();
            keep
        };
        if !keep.is_empty() && keep.len() < total_files {
            let id = TorrentIdOrHash::parse(&info_hash).map_err(|_| "bad info hash".to_string())?;
            let _ = shared
                .api
                .api_torrent_action_update_only_files(id, &keep.into_iter().collect())
                .await;
        }
    }

    let torrent_name = res.details.name.clone().unwrap_or_else(|| info_hash.clone());
    Ok(PreparedPlayback {
        info_hash,
        torrent_name,
        videos: playable,
        total_files,
        filtered_extras,
    })
}

fn read_connection_limit(db: &Db) -> usize {
    let conn = db.0.lock();
    conn.query_row("SELECT value FROM app_settings WHERE key = 'connectionLimit'", [], |r| {
        r.get::<_, String>(0)
    })
    .ok()
    .and_then(|v| v.parse::<usize>().ok())
    .unwrap_or(100)
}

// ---------- Tauri commands ----------

fn engine_shared(state: &State<'_, EngineState>) -> Result<Arc<EngineShared>, String> {
    state
        .shared()
        .ok_or_else(|| "The torrent engine is not ready yet".to_string())
}

#[tauri::command]
pub async fn playback_prepare_uri(
    engine: State<'_, EngineState>,
    db: State<'_, Db>,
    app: AppHandle,
    uri: String,
    peers: Option<Vec<String>>,
) -> Result<PreparedPlayback, String> {
    let shared = engine_shared(&engine)?;
    let stage = move |s: &str| {
        let _ = app.emit("playback:stage", s.to_string());
    };
    prepare_core(
        &shared,
        &db,
        SourceInput::Magnet { uri, peers: peers.unwrap_or_default() },
        Some(&stage),
    )
    .await
}

#[tauri::command]
pub async fn playback_prepare_file(
    engine: State<'_, EngineState>,
    db: State<'_, Db>,
    app: AppHandle,
    contents_base64: String,
    peers: Option<Vec<String>>,
) -> Result<PreparedPlayback, String> {
    use base64::{Engine as _, engine::general_purpose};
    let bytes = general_purpose::STANDARD
        .decode(contents_base64.as_bytes())
        .map_err(|e| format!("This source is invalid: {e}"))?;
    let shared = engine_shared(&engine)?;
    let stage = move |s: &str| {
        let _ = app.emit("playback:stage", s.to_string());
    };
    prepare_core(
        &shared,
        &db,
        SourceInput::TorrentBytes { bytes, peers: peers.unwrap_or_default() },
        Some(&stage),
    )
    .await
}

#[tauri::command]
pub async fn episode_matches(
    engine: State<'_, EngineState>,
    info_hash: String,
    season: u32,
    episode: u32,
) -> Result<Vec<EpisodeFileMatch>, String> {
    let shared = engine_shared(&engine)?;
    let id = TorrentIdOrHash::parse(&info_hash)
        .map_err(|_| "This source is invalid: bad info hash".to_string())?;
    let details = shared
        .api
        .api_torrent_details(id)
        .map_err(|e| format!("Could not read torrent files: {e}"))?;
    let files: Vec<VideoFileInfo> = (details.files.unwrap_or_default())
        .into_iter()
        .enumerate()
        .map(|(i, f)| VideoFileInfo {
            file_index: i,
            path: f.components.join("/"),
            name: f.name,
            size_bytes: f.length,
        })
        .collect();
    Ok(crate::metadata::video::match_episode(&files, season, episode))
}

#[tauri::command]
pub fn playback_begin(
    registry: State<'_, PlaybackRegistry>,
    media_key: String,
    info_hash: String,
    file_id: usize,
    source_id: String,
) -> Result<(), String> {
    registry.begin(media_key, info_hash, file_id, source_id);
    Ok(())
}

#[tauri::command]
pub fn playback_end(registry: State<'_, PlaybackRegistry>, media_key: String) -> bool {
    registry.end(&media_key)
}

#[tauri::command]
pub fn playback_active(registry: State<'_, PlaybackRegistry>) -> Vec<PlaybackSession> {
    registry.active()
}

#[cfg(test)]
mod tests {
    use super::magnet_info_hash;

    #[test]
    fn parses_hex_btih() {
        let uri = "magnet:?xt=urn:btih:08ADA5A7A6183AAE1E09D831DF6748D1056693A4&dn=Sintel";
        assert_eq!(
            magnet_info_hash(uri).unwrap(),
            "08ada5a7a6183aae1e09d831df6748d1056693a4"
        );
    }

    #[test]
    fn parses_base32_btih() {
        // dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c in base32 is 3WBFL3G4PSSV7MF37AJSHWDQMLNR63I4
        let uri = "magnet:?xt=urn:btih:3wbfl3g4pssv7mf37ajshwdqmlnr63i4&dn=bbb";
        let hex = magnet_info_hash(uri).unwrap();
        assert_eq!(hex, "dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c");
    }

    #[test]
    fn rejects_invalid_magnets() {
        assert!(magnet_info_hash("http://example.com").is_err());
        assert!(magnet_info_hash("magnet:?dn=notorrent").is_err());
        assert!(magnet_info_hash("magnet:?xt=urn:btih:short").is_err());
    }

    // ---- Full in-process pipeline test (deterministic, no network) ----

    use std::io::Write as _;

    use super::{prepare_core, PlaybackRegistry, SourceInput};
    use librqbit::{ListenerMode, ListenerOptions, Session, SessionOptions};

    use crate::metadata::video::{match_episode, playable_videos};

    fn write_test_files(dir: &std::path::Path) {
        let show = dir.join("Show.S01E01.mkv");
        let sample = dir.join("Sample");
        std::fs::create_dir_all(&sample).unwrap();
        let sample_file = sample.join("trailer.mkv");
        let cover = dir.join("cover.jpg");

        let mut f = std::fs::File::create(&show).unwrap();
        let pattern: Vec<u8> = (0..255u8).collect();
        for _ in 0..(1024 * 1024 / 255 + 1) {
            f.write_all(&pattern).unwrap();
        }
        std::fs::write(&sample_file, vec![0u8; 64 * 1024]).unwrap();
        std::fs::write(&cover, vec![1u8; 1024]).unwrap();
    }

    #[tokio::test]
    async fn prepare_pipeline_classifies_reuses_and_restricts_selection() {
        let seed_dir = std::env::temp_dir().join(format!("sn-seed-{}", std::process::id()));
        let down_dir = std::env::temp_dir().join(format!("sn-dl-{}", std::process::id()));
        for d in [&seed_dir, &down_dir] {
            let _ = std::fs::remove_dir_all(d);
            std::fs::create_dir_all(d).unwrap();
        }
        write_test_files(&seed_dir);

        // Seeder session: creates the torrent from the directory and serves it.
        let seeder = Session::new_with_opts(
            seed_dir.clone(),
            SessionOptions {
                listen: Some(ListenerOptions {
                    mode: ListenerMode::TcpOnly,
                    listen_addr: (std::net::Ipv4Addr::LOCALHOST, 0).into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .expect("seeder session");
        let (created, _handle) = seeder
            .create_and_serve_torrent(
                &seed_dir,
                librqbit::CreateTorrentOptions {
                    name: Some("Show Season Pack"),
                    ..Default::default()
                },
            )
            .await
            .expect("torrent creation");
        let torrent_bytes = created.as_bytes().expect("torrent bytes").to_vec();

        // App engine (downloader) built exactly like production.
        let shared = crate::engine::build_session(down_dir.clone())
            .await
            .expect("engine session");
        let db = crate::storage::Db::open_in_memory().unwrap();

        // 1) Fresh add: metadata immediate (torrent bytes carry it), classification filters junk.
        let prepared = prepare_core(
        &shared,
        &db,
            SourceInput::TorrentBytes { bytes: torrent_bytes.clone(), peers: vec![] },
            None,
        )
        .await
        .expect("prepare succeeds");
        assert!(!prepared.info_hash.is_empty());
        assert_eq!(prepared.torrent_name, "Show Season Pack");
        assert_eq!(prepared.total_files, 3);
        assert_eq!(prepared.videos.len(), 1, "only the real episode is playable");
        assert_eq!(prepared.videos[0].path, "Show.S01E01.mkv");
        assert_eq!(
            prepared.filtered_extras, 2,
            "one extras trailer + one non-video filtered"
        );

        // Fresh playback-owned torrent: selection restricted to the video only.
        let details = shared
            .api
            .api_torrent_details(librqbit::api::TorrentIdOrHash::parse(&prepared.info_hash).unwrap())
            .expect("details");
        let files = details.files.expect("files after metadata");
        for (i, f) in files.iter().enumerate() {
            let is_episode = f.components.join("/").ends_with("Show.S01E01.mkv");
            let is_sample = f.components.join("/").contains("trailer");
            let is_cover = f.components.join("/").ends_with("cover.jpg");
            let expected = match (is_episode, is_sample, is_cover) {
                (true, false, false) => true,   // the playable episode stays selected
                (false, true, false) => false,  // sample trailer excluded
                (false, false, true) => false,  // non-video excluded
                _ => panic!("unexpected file set at index {i}: {}", f.components.join("/")),
            };
            assert_eq!(
                f.included, expected,
                "file {} ({}) included flag",
                i,
                f.components.join("/")
            );
        }

        // 2) Duplicate add: reuse path must not change the existing selection.
        let prepared2 = prepare_core(
            &shared,
            &db,
            SourceInput::TorrentBytes { bytes: torrent_bytes, peers: vec![] },
            None,
        )
        .await
        .expect("reuse prepare");
        assert_eq!(prepared2.info_hash, prepared.info_hash);
        let details2 = shared
            .api
            .api_torrent_details(librqbit::api::TorrentIdOrHash::parse(&prepared2.info_hash).unwrap())
            .expect("details2");
        let included_before: Vec<bool> = files.iter().map(|f| f.included).collect();
        let included_after: Vec<bool> = details2.files.expect("files2").iter().map(|f| f.included).collect();
        assert_eq!(
            included_before, included_after,
            "reuse must not alter file selection"
        );

        // 3) Episode matching on the playable list.
        let matches = match_episode(&prepared.videos, 1, 1);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].confidence >= 80);
        assert!(!playable_videos(&prepared.videos).is_empty());

        // 4) Playback registry lifecycle.
        let registry = PlaybackRegistry::default();
        registry.begin("tmdb:tv:123:1:1".to_string(), prepared.info_hash.clone(), 0, "test-src".to_string());
        assert_eq!(registry.active().len(), 1);
        assert!(registry.get("tmdb:tv:123:1:1").is_some());
        assert!(registry.end("tmdb:tv:123:1:1"));
        assert!(registry.active().is_empty());
        assert!(!registry.end("tmdb:tv:123:1:1"), "double end is a no-op");

        // Cleanup.
        let _ = shared.api.api_torrent_action_delete(librqbit::api::TorrentIdOrHash::parse(&prepared.info_hash).unwrap()).await;
        let _ = std::fs::remove_dir_all(seed_dir);
        let _ = std::fs::remove_dir_all(down_dir);
    }
}
