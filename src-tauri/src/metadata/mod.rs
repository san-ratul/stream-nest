pub mod config;
pub mod images;
pub mod repo;
pub mod sources;
pub mod sync;
pub mod tmdb;

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use tauri::State;

use crate::storage::Db;
use repo::CatalogCard;
use sources::{ContentSourceProvider, AuthorizedSource};
use sync::{SyncPhase, SyncState};
use tmdb::{ImageConfig, TmdbClient, TmdbError};

pub struct TmdbState {
    pub client: Option<Arc<TmdbClient>>,
    pub image_config: Arc<RwLock<ImageConfig>>,
    pub images: RwLock<Option<images::ImageServerInfo>>,
    pub sync: Arc<SyncState>,
    pub sources: Box<dyn ContentSourceProvider + Send + Sync>,
    pub data_dir: PathBuf,
}

#[derive(Serialize)]
pub struct CommandErrorMeta {
    pub message: String,
}

impl From<TmdbError> for CommandErrorMeta {
    fn from(e: TmdbError) -> Self {
        Self { message: e.to_string() }
    }
}

impl From<rusqlite::Error> for CommandErrorMeta {
    fn from(e: rusqlite::Error) -> Self {
        Self { message: e.to_string() }
    }
}

impl From<std::io::Error> for CommandErrorMeta {
    fn from(e: std::io::Error) -> Self {
        Self { message: e.to_string() }
    }
}

pub type MetaResult<T> = Result<T, CommandErrorMeta>;

/// Builds the local proxy URL for a TMDB image path.
#[tauri::command]
pub fn catalog_image_url(
    state: State<'_, TmdbState>,
    path: Option<String>,
    kind: Option<String>,
    size: Option<String>,
) -> Option<String> {
    let path = path?;
    if path.is_empty() {
        return None;
    }
    let images = state.images.read();
    let info = images.as_ref()?;
    let kind = kind.as_deref().unwrap_or("poster");
    let size = size.as_deref().unwrap_or("w342");
    images::local_url(info, kind, size, &path)
}

#[derive(Serialize)]
pub struct HomeSection {
    pub key: String,
    pub title: String,
    pub sort_order: i64,
    pub items: Vec<CatalogCard>,
}

#[tauri::command]
pub fn catalog_home(db: State<'_, Db>, per_section: Option<i64>) -> MetaResult<Vec<HomeSection>> {
    let conn = db.0.lock();
    let per = per_section.unwrap_or(20).clamp(4, 50);
    let mut sections = Vec::new();
    for def in sync::COLLECTIONS {
        let items = repo::collection_cards(&conn, def.key, per)?;
        if items.is_empty() {
            continue; // never render empty/misleading sections
        }
        sections.push(HomeSection {
            key: def.key.to_string(),
            title: def.title.to_string(),
            sort_order: def.sort_order,
            items,
        });
    }
    let recent = repo::recently_added(&conn, per)?;
    if !recent.is_empty() {
        sections.push(HomeSection {
            key: "recently_added".to_string(),
            title: "Recently Added to Catalog".to_string(),
            sort_order: 99,
            items: recent,
        });
    }
    Ok(sections)
}

#[derive(Serialize)]
pub struct SearchResults {
    pub movies: Vec<CatalogCard>,
    pub tv: Vec<CatalogCard>,
    pub documentaries: Vec<CatalogCard>,
    pub legal_entries: Vec<crate::catalog::provider::CatalogEntry>,
    pub source_note: String,
}

/// Local-first search over the synchronized SQLite catalog, plus the
/// rights-verified bundled legal catalog.
#[tauri::command]
pub fn catalog_search(
    db: State<'_, Db>,
    catalog: State<'_, crate::catalog::CatalogState>,
    query: String,
    filter: Option<String>,
    limit: Option<i64>,
) -> MetaResult<SearchResults> {
    let conn = db.0.lock();
    let limit = limit.unwrap_or(30).clamp(1, 100);
    let q = query.trim().to_string();
    let filter = filter.as_deref().unwrap_or("all");

    let mut result = SearchResults {
        movies: Vec::new(),
        tv: Vec::new(),
        documentaries: Vec::new(),
        legal_entries: Vec::new(),
        source_note: catalog.provider.source_note().to_string(),
    };
    if q.is_empty() {
        return Ok(result);
    }
    let matches = |f: &str| f == "all" || f == filter;
    if matches("movies") {
        result.movies = repo::search_movies(&conn, &q, limit)?;
    }
    if matches("tv") {
        result.tv = repo::search_tv(&conn, &q, limit)?;
    }
    if matches("documentaries") {
        result.documentaries = repo::search_documentaries(&conn, &q, limit)?;
    }
    if matches("legal") || filter == "all" {
        result.legal_entries = catalog.provider.list(Some(&q), None);
    }
    Ok(result)
}

#[derive(Serialize)]
pub struct MovieDetailsResponse {
    pub detail: repo::MovieDetail,
    pub sources: Vec<AuthorizedSource>,
    pub metadata_provider: String,
}

#[tauri::command]
pub async fn movie_details(
    db: State<'_, Db>,
    tmdb: State<'_, TmdbState>,
    tmdb_id: i64,
    refresh: Option<bool>,
) -> MetaResult<MovieDetailsResponse> {
    let details = ensure_movie_details(&db, &tmdb, tmdb_id, refresh.unwrap_or(false)).await?;
    let sources = tmdb.sources.sources_for("movie", tmdb_id);
    Ok(MovieDetailsResponse {
        detail: details,
        sources,
        metadata_provider: "TMDB".to_string(),
    })
}

#[derive(Serialize)]
pub struct TvDetailsResponse {
    pub detail: repo::TvDetail,
    pub sources: Vec<AuthorizedSource>,
    pub metadata_provider: String,
}

#[tauri::command]
pub async fn tv_details(
    db: State<'_, Db>,
    tmdb: State<'_, TmdbState>,
    tmdb_id: i64,
    refresh: Option<bool>,
) -> MetaResult<TvDetailsResponse> {
    let details = ensure_tv_details(&db, &tmdb, tmdb_id, refresh.unwrap_or(false)).await?;
    let sources = tmdb.sources.sources_for("tv", tmdb_id);
    Ok(TvDetailsResponse {
        detail: details,
        sources,
        metadata_provider: "TMDB".to_string(),
    })
}

#[tauri::command]
pub async fn season_episodes(
    db: State<'_, Db>,
    tmdb: State<'_, TmdbState>,
    tmdb_id: i64,
    season_number: i64,
) -> MetaResult<Vec<repo::EpisodeRow>> {
    if let Some(client) = tmdb.client.clone() {
        sync::ensure_season_episodes(&db, &client, tmdb_id, season_number)
            .await
            .map_err(CommandErrorMeta::from)?;
    }
    let conn = db.0.lock();
    Ok(repo::season_episodes(&conn, tmdb_id, season_number)?)
}

async fn ensure_movie_details(
    db: &State<'_, Db>,
    tmdb: &State<'_, TmdbState>,
    tmdb_id: i64,
    force: bool,
) -> MetaResult<repo::MovieDetail> {
    if let Some(client) = tmdb.client.clone() {
        sync::ensure_details(db, &client, "movie", tmdb_id, force)
            .await
            .map_err(CommandErrorMeta::from)?;
    }
    let conn = db.0.lock();
    repo::movie_detail(&conn, tmdb_id)?
        .map(Ok)
        .unwrap_or_else(|| {
            Err(CommandErrorMeta {
                message: if tmdb.client.is_none() {
                    "TMDB is not configured: add your API key to .env (TMDB_API_KEY=...) and restart."
                        .to_string()
                } else {
                    format!("movie {tmdb_id} is not in the local catalog; run a catalog sync first")
                },
            })
        })
}

async fn ensure_tv_details(
    db: &State<'_, Db>,
    tmdb: &State<'_, TmdbState>,
    tmdb_id: i64,
    force: bool,
) -> MetaResult<repo::TvDetail> {
    if let Some(client) = tmdb.client.clone() {
        sync::ensure_details(db, &client, "tv", tmdb_id, force)
            .await
            .map_err(CommandErrorMeta::from)?;
    }
    let conn = db.0.lock();
    repo::tv_detail(&conn, tmdb_id)?
        .map(Ok)
        .unwrap_or_else(|| {
            Err(CommandErrorMeta {
                message: if tmdb.client.is_none() {
                    "TMDB is not configured: add your API key to .env (TMDB_API_KEY=...) and restart."
                        .to_string()
                } else {
                    format!("show {tmdb_id} is not in the local catalog; run a catalog sync first")
                },
            })
        })
}

#[derive(Serialize)]
pub struct CatalogStatusResponse {
    pub has_api_key: bool,
    pub db: repo::SyncStatus,
    pub phase: SyncPhase,
    pub provider_note: String,
}

#[tauri::command]
pub fn catalog_status(
    db: State<'_, Db>,
    tmdb: State<'_, TmdbState>,
) -> MetaResult<CatalogStatusResponse> {
    let conn = db.0.lock();
    let mut status = repo::sync_status(&conn)?;
    status.has_api_key = tmdb.client.is_some();
    let phase = tmdb.sync.phase.lock().clone();
    Ok(CatalogStatusResponse {
        has_api_key: status.has_api_key,
        db: status,
        phase,
        provider_note: tmdb.sources.note().to_string(),
    })
}

/// Starts a full synchronization if none is running. Returns immediately;
/// poll catalog_status for progress.
#[tauri::command]
pub fn catalog_sync_start(
    db: State<'_, Db>,
    tmdb: State<'_, TmdbState>,
) -> MetaResult<SyncPhase> {
    let Some(client) = tmdb.client.clone() else {
        return Err(CommandErrorMeta {
            message: "TMDB is not configured: add TMDB_API_KEY to .env and restart the app"
                .to_string(),
        });
    };
    {
        let phase = tmdb.sync.phase.lock();
        if matches!(&*phase, SyncPhase::Running { .. }) {
            return Ok(phase.clone());
        }
    }
    let sync_state = tmdb.sync.clone();
    let image_config = tmdb.image_config.clone();
    let db_clone = Db(db.0.clone());
    *sync_state.cancel.lock() = false;
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync::run_sync(db_clone, client, image_config, sync_state).await {
            log::warn!("catalog sync failed: {e}");
        }
    });
    Ok(tmdb.sync.phase.lock().clone())
}

#[tauri::command]
pub fn catalog_sync_cancel(tmdb: State<'_, TmdbState>) -> MetaResult<()> {
    *tmdb.sync.cancel.lock() = true;
    Ok(())
}

/// Clears synchronized catalog data and the image cache.
/// Personal data (history, favorites, library, torrents, settings) is preserved.
#[tauri::command]
pub fn catalog_clear(db: State<'_, Db>, tmdb: State<'_, TmdbState>) -> MetaResult<u64> {
    {
        let phase = tmdb.sync.phase.lock();
        if matches!(&*phase, SyncPhase::Running { .. }) {
            return Err(CommandErrorMeta {
                message: "cannot clear the catalog while a synchronization is running".to_string(),
            });
        }
    }
    let conn = db.0.lock();
    let removed = repo::clear_catalog(&conn)? as u64;
    drop(conn);
    let images_dir = tmdb.data_dir.join("images");
    let mut deleted = 0u64;
    if let Ok(entries) = std::fs::read_dir(&images_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() && std::fs::remove_file(entry.path()).is_ok() {
                deleted += 1;
            }
        }
    }
    // Reset phase so the UI reflects the empty catalog.
    *tmdb.sync.phase.lock() = SyncPhase::Idle;
    Ok(removed + deleted)
}

#[tauri::command]
pub fn catalog_set_pages(db: State<'_, Db>, pages: i64) -> MetaResult<()> {
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES ('catalog.sync.pages', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [pages.clamp(1, 5).to_string()],
    )?;
    Ok(())
}

/// Convenience for tests and wiring.
pub fn build_client(api_key: String) -> anyhow::Result<Arc<TmdbClient>> {
    let fetcher = tmdb::ReqwestFetcher::new(std::time::Duration::from_secs(15))?;
    Ok(Arc::new(TmdbClient::new(Arc::new(fetcher), api_key)))
}

pub type SharedSyncState = Arc<Mutex<SyncPhase>>;
