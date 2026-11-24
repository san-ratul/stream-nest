use std::{path::PathBuf, sync::Arc};

use parking_lot::Mutex;
use rusqlite::Connection;

pub mod commands;

pub struct Db(pub Arc<Mutex<Connection>>);

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS torrents (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  info_hash TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'added',
  output_folder TEXT,
  added_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS torrent_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  torrent_id INTEGER NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  file_index INTEGER NOT NULL,
  path TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  is_video INTEGER NOT NULL DEFAULT 0,
  selected INTEGER NOT NULL DEFAULT 1,
  UNIQUE(torrent_id, file_index)
);
CREATE TABLE IF NOT EXISTS playback_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  media_key TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  position_seconds REAL NOT NULL DEFAULT 0,
  duration_seconds REAL NOT NULL DEFAULT 0,
  last_played_at TEXT NOT NULL DEFAULT (datetime('now')),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_history_media ON playback_history(media_key);
CREATE INDEX IF NOT EXISTS idx_history_last ON playback_history(last_played_at DESC);
CREATE TABLE IF NOT EXISTS library_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  media_key TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL,
  media_type TEXT NOT NULL DEFAULT 'torrent_video',
  path TEXT,
  added_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS favorites (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  media_key TEXT UNIQUE NOT NULL,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_cache (
  source_id TEXT PRIMARY KEY,
  payload TEXT NOT NULL,
  fetched_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS schema_version (
  version INTEGER NOT NULL
);
";

impl Db {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA_V1)?;
        conn.execute("INSERT OR IGNORE INTO schema_version(version) VALUES (1)", [])?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA_V1)?;
        conn.execute("INSERT OR IGNORE INTO schema_version(version) VALUES (1)", [])?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }
}
