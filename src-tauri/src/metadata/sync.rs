use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use rusqlite::OptionalExtension;

use crate::storage::Db;

use super::repo;
use super::tmdb::{ImageConfig, MediaListItem, TmdbClient, TmdbError};

/// One synchronized home collection: (key, title, media_type, endpoint, sort_order).
/// Endpoints are documented TMDB paths; `discover` filters keep sections meaningful.
pub struct CollectionDef {
    pub key: &'static str,
    pub title: &'static str,
    pub media_type: &'static str,
    pub endpoint: &'static str,
    pub sort_order: i64,
}

pub const COLLECTIONS: &[CollectionDef] = &[
    CollectionDef { key: "trending_movies", title: "Trending Movies", media_type: "movie", endpoint: "trending/movie/day", sort_order: 1 },
    CollectionDef { key: "popular_movies", title: "Popular Movies", media_type: "movie", endpoint: "movie/popular", sort_order: 2 },
    CollectionDef { key: "top_rated_movies", title: "Top Rated Movies", media_type: "movie", endpoint: "movie/top_rated", sort_order: 3 },
    CollectionDef { key: "upcoming_movies", title: "Upcoming Movies", media_type: "movie", endpoint: "movie/upcoming", sort_order: 4 },
    CollectionDef { key: "now_playing_movies", title: "Now Playing", media_type: "movie", endpoint: "movie/now_playing", sort_order: 5 },
    CollectionDef { key: "latest_movies", title: "Latest Movies", media_type: "movie", endpoint: "discover/movie?sort_by=primary_release_date.desc&vote_count.gte=50", sort_order: 6 },
    CollectionDef { key: "documentaries", title: "Documentaries", media_type: "movie", endpoint: "discover/movie?with_genres=99&sort_by=popularity.desc&include_adult=false", sort_order: 7 },
    CollectionDef { key: "trending_tv", title: "Trending TV Shows", media_type: "tv", endpoint: "trending/tv/day", sort_order: 8 },
    CollectionDef { key: "popular_tv", title: "Popular TV Shows", media_type: "tv", endpoint: "tv/popular", sort_order: 9 },
    CollectionDef { key: "top_rated_tv", title: "Top Rated TV Shows", media_type: "tv", endpoint: "tv/top_rated", sort_order: 10 },
    CollectionDef { key: "latest_tv", title: "Latest TV Shows", media_type: "tv", endpoint: "discover/tv?sort_by=first_air_date.desc&vote_count.gte=10", sort_order: 11 },
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionProgress {
    pub key: String,
    pub title: String,
    pub status: String, // pending | running | done | failed | skipped
    pub error: Option<String>,
    pub items: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "phase", content = "detail")]
pub enum SyncPhase {
    Idle,
    Running {
        current: usize,
        total: usize,
        step: String,
        collections: Vec<CollectionProgress>,
    },
    Completed {
        records: i64,
        errors: Vec<String>,
        finished_at: String,
    },
    Failed {
        error: String,
        finished_at: String,
    },
}

pub struct SyncState {
    pub phase: Mutex<SyncPhase>,
    pub cancel: Mutex<bool>,
}

impl SyncState {
    pub fn new() -> Self {
        Self {
            phase: Mutex::new(SyncPhase::Idle),
            cancel: Mutex::new(false),
        }
    }
}

/// Civil date from days since epoch (Howard Hinnant's algorithm); no chrono dependency.
pub fn today_ymd() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86400)
        .unwrap_or(0) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn pages_setting(conn: &rusqlite::Connection) -> i64 {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = 'catalog.sync.pages'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(2)
    .clamp(1, 5)
}

pub struct SyncOutcome {
    pub records: i64,
    pub errors: Vec<String>,
}

/// Runs a full catalog synchronization. Each collection commits in its own
/// transaction, so an interrupted run leaves previously synced collections intact
/// and the next run simply upserts over them (idempotent by TMDB id).
pub async fn run_sync(
    db: Db,
    client: Arc<TmdbClient>,
    image_config: Arc<RwLock<ImageConfig>>,
    state: Arc<SyncState>,
) -> Result<SyncOutcome, TmdbError> {
    let progress: Vec<CollectionProgress> = COLLECTIONS
        .iter()
        .map(|c| CollectionProgress {
            key: c.key.to_string(),
            title: c.title.to_string(),
            status: "pending".to_string(),
            error: None,
            items: 0,
        })
        .collect();
    {
        let mut phase = state.phase.lock();
        *phase = SyncPhase::Running {
            current: 0,
            total: COLLECTIONS.len() + 2, // genres step + image config + collections
            step: "preparing".to_string(),
            collections: progress.clone(),
        };
    }

    let log_id = {
        let conn = db.0.lock();
        repo::sync_log_start(&conn, "full")
    }
    .map_err(|e| TmdbError::Network(format!("database unavailable: {e}")))?;

    let outcome = sync_inner(&db, &client, &image_config, &state, &progress).await;

    let records = outcome.as_ref().map(|o| o.records).unwrap_or(0);
    {
        let conn = db.0.lock();
        let _ = match &outcome {
            Ok(o) => {
                let err = if o.errors.is_empty() {
                    None
                } else {
                    Some(o.errors.join("; "))
                };
                repo::sync_log_finish(&conn, log_id, "completed", o.records, err.as_deref())
            }
            Err(e) => repo::sync_log_finish(&conn, log_id, "failed", records, Some(&e.to_string())),
        };
    }

    match outcome {
        Ok(o) => {
            let phase_done = SyncPhase::Completed {
                records: o.records,
                errors: o.errors.clone(),
                finished_at: now_string(),
            };
            *state.phase.lock() = phase_done;
            Ok(o)
        }
        Err(e) => {
            let phase_failed = SyncPhase::Failed {
                error: e.to_string(),
                finished_at: now_string(),
            };
            *state.phase.lock() = phase_failed;
            Err(e)
        }
    }
}

async fn sync_inner(
    db: &Db,
    client: &Arc<TmdbClient>,
    image_config: &Arc<RwLock<ImageConfig>>,
    state: &Arc<SyncState>,
    progress: &[CollectionProgress],
) -> Result<SyncOutcome, TmdbError> {
    let mut errors: Vec<String> = Vec::new();
    let mut records: i64 = 0;
    let mut step_index: usize = 0;

    // Step 1: image configuration (needed by the local image proxy).
    set_step(state, progress, step_index, "image configuration", None);
    match client.configuration().await {
        Ok(cfg) => *image_config.write() = cfg,
        Err(e) => errors.push(format!("image configuration: {e}")),
    }
    if cancelled(state) {
        mark_cancelled(state, progress, step_index);
        return Ok(SyncOutcome { records, errors: vec!["cancelled by user".to_string()] });
    }

    // Step 2: genres for movies + tv.
    set_step(state, progress, step_index, "genres", None);
    let movies_result = client.movie_genres().await;
    let tv_result = client.tv_genres().await;
    {
        let conn = db.0.lock();
        if let Ok(genres) = &movies_result {
            for g in genres {
                let _ = conn.execute(
                    "INSERT INTO genres(tmdb_id, name, media_type) VALUES (?1, ?2, 'movie')
                     ON CONFLICT(tmdb_id, media_type) DO UPDATE SET name = excluded.name",
                    rusqlite::params![g.id, g.name],
                );
            }
            records += genres.len() as i64;
        }
        if let Ok(genres) = &tv_result {
            for g in genres {
                let _ = conn.execute(
                    "INSERT INTO genres(tmdb_id, name, media_type) VALUES (?1, ?2, 'tv')
                     ON CONFLICT(tmdb_id, media_type) DO UPDATE SET name = excluded.name",
                    rusqlite::params![g.id, g.name],
                );
            }
            records += genres.len() as i64;
        }
    }
    if let Err(e) = movies_result {
        errors.push(format!("movie genres: {e}"));
    }
    if let Err(e) = tv_result {
        errors.push(format!("tv genres: {e}"));
    }
    step_index += 1;
    if cancelled(state) {
        mark_cancelled(state, progress, step_index);
        return Ok(SyncOutcome { records, errors: vec!["cancelled by user".to_string()] });
    }

    let pages = {
        let conn = db.0.lock();
        pages_setting(&conn)
    };
    let today = today_ymd();

    for (idx, def) in COLLECTIONS.iter().enumerate() {
        step_index = idx + 1;
        set_step(state, progress, step_index, def.title, Some(idx));
        set_collection_status(state, idx, "running");

        match sync_collection(db, client, def, pages, &today).await {
            Ok(count) => {
                records += count;
                set_collection_status_items(state, idx, "done", count);
            }
            Err(e) => {
                errors.push(format!("{}: {}", def.title, e));
                set_collection_status(state, idx, "failed");
                set_collection_error(state, idx, e.to_string());
            }
        }

        if cancelled(state) {
            mark_cancelled(state, progress, idx + 1);
            return Ok(SyncOutcome { records, errors: vec!["cancelled by user".to_string()] });
        }
    }

    Ok(SyncOutcome { records, errors })
}

async fn sync_collection(
    db: &Db,
    client: &Arc<TmdbClient>,
    def: &CollectionDef,
    pages: i64,
    today: &str,
) -> Result<i64, TmdbError> {
    let endpoint = def.endpoint.replace("{today}", today);
    let mut items: Vec<(MediaListItem, Vec<i64>)> = Vec::new();
    for page in 1..=pages.max(1) {
        let tmdb_page = client.list_page(&endpoint, page as u32).await?;
        let total_pages = tmdb_page.total_pages;
        let results = tmdb_page.results;
        let on_last_page = (total_pages > 0 && page as i64 >= total_pages) || results.is_empty();
        for item in results {
            if item.adult.unwrap_or(false) {
                continue;
            }
            items.push((item, Vec::new()));
        }
        if on_last_page {
            break;
        }
    }

    // Commit the whole collection atomically: items + collection membership.
    let count: i64;
    {
        let conn = db.0.lock();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| TmdbError::Network(format!("database unavailable: {e}")))?;
        let collection_id = repo::upsert_collection(&tx, def.key, def.title, def.media_type, def.sort_order)?;
        let mut members: Vec<(&str, i64)> = Vec::with_capacity(items.len());
        for (item, _) in &items {
            if members.iter().any(|(_, id)| *id == item.id) {
                continue; // upstream lists can repeat an id across pages
            }
            match def.media_type {
                "movie" => {
                    let row_id = repo::upsert_movie_list_item(&tx, item)?;
                    repo::link_list_item_genres(&tx, row_id, &item.genre_ids)?;
                }
                "tv" => {
                    // TV list items carry only summary data; store as a show row.
                    // Genre links need genre ids -> genres table via genre_ids? tv list items
                    // also carry genre_ids; resolve against tv genres.
                    let row_id = upsert_tv_list_item(&tx, item)?;
                    link_tv_list_item_genres(&tx, row_id, &item.genre_ids)?;
                }
                _ => unreachable!("collection media_type is static"),
            }
            members.push((def.media_type, item.id));
        }
        repo::replace_collection_items(&tx, collection_id, &members)?;
        tx.commit()
            .map_err(|e| TmdbError::Network(format!("commit failed: {e}")))?;
        count = members.len() as i64;
    }
    Ok(count)
}

pub fn upsert_tv_list_item(conn: &rusqlite::Connection, item: &MediaListItem) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO tv_shows(tmdb_id, name, original_name, overview, first_air_date, poster_path,
            backdrop_path, popularity, vote_average, vote_count, original_language)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(tmdb_id) DO UPDATE SET
            name = excluded.name,
            original_name = excluded.original_name,
            overview = excluded.overview,
            first_air_date = excluded.first_air_date,
            poster_path = excluded.poster_path,
            backdrop_path = excluded.backdrop_path,
            popularity = excluded.popularity,
            vote_average = excluded.vote_average,
            vote_count = excluded.vote_count,
            original_language = excluded.original_language,
            updated_at = datetime('now')",
        rusqlite::params![
            item.id,
            item.display_title(),
            item.original_name,
            item.overview,
            item.first_air_date,
            item.poster_path,
            item.backdrop_path,
            item.popularity,
            item.vote_average,
            item.vote_count,
            item.original_language,
        ],
    )?;
    repo::tv_row_id(conn, item.id)
}

pub fn link_tv_list_item_genres(
    conn: &rusqlite::Connection,
    tv_row_id: i64,
    genre_ids: &[i64],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM tv_show_genres WHERE tv_show_id = ?1", [tv_row_id])?;
    for tmdb_gid in genre_ids {
        let gid: Option<i64> = conn
            .query_row(
                "SELECT id FROM genres WHERE tmdb_id = ?1 AND media_type = 'tv'",
                [tmdb_gid],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(gid) = gid {
            conn.execute(
                "INSERT OR IGNORE INTO tv_show_genres(tv_show_id, genre_id) VALUES (?1, ?2)",
                rusqlite::params![tv_row_id, gid],
            )?;
        }
    }
    Ok(())
}

/// On-demand details fetch (movies + tv, including seasons summary).
/// Called when the user opens a details page and details were never synchronized.
pub async fn ensure_details(
    db: &Db,
    client: &Arc<TmdbClient>,
    media_type: &str,
    tmdb_id: i64,
    force: bool,
) -> Result<(), TmdbError> {
    let already = {
        let conn = db.0.lock();
        repo::detail_fetched_public(&conn, media_type, tmdb_id)
    };
    if already && !force {
        return Ok(());
    }
    match media_type {
        "movie" => {
            let details = client.movie_details(tmdb_id).await?;
            let credits_json = serde_json::to_string(&details.credits.clone().unwrap_or_default())
                .unwrap_or_else(|_| "{}".to_string());
            let conn = db.0.lock();
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| TmdbError::Network(format!("database unavailable: {e}")))?;
            repo::upsert_movie_details(&tx, &details)?;
            repo::store_credits(&tx, &format!("tmdb:movie:{tmdb_id}:credits"), &credits_json)?;
            repo::mark_details_fetched(&tx, "movie", tmdb_id)?;
            tx.commit()
                .map_err(|e| TmdbError::Network(format!("commit failed: {e}")))?;
        }
        "tv" => {
            let details = client.tv_details(tmdb_id).await?;
            let credits_json = serde_json::to_string(&details.credits.clone().unwrap_or_default())
                .unwrap_or_else(|_| "{}".to_string());
            let conn = db.0.lock();
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| TmdbError::Network(format!("database unavailable: {e}")))?;
            repo::upsert_tv_details(&tx, &details)?;
            repo::store_credits(&tx, &format!("tmdb:tv:{tmdb_id}:credits"), &credits_json)?;
            repo::mark_details_fetched(&tx, "tv", tmdb_id)?;
            tx.commit()
                .map_err(|e| TmdbError::Network(format!("commit failed: {e}")))?;
        }
        _ => return Err(TmdbError::InvalidResponse(format!("unknown media type {media_type}"))),
    }
    Ok(())
}

/// On-demand season episodes fetch when the user expands a season.
pub async fn ensure_season_episodes(
    db: &Db,
    client: &Arc<TmdbClient>,
    tv_tmdb_id: i64,
    season_number: i64,
) -> Result<(), TmdbError> {
    let already = {
        let conn = db.0.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM tv_episodes e
             JOIN tv_seasons s ON s.id = e.season_id
             JOIN tv_shows t ON t.id = s.tv_show_id
             WHERE t.tmdb_id = ?1 AND s.season_number = ?2",
            rusqlite::params![tv_tmdb_id, season_number],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    };
    if already {
        return Ok(());
    }
    let season = client.season_details(tv_tmdb_id, season_number).await?;
    let conn = db.0.lock();
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| TmdbError::Network(format!("database unavailable: {e}")))?;
    let tv_row_id: i64 = conn
        .query_row("SELECT id FROM tv_shows WHERE tmdb_id = ?1", [tv_tmdb_id], |r| r.get(0))
        .map_err(|e| TmdbError::Network(format!("show missing locally: {e}")))?;
    repo::upsert_season_details(&tx, tv_row_id, &season)?;
    tx.commit()
        .map_err(|e| TmdbError::Network(format!("commit failed: {e}")))?;
    Ok(())
}

fn cancelled(state: &Arc<SyncState>) -> bool {
    *state.cancel.lock()
}

fn mark_cancelled(state: &Arc<SyncState>, progress: &[CollectionProgress], current: usize) {
    // The final sync_inner return path writes the log row; here we only fix UI phase.
    set_step(state, progress, current, "cancelled", None);
}

fn set_step(
    state: &Arc<SyncState>,
    progress: &[CollectionProgress],
    current: usize,
    step: &str,
    collection_idx: Option<usize>,
) {
    let mut phase = state.phase.lock();
    if let SyncPhase::Running { collections, .. } = &mut *phase {
        if collections.len() != progress.len() {
            *collections = progress.to_vec();
        }
        if let Some(idx) = collection_idx {
            if let Some(c) = collections.get_mut(idx) {
                c.status = "running".to_string();
            }
        }
        *phase = SyncPhase::Running {
            current,
            total: COLLECTIONS.len() + 2,
            step: step.to_string(),
            collections: collections.clone(),
        };
    }
}

fn set_collection_status(state: &Arc<SyncState>, idx: usize, status: &str) {
    let mut phase = state.phase.lock();
    if let SyncPhase::Running { collections, .. } = &mut *phase {
        if let Some(c) = collections.get_mut(idx) {
            c.status = status.to_string();
        }
    }
}

fn set_collection_status_items(state: &Arc<SyncState>, idx: usize, status: &str, items: i64) {
    let mut phase = state.phase.lock();
    if let SyncPhase::Running { collections, .. } = &mut *phase {
        if let Some(c) = collections.get_mut(idx) {
            c.status = status.to_string();
            c.items = items;
        }
    }
}

fn set_collection_error(state: &Arc<SyncState>, idx: usize, error: String) {
    let mut phase = state.phase.lock();
    if let SyncPhase::Running { collections, .. } = &mut *phase {
        if let Some(c) = collections.get_mut(idx) {
            c.error = Some(error);
        }
    }
}

fn now_string() -> String {
    // SQLite-format local timestamp approximation via epoch seconds.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch:{secs}")
}
