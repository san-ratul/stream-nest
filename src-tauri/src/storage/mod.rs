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

/// Phase 2 discovery schema: TMDB-synchronized catalog tables.
/// Forward-only migration; existing MVP tables are untouched.
/// Reverse (documented, not executed): DROP the tables below in reverse order.
const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS movies (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tmdb_id INTEGER NOT NULL UNIQUE,
  title TEXT NOT NULL,
  original_title TEXT,
  overview TEXT,
  release_date TEXT,
  poster_path TEXT,
  backdrop_path TEXT,
  popularity REAL,
  vote_average REAL,
  vote_count INTEGER,
  original_language TEXT,
  adult INTEGER NOT NULL DEFAULT 0,
  runtime_minutes INTEGER,
  metadata_updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_movies_title ON movies(title);
CREATE INDEX IF NOT EXISTS idx_movies_popularity ON movies(popularity DESC);

CREATE TABLE IF NOT EXISTS tv_shows (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tmdb_id INTEGER NOT NULL UNIQUE,
  name TEXT NOT NULL,
  original_name TEXT,
  overview TEXT,
  first_air_date TEXT,
  poster_path TEXT,
  backdrop_path TEXT,
  popularity REAL,
  vote_average REAL,
  vote_count INTEGER,
  original_language TEXT,
  metadata_updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_tv_name ON tv_shows(name);
CREATE INDEX IF NOT EXISTS idx_tv_popularity ON tv_shows(popularity DESC);

CREATE TABLE IF NOT EXISTS genres (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tmdb_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'tv')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(tmdb_id, media_type)
);

CREATE TABLE IF NOT EXISTS movie_genres (
  movie_id INTEGER NOT NULL REFERENCES movies(id) ON DELETE CASCADE,
  genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  PRIMARY KEY (movie_id, genre_id)
);

CREATE TABLE IF NOT EXISTS tv_show_genres (
  tv_show_id INTEGER NOT NULL REFERENCES tv_shows(id) ON DELETE CASCADE,
  genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
  PRIMARY KEY (tv_show_id, genre_id)
);

CREATE TABLE IF NOT EXISTS tv_seasons (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tv_show_id INTEGER NOT NULL REFERENCES tv_shows(id) ON DELETE CASCADE,
  tmdb_id INTEGER UNIQUE,
  season_number INTEGER NOT NULL,
  name TEXT,
  overview TEXT,
  poster_path TEXT,
  air_date TEXT,
  episode_count INTEGER,
  UNIQUE(tv_show_id, season_number)
);

CREATE TABLE IF NOT EXISTS tv_episodes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tv_show_id INTEGER NOT NULL REFERENCES tv_shows(id) ON DELETE CASCADE,
  season_id INTEGER NOT NULL REFERENCES tv_seasons(id) ON DELETE CASCADE,
  tmdb_id INTEGER,
  episode_number INTEGER NOT NULL,
  season_number INTEGER NOT NULL,
  name TEXT,
  overview TEXT,
  air_date TEXT,
  still_path TEXT,
  vote_average REAL,
  UNIQUE(season_id, episode_number)
);
CREATE INDEX IF NOT EXISTS idx_episodes_season ON tv_episodes(season_id);

CREATE TABLE IF NOT EXISTS catalog_collections (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_key TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'tv', 'mixed')),
  sort_order INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT
);

CREATE TABLE IF NOT EXISTS catalog_collection_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL REFERENCES catalog_collections(id) ON DELETE CASCADE,
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'tv')),
  tmdb_id INTEGER NOT NULL,
  position INTEGER NOT NULL,
  UNIQUE(collection_id, tmdb_id)
);
CREATE INDEX IF NOT EXISTS idx_collection_items_pos ON catalog_collection_items(collection_id, position);

CREATE TABLE IF NOT EXISTS sync_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  provider TEXT NOT NULL DEFAULT 'tmdb',
  sync_type TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'interrupted')),
  started_at TEXT NOT NULL DEFAULT (datetime('now')),
  completed_at TEXT,
  records_processed INTEGER NOT NULL DEFAULT 0,
  error_message TEXT
);
";

const CURRENT_SCHEMA_VERSION: i64 = 2;

impl Db {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self::init(conn).map_err(anyhow::Error::new)?;
        Ok(db)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, rusqlite::Error> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA_V1)?;
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE((SELECT MAX(version) FROM schema_version), 0)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if version < 2 {
            conn.execute_batch(SCHEMA_V2)?;
        }
        conn.execute(
            "INSERT INTO schema_version(version)
             SELECT ?1 WHERE NOT EXISTS (SELECT 1 FROM schema_version WHERE version >= ?1)",
            [CURRENT_SCHEMA_VERSION],
        )?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }
}
