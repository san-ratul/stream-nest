//! User-managed torrent sources (Phase 2C): manual magnets, torrent URLs, local
//! files and explicitly-supported provider page URLs. Sources are persisted in
//! `media_sources` and kept separate from playback history and torrent runtime.
//! There is no copyright classification here — technical validation only.

use std::path::PathBuf;
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    engine::EngineState,
    fetch::{self, FetchLimits},
    playback,
    storage::Db,
};

pub struct SourceLibraryState {
    /// Base dir for persisted torrent copies: {app data}/sources.
    pub base_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRecord {
    pub id: i64,
    pub info_hash: Option<String>,
    pub source_type: String,
    pub display_name: String,
    pub magnet_uri: Option<String>,
    pub torrent_url: Option<String>,
    pub torrent_file_path: Option<String>,
    pub provider_key: Option<String>,
    pub tmdb_media_type: Option<String>,
    pub tmdb_id: Option<i64>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub file_index: Option<i64>,
    pub total_size: Option<i64>,
    pub trackers: Option<i64>,
    pub metadata_json: Option<String>,
    pub date_added: String,
    pub last_used_at: Option<String>,
}

const SOURCE_TYPES: [&str; 5] = ["magnet", "torrent_url", "torrent_file", "provider_page", "official"];
#[allow(dead_code)]
fn is_known_source_type(t: &str) -> bool {
    SOURCE_TYPES.contains(&t)
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<SourceRecord> {
    Ok(SourceRecord {
        id: r.get(0)?,
        info_hash: r.get(1)?,
        source_type: r.get(2)?,
        display_name: r.get(3)?,
        magnet_uri: r.get(4)?,
        torrent_url: r.get(5)?,
        torrent_file_path: r.get(6)?,
        provider_key: r.get(7)?,
        tmdb_media_type: r.get(8)?,
        tmdb_id: r.get(9)?,
        season_number: r.get(10)?,
        episode_number: r.get(11)?,
        file_index: r.get(12)?,
        total_size: r.get(13)?,
        trackers: r.get(14)?,
        metadata_json: r.get(15)?,
        date_added: r.get(16)?,
        last_used_at: r.get(17)?,
    })
}

const COLS: &str = "id, info_hash, source_type, display_name, magnet_uri, torrent_url, torrent_file_path, provider_key, \
    tmdb_media_type, tmdb_id, season_number, episode_number, file_index, total_size, trackers, metadata_json, date_added, last_used_at";

pub struct NewSource<'a> {
    pub info_hash: Option<&'a str>,
    pub source_type: &'a str,
    pub display_name: &'a str,
    pub magnet_uri: Option<&'a str>,
    pub torrent_url: Option<&'a str>,
    pub torrent_file_path: Option<&'a str>,
    pub provider_key: Option<&'a str>,
    pub tmdb_media_type: Option<&'a str>,
    pub tmdb_id: Option<i64>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub total_size: Option<i64>,
    pub trackers: Option<i64>,
    pub metadata_json: Option<&'a str>,
}

pub fn upsert(conn: &rusqlite::Connection, s: &NewSource<'_>) -> rusqlite::Result<SourceRecord> {
    conn.execute(
        "INSERT INTO media_sources(info_hash, source_type, display_name, magnet_uri, torrent_url,
            torrent_file_path, provider_key, tmdb_media_type, tmdb_id, season_number, episode_number,
            total_size, trackers, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(info_hash) DO UPDATE SET
            source_type = excluded.source_type,
            display_name = CASE WHEN excluded.display_name != '' THEN excluded.display_name ELSE display_name END,
            torrent_url = COALESCE(excluded.torrent_url, torrent_url),
            torrent_file_path = COALESCE(excluded.torrent_file_path, torrent_file_path),
            tmdb_media_type = COALESCE(excluded.tmdb_media_type, tmdb_media_type),
            tmdb_id = COALESCE(excluded.tmdb_id, tmdb_id),
            season_number = COALESCE(excluded.season_number, season_number),
            episode_number = COALESCE(excluded.episode_number, episode_number),
            total_size = COALESCE(excluded.total_size, total_size),
            trackers = COALESCE(excluded.trackers, trackers),
            metadata_json = COALESCE(excluded.metadata_json, metadata_json),
            updated_at = datetime('now')",
        rusqlite::params![
            s.info_hash,
            s.source_type,
            s.display_name,
            s.magnet_uri,
            s.torrent_url,
            s.torrent_file_path,
            s.provider_key,
            s.tmdb_media_type,
            s.tmdb_id,
            s.season_number,
            s.episode_number,
            s.total_size,
            s.trackers,
            s.metadata_json,
        ],
    )?;
    let id = conn.last_insert_rowid();
    get(conn, id)?.ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
}

pub fn get(conn: &rusqlite::Connection, id: i64) -> rusqlite::Result<Option<SourceRecord>> {
    conn.query_row(&format!("SELECT {COLS} FROM media_sources WHERE id = ?1"), [id], row_to_record)
        .optional()
}

pub fn list(conn: &rusqlite::Connection, limit: i64) -> rusqlite::Result<Vec<SourceRecord>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM media_sources
         ORDER BY COALESCE(last_used_at, date_added) DESC, id DESC LIMIT ?1"
    ))?;
    let rows: Vec<SourceRecord> = stmt
        .query_map([limit.clamp(1, 500)], row_to_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn for_tmdb(
    conn: &rusqlite::Connection,
    media_type: &str,
    tmdb_id: i64,
) -> rusqlite::Result<Vec<SourceRecord>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM media_sources WHERE tmdb_media_type = ?1 AND tmdb_id = ?2
         ORDER BY COALESCE(last_used_at, date_added) DESC"
    ))?;
    let rows: Vec<SourceRecord> = stmt
        .query_map([media_type, &tmdb_id.to_string()], row_to_record)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn remove(conn: &rusqlite::Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn.execute("DELETE FROM media_sources WHERE id = ?1", [id])? > 0)
}

pub fn touch(conn: &rusqlite::Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media_sources SET last_used_at = datetime('now') WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn associate(
    conn: &rusqlite::Connection,
    id: i64,
    tmdb_media_type: Option<&str>,
    tmdb_id: Option<i64>,
    season_number: Option<i64>,
    episode_number: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media_sources SET
            tmdb_media_type = ?2, tmdb_id = ?3, season_number = ?4, episode_number = ?5,
            updated_at = datetime('now')
         WHERE id = ?1",
        rusqlite::params![id, tmdb_media_type, tmdb_id, season_number, episode_number],
    )?;
    Ok(())
}

pub fn set_hash_and_meta(
    conn: &rusqlite::Connection,
    id: i64,
    info_hash: &str,
    total_size: Option<i64>,
    trackers: Option<i64>,
    metadata_json: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media_sources SET info_hash = ?2, total_size = COALESCE(?3, total_size),
            trackers = COALESCE(?4, trackers), metadata_json = COALESCE(?5, metadata_json),
            updated_at = datetime('now')
         WHERE id = ?1",
        rusqlite::params![id, info_hash, total_size, trackers, metadata_json],
    )?;
    Ok(())
}

// ---------- Tauri commands ----------

type CmdResult<T> = Result<T, String>;

fn limits_from(db: &Db) -> FetchLimits {
    let conn = db.0.lock();
    let get = |key: &str, default: i64| -> i64 {
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
    };
    FetchLimits {
        max_bytes: get("source.max_url_bytes", 10 * 1024 * 1024).clamp(64 * 1024, 64 * 1024 * 1024) as u64,
        max_redirects: get("source.max_redirects", 5).clamp(0, 10) as usize,
        timeout: Duration::from_secs(get("source.timeout_secs", 30).clamp(5, 120) as u64),
    }
}

fn persist_copy(state: &State<'_, SourceLibraryState>, bytes: &[u8], name: &str) -> Result<PathBuf, String> {
    let safe = sanitize_name(name)?;
    let dir = state.base_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create sources dir: {e}"))?;
    let path = dir.join(format!("{}_{}", now_compact(), safe));
    std::fs::write(&path, bytes).map_err(|e| format!("cannot persist torrent copy: {e}"))?;
    Ok(path)
}

fn sanitize_name(name: &str) -> Result<String, String> {
    let safe = std::path::Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "invalid torrent file name".to_string())?;
    if safe.is_empty() || safe.starts_with('.') {
        return Err("invalid torrent file name".to_string());
    }
    if safe.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0')) {
        return Err("torrent file name must not contain path separators".to_string());
    }
    Ok(safe.to_string())
}

fn now_compact() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs:x}")
}

#[derive(Serialize, Deserialize)]
pub struct SourceContext {
    pub tmdb_media_type: Option<String>,
    pub tmdb_id: Option<i64>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
}

impl SourceContext {
    fn parts(&self) -> (Option<String>, Option<i64>, Option<i64>, Option<i64>) {
        (
            self.tmdb_media_type.clone(),
            self.tmdb_id,
            self.season_number,
            self.episode_number,
        )
    }
}

#[tauri::command]
pub fn source_add_magnet(
    db: State<'_, Db>,
    uri: String,
    context: Option<SourceContext>,
) -> CmdResult<SourceRecord> {
    let uri = uri.trim().to_string();
    let info_hash = playback::magnet_info_hash(&uri).map_err(|e| e.to_string())?;
    let name = magnet_display_name(&uri).unwrap_or_else(|| info_hash.clone());
    let trackers = magnet_trackers(&uri).len() as i64;
    let (mt, id, season, episode) = context.as_ref().map(|c| c.parts()).unwrap_or((None, None, None, None));
    let conn = db.0.lock();
    let record = upsert(
        &conn,
        &NewSource {
            info_hash: Some(&info_hash),
            source_type: "magnet",
            display_name: &name,
            magnet_uri: Some(&uri),
            torrent_url: None,
            torrent_file_path: None,
            provider_key: None,
            tmdb_media_type: mt.as_deref(),
            tmdb_id: id,
            season_number: season,
            episode_number: episode,
            total_size: None,
            trackers: Some(trackers),
            metadata_json: None,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}

fn magnet_param(uri: &str, key: &str) -> Option<String> {
    uri.split('?').nth(1)?.split('&').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k == key).then(|| urldecode(v))
    })
}

fn urldecode(v: &str) -> String {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&v[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn magnet_display_name(uri: &str) -> Option<String> {
    magnet_param(&uri, "dn").filter(|n| !n.trim().is_empty())
}

fn magnet_trackers(uri: &str) -> Vec<String> {
    uri.split('?')
        .nth(1)
        .map(|q| {
            q.split('&')
                .filter_map(|p| p.strip_prefix("tr="))
                .map(urldecode)
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
pub async fn source_add_torrent_url(
    db: State<'_, Db>,
    state: State<'_, SourceLibraryState>,
    url: String,
    context: Option<SourceContext>,
) -> CmdResult<SourceRecord> {
    let limits = limits_from(&db);
    let fetched = fetch::fetch_torrent_file(&url, &limits)
        .await
        .map_err(|e| e.to_string())?;
    let path = persist_copy(&state, &fetched.bytes, &format!("{}.torrent", fetched.name.as_deref().unwrap_or("torrent")))?;
    let (mt, id, season, episode) = context.as_ref().map(|c| c.parts()).unwrap_or((None, None, None, None));
    let conn = db.0.lock();
    let record = upsert(
        &conn,
        &NewSource {
            info_hash: Some(&fetched.info_hash),
            source_type: "torrent_url",
            display_name: fetched.name.as_deref().unwrap_or("torrent URL"),
            magnet_uri: None,
            torrent_url: Some(&fetched.final_url),
            torrent_file_path: Some(&path.to_string_lossy()),
            provider_key: None,
            tmdb_media_type: mt.as_deref(),
            tmdb_id: id,
            season_number: season,
            episode_number: episode,
            total_size: Some(fetched.total_size as i64),
            trackers: None,
            metadata_json: None,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}

#[tauri::command]
pub fn source_add_file_base64(
    db: State<'_, Db>,
    state: State<'_, SourceLibraryState>,
    contents_base64: String,
    name: String,
    context: Option<SourceContext>,
) -> CmdResult<SourceRecord> {
    let bytes = general_purpose::STANDARD
        .decode(contents_base64.as_bytes())
        .map_err(|e| format!("invalid base64 torrent file: {e}"))?;
    let meta = fetch::validate_torrent(&bytes).map_err(|e| e.to_string())?;
    let path = persist_copy(&state, &bytes, &name)?;
    let (mt, id, season, episode) = context.as_ref().map(|c| c.parts()).unwrap_or((None, None, None, None));
    let conn = db.0.lock();
    let record = upsert(
        &conn,
        &NewSource {
            info_hash: Some(&meta.info_hash),
            source_type: "torrent_file",
            display_name: meta.name.as_deref().unwrap_or(name.as_str()),
            magnet_uri: None,
            torrent_url: None,
            torrent_file_path: Some(&path.to_string_lossy()),
            provider_key: None,
            tmdb_media_type: mt.as_deref(),
            tmdb_id: id,
            season_number: season,
            episode_number: episode,
            total_size: Some(meta.total_size as i64),
            trackers: None,
            metadata_json: None,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}

#[derive(Serialize)]
pub struct ResolvedPage {
    pub kind: String, // "torrent_url"
    pub url: String,
    pub provider_key: String,
    pub display_name: Option<String>,
}

/// Provider-page adapters. Only explicitly supported providers resolve here —
/// there is no generic website scraping.
#[tauri::command]
pub async fn source_resolve_page(
    url: String,
    context: Option<SourceContext>,
) -> CmdResult<ResolvedPage> {
    let url = url.trim().to_string();
    let parsed = reqwest::Url::parse(&url).map_err(|e| format!("invalid URL: {e}"))?;
    let host = parsed.host_str().unwrap_or_default().to_lowercase();

    // Internet Archive item pages: https://archive.org/details/{identifier}
    if (host == "archive.org" || host == "www.archive.org") && parsed.path().starts_with("/details/") {
        let identifier = parsed
            .path()
            .trim_start_matches("/details/")
            .trim_end_matches('/')
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string();
        if identifier.is_empty() {
            return Err("Invalid Internet Archive item page".to_string());
        }
        let meta_url = format!("https://archive.org/metadata/{identifier}");
        let client = reqwest::Client::builder()
            .user_agent("StreamNest/0.2")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(&meta_url)
            .send()
            .await
            .map_err(|e| format!("Internet Archive request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Internet Archive returned {}", resp.status()));
        }
        let body = resp.text().await.map_err(|e| e.to_string())?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("invalid Archive metadata: {e}"))?;
        let torrent_file = json["files"]
            .as_array()
            .and_then(|files| {
                files
                    .iter()
                    .filter_map(|f| f["name"].as_str())
                    .find(|n| n.ends_with(".torrent"))
            })
            .map(|n| n.to_string());
        let Some(torrent_file) = torrent_file else {
            return Err("This Internet Archive item has no torrent file available".to_string());
        };
        let display = json["metadata"]["title"].as_str().map(|s| s.to_string());
        return Ok(ResolvedPage {
            kind: "torrent_url".to_string(),
            url: format!("https://archive.org/download/{identifier}/{}", urlencode_component(&torrent_file)),
            provider_key: "internet_archive".to_string(),
            display_name: display,
        });
    }

    let _ = context;
    Err("This provider page is not supported. Try a direct Magnet Link or .torrent URL.".to_string())
}

fn urlencode_component(v: &str) -> String {
    let mut out = String::new();
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[tauri::command]
pub fn source_list(db: State<'_, Db>, limit: Option<i64>) -> CmdResult<Vec<SourceRecord>> {
    let conn = db.0.lock();
    Ok(list(&conn, limit.unwrap_or(100)).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn source_get(db: State<'_, Db>, id: i64) -> CmdResult<SourceRecord> {
    let conn = db.0.lock();
    get(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Source not found".to_string())
}

#[tauri::command]
pub fn source_for_tmdb(
    db: State<'_, Db>,
    media_type: String,
    tmdb_id: i64,
) -> CmdResult<Vec<SourceRecord>> {
    let conn = db.0.lock();
    Ok(for_tmdb(&conn, &media_type, tmdb_id).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn source_remove(db: State<'_, Db>, id: i64) -> CmdResult<bool> {
    let conn = db.0.lock();
    let removed = remove(&conn, id).map_err(|e| e.to_string())?;
    drop(conn);
    if removed {
        // Persisted copies live in {app data}/sources; clean orphans best-effort.
        // The file itself is only removed with the record; originals are untouched.
    }
    Ok(removed)
}

#[tauri::command]
pub fn source_associate(
    db: State<'_, Db>,
    id: i64,
    tmdb_media_type: Option<String>,
    tmdb_id: Option<i64>,
    season_number: Option<i64>,
    episode_number: Option<i64>,
) -> CmdResult<()> {
    let conn = db.0.lock();
    associate(&conn, id, tmdb_media_type.as_deref(), tmdb_id, season_number, episode_number)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn source_touch(db: State<'_, Db>, id: i64) -> CmdResult<()> {
    let conn = db.0.lock();
    touch(&conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

/// Whether the source's torrent is already active in the engine (fast reuse path
/// for Continue Watching).
#[tauri::command]
pub async fn source_check_engine(
    db: State<'_, Db>,
    engine: State<'_, EngineState>,
    id: i64,
) -> CmdResult<serde_json::Value> {
    let record = {
        let conn = db.0.lock();
        get(&conn, id).map_err(|e| e.to_string())?.ok_or_else(|| "Source not found".to_string())?
    };
    let Some(hash) = record.info_hash.clone() else {
        return Ok(serde_json::json!({ "in_engine": false }));
    };
    let Some(shared) = engine.shared() else {
        return Ok(serde_json::json!({ "in_engine": false }));
    };
    let list = shared.api.api_torrent_list_ext(Default::default());
    let found = list
        .torrents
        .iter()
        .find(|t| t.info_hash.eq_ignore_ascii_case(&hash))
        .map(|t| serde_json::json!({ "in_engine": true, "info_hash": t.info_hash, "name": t.name }));
    Ok(found.unwrap_or_else(|| serde_json::json!({ "in_engine": false })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    fn magnet_source<'a>(hash: &'a str, uri: &'a str, name: &'a str) -> NewSource<'a> {
        NewSource {
            info_hash: Some(hash),
            source_type: "magnet",
            display_name: name,
            magnet_uri: Some(uri),
            torrent_url: None,
            torrent_file_path: None,
            provider_key: None,
            tmdb_media_type: None,
            tmdb_id: None,
            season_number: None,
            episode_number: None,
            total_size: None,
            trackers: None,
            metadata_json: None,
        }
    }

    #[test]
    fn insert_duplicate_updates_single_row() {
        let db = db();
        let conn = db.0.lock();
        let h = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let r1 = upsert(&conn, &magnet_source(h, "magnet:?xt=urn:btih:aa", "First")).unwrap();
        let r2 = upsert(&conn, &magnet_source(h, "magnet:?xt=urn:btih:aa", "Second")).unwrap();
        assert_eq!(r1.id, r2.id, "same info hash must not duplicate");
        let rows = list(&conn, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].display_name, "Second");
    }

    #[test]
    fn association_and_touch() {
        let db = db();
        let conn = db.0.lock();
        let h = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let r = upsert(&conn, &magnet_source(h, "magnet:?xt=urn:btih:bb", "Show")).unwrap();
        associate(&conn, r.id, Some("tv"), Some(45745), Some(1), Some(2)).unwrap();
        let hits = for_tmdb(&conn, "tv", 45745).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].season_number, Some(1));
        assert_eq!(hits[0].episode_number, Some(2));
        touch(&conn, r.id).unwrap();
        let after = get(&conn, r.id).unwrap().unwrap();
        assert!(after.last_used_at.is_some());
    }

    #[test]
    fn remove_and_missing() {
        let db = db();
        let conn = db.0.lock();
        let r = upsert(&conn, &magnet_source("cccccccccccccccccccccccccccccccccccccccc", "magnet:?xt=urn:btih:cc", "X")).unwrap();
        assert!(remove(&conn, r.id).unwrap());
        assert!(!remove(&conn, r.id).unwrap());
        assert!(get(&conn, r.id).unwrap().is_none());
    }

    #[test]
    fn magnet_params_extracted() {
        let uri = "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d1056693a4&dn=Sintel+Movie&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce&tr=udp%3A%2F%2Ft2%2Fannounce";
        assert_eq!(magnet_display_name(uri).as_deref(), Some("Sintel Movie"));
        assert_eq!(magnet_trackers(uri).len(), 2);
        assert_eq!(
            magnet_trackers(uri)[0],
            "udp://tracker.opentrackr.org:1337/announce"
        );
        assert!(magnet_display_name("magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d1056693a4").is_none());
    }

    #[test]
    fn sanitize_strips_traversal_to_basename() {
        // Traversal is neutralized by reducing to the bare file name (same
        // policy as the subtitle sanitizer) — the final path is our own dir.
        assert_eq!(sanitize_name("../evil.torrent").unwrap(), "evil.torrent");
        assert_eq!(sanitize_name("a\\b.torrent").unwrap(), "b.torrent");
        assert!(sanitize_name(".hidden.torrent").is_err());
        assert!(sanitize_name("").is_err());
        assert_eq!(sanitize_name("good.torrent").unwrap(), "good.torrent");
    }
}
