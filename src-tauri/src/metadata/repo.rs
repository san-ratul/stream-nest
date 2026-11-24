use rusqlite::{params, Connection, OptionalExtension};

use super::tmdb::{
    EpisodeItem, GenreItem, MediaListItem, MovieDetails, SeasonDetails, SeasonSummary, TvDetails,
};

/// All repository functions take a connection so callers control transaction scope.
/// Multi-record writes run inside transactions opened by the sync engine.

pub fn upsert_movie_list_item(conn: &Connection, item: &MediaListItem) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO movies(tmdb_id, title, original_title, overview, release_date, poster_path,
            backdrop_path, popularity, vote_average, vote_count, original_language, adult)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(tmdb_id) DO UPDATE SET
            title = excluded.title,
            original_title = excluded.original_title,
            overview = excluded.overview,
            release_date = excluded.release_date,
            poster_path = excluded.poster_path,
            backdrop_path = excluded.backdrop_path,
            popularity = excluded.popularity,
            vote_average = excluded.vote_average,
            vote_count = excluded.vote_count,
            original_language = excluded.original_language,
            adult = excluded.adult,
            updated_at = datetime('now')",
        params![
            item.id,
            item.display_title(),
            item.original_title,
            item.overview,
            item.date(),
            item.poster_path,
            item.backdrop_path,
            item.popularity,
            item.vote_average,
            item.vote_count,
            item.original_language,
            item.adult.unwrap_or(false) as i64,
        ],
    )?;
    movie_row_id(conn, item.id)
}

pub fn movie_row_id(conn: &Connection, tmdb_id: i64) -> rusqlite::Result<i64> {
    conn.query_row("SELECT id FROM movies WHERE tmdb_id = ?1", [tmdb_id], |r| r.get(0))
}

pub fn tv_row_id(conn: &Connection, tmdb_id: i64) -> rusqlite::Result<i64> {
    conn.query_row("SELECT id FROM tv_shows WHERE tmdb_id = ?1", [tmdb_id], |r| r.get(0))
}

pub fn upsert_movie_details(conn: &Connection, details: &MovieDetails) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO movies(tmdb_id, title, original_title, overview, release_date, poster_path,
            backdrop_path, popularity, vote_average, vote_count, original_language, adult, runtime_minutes,
            metadata_updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'))
         ON CONFLICT(tmdb_id) DO UPDATE SET
            title = excluded.title,
            original_title = excluded.original_title,
            overview = excluded.overview,
            release_date = excluded.release_date,
            poster_path = excluded.poster_path,
            backdrop_path = excluded.backdrop_path,
            popularity = excluded.popularity,
            vote_average = excluded.vote_average,
            vote_count = excluded.vote_count,
            original_language = excluded.original_language,
            adult = excluded.adult,
            runtime_minutes = excluded.runtime_minutes,
            metadata_updated_at = datetime('now'),
            updated_at = datetime('now')",
        params![
            details.id,
            details.display_title(),
            details.original_title,
            details.overview,
            details.release_date,
            details.poster_path,
            details.backdrop_path,
            details.popularity,
            details.vote_average,
            details.vote_count,
            details.original_language,
            details.adult.unwrap_or(false) as i64,
            details.runtime,
        ],
    )?;
    let id = movie_row_id(conn, details.id)?;
    replace_movie_genres(conn, id, &details.genres)?;
    Ok(id)
}

pub fn upsert_tv_details(conn: &Connection, details: &TvDetails) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO tv_shows(tmdb_id, name, original_name, overview, first_air_date, poster_path,
            backdrop_path, popularity, vote_average, vote_count, original_language, metadata_updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))
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
            metadata_updated_at = datetime('now'),
            updated_at = datetime('now')",
        params![
            details.id,
            details.display_name(),
            details.original_name,
            details.overview,
            details.first_air_date,
            details.poster_path,
            details.backdrop_path,
            details.popularity,
            details.vote_average,
            details.vote_count,
            details.original_language,
        ],
    )?;
    let id = tv_row_id(conn, details.id)?;
    replace_tv_genres(conn, id, &details.genres)?;
    for season in &details.seasons {
        upsert_season(conn, id, season)?;
    }
    Ok(id)
}

pub fn upsert_season(
    conn: &Connection,
    tv_show_id: i64,
    season: &SeasonSummary,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO tv_seasons(tv_show_id, tmdb_id, season_number, name, overview, poster_path, air_date, episode_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(tv_show_id, season_number) DO UPDATE SET
            tmdb_id = COALESCE(excluded.tmdb_id, tmdb_id),
            name = COALESCE(excluded.name, name),
            overview = COALESCE(excluded.overview, overview),
            poster_path = COALESCE(excluded.poster_path, poster_path),
            air_date = COALESCE(excluded.air_date, air_date),
            episode_count = COALESCE(excluded.episode_count, episode_count)",
        params![
            tv_show_id,
            season.id,
            season.season_number,
            season.name,
            season.overview,
            season.poster_path,
            season.air_date,
            season.episode_count,
        ],
    )?;
    conn.query_row(
        "SELECT id FROM tv_seasons WHERE tv_show_id = ?1 AND season_number = ?2",
        params![tv_show_id, season.season_number],
        |r| r.get(0),
    )
}

pub fn upsert_episodes(
    conn: &Connection,
    tv_show_id: i64,
    season_id: i64,
    episodes: &[EpisodeItem],
) -> rusqlite::Result<()> {
    for ep in episodes {
        conn.execute(
            "INSERT INTO tv_episodes(tv_show_id, season_id, tmdb_id, episode_number, season_number, name, overview, air_date, still_path, vote_average)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(season_id, episode_number) DO UPDATE SET
                tmdb_id = COALESCE(excluded.tmdb_id, tmdb_id),
                name = COALESCE(excluded.name, name),
                overview = COALESCE(excluded.overview, overview),
                air_date = COALESCE(excluded.air_date, air_date),
                still_path = COALESCE(excluded.still_path, still_path),
                vote_average = COALESCE(excluded.vote_average, vote_average)",
            params![
                tv_show_id,
                season_id,
                ep.id,
                ep.episode_number,
                ep.season_number,
                ep.name,
                ep.overview,
                ep.air_date,
                ep.still_path,
                ep.vote_average,
            ],
        )?;
    }
    Ok(())
}

pub fn upsert_season_details(
    conn: &Connection,
    tv_show_id: i64,
    details: &SeasonDetails,
) -> rusqlite::Result<i64> {
    let summary = SeasonSummary {
        id: details.id,
        season_number: details.season_number,
        name: details.name.clone(),
        overview: details.overview.clone(),
        poster_path: details.poster_path.clone(),
        air_date: details.air_date.clone(),
        episode_count: Some(details.episodes.len() as i64),
    };
    let season_id = upsert_season(conn, tv_show_id, &summary)?;
    upsert_episodes(conn, tv_show_id, season_id, &details.episodes)?;
    Ok(season_id)
}

fn ensure_genre(conn: &Connection, genre: &GenreItem, media_type: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO genres(tmdb_id, name, media_type) VALUES (?1, ?2, ?3)
         ON CONFLICT(tmdb_id, media_type) DO UPDATE SET name = excluded.name, updated_at = datetime('now')",
        params![genre.id, genre.name, media_type],
    )?;
    // last_insert_rowid() is unreliable on the DO UPDATE path: resolve by natural key.
    conn.query_row(
        "SELECT id FROM genres WHERE tmdb_id = ?1 AND media_type = ?2",
        params![genre.id, media_type],
        |r| r.get(0),
    )
}

pub fn replace_movie_genres(
    conn: &Connection,
    movie_id: i64,
    genres: &[GenreItem],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM movie_genres WHERE movie_id = ?1", [movie_id])?;
    for g in genres {
        let gid = ensure_genre(conn, g, "movie")?;
        conn.execute(
            "INSERT OR IGNORE INTO movie_genres(movie_id, genre_id) VALUES (?1, ?2)",
            params![movie_id, gid],
        )?;
    }
    Ok(())
}

pub fn replace_tv_genres(
    conn: &Connection,
    tv_show_id: i64,
    genres: &[GenreItem],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM tv_show_genres WHERE tv_show_id = ?1", [tv_show_id])?;
    for g in genres {
        let gid = ensure_genre(conn, g, "tv")?;
        conn.execute(
            "INSERT OR IGNORE INTO tv_show_genres(tv_show_id, genre_id) VALUES (?1, ?2)",
            params![tv_show_id, gid],
        )?;
    }
    Ok(())
}

pub fn link_list_item_genres(
    conn: &Connection,
    movie_row_id: i64,
    genre_ids: &[i64],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM movie_genres WHERE movie_id = ?1", [movie_row_id])?;
    for tmdb_gid in genre_ids {
        let gid: Option<i64> = conn
            .query_row(
                "SELECT id FROM genres WHERE tmdb_id = ?1 AND media_type = 'movie'",
                [tmdb_gid],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(gid) = gid {
            conn.execute(
                "INSERT OR IGNORE INTO movie_genres(movie_id, genre_id) VALUES (?1, ?2)",
                params![movie_row_id, gid],
            )?;
        }
    }
    Ok(())
}

pub fn upsert_collection(
    conn: &Connection,
    key: &str,
    title: &str,
    media_type: &str,
    sort_order: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO catalog_collections(collection_key, title, media_type, sort_order)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(collection_key) DO UPDATE SET
            title = excluded.title, media_type = excluded.media_type, sort_order = excluded.sort_order",
        params![key, title, media_type, sort_order],
    )?;
    conn.query_row(
        "SELECT id FROM catalog_collections WHERE collection_key = ?1",
        [key],
        |r| r.get(0),
    )
}

/// Replaces collection contents in one statement scope (caller wraps in a transaction
/// together with the item upserts so an interrupted sync never leaves a half-updated collection).
pub fn replace_collection_items(
    conn: &Connection,
    collection_id: i64,
    items: &[( &str, i64 )], // (media_type, tmdb_id) in display order
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM catalog_collection_items WHERE collection_id = ?1",
        [collection_id],
    )?;
    for (position, (media_type, tmdb_id)) in items.iter().enumerate() {
        conn.execute(
            "INSERT INTO catalog_collection_items(collection_id, media_type, tmdb_id, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![collection_id, media_type, tmdb_id, position as i64],
        )?;
    }
    conn.execute(
        "UPDATE catalog_collections SET last_synced_at = datetime('now') WHERE id = ?1",
        [collection_id],
    )?;
    Ok(())
}

pub fn sync_log_start(conn: &Connection, sync_type: &str) -> rusqlite::Result<i64> {
    // An interrupted previous run leaves a 'running' row; mark it before opening a new one.
    conn.execute(
        "UPDATE sync_logs SET status = 'interrupted',
            error_message = COALESCE(error_message, 'superseded by a newer sync run')
         WHERE status = 'running'",
        [],
    )?;
    conn.execute(
        "INSERT INTO sync_logs(provider, sync_type, status) VALUES ('tmdb', ?1, 'running')",
        [sync_type],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn sync_log_finish(
    conn: &Connection,
    log_id: i64,
    status: &str,
    records_processed: i64,
    error_message: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sync_logs SET status = ?2, completed_at = datetime('now'),
            records_processed = ?3, error_message = ?4 WHERE id = ?1",
        params![log_id, status, records_processed, error_message],
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogCard {
    pub media_type: String,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<String>,
    pub poster_path: Option<String>,
    pub rating: Option<f64>,
    pub overview: Option<String>,
}

fn card_from_row(row: &rusqlite::Row<'_>, media_type: &str) -> rusqlite::Result<CatalogCard> {
    Ok(CatalogCard {
        media_type: media_type.to_string(),
        tmdb_id: row.get(0)?,
        title: row.get(1)?,
        year: row
            .get::<_, Option<String>>(2)?
            .and_then(|d| d.get(..4).map(|s| s.to_string())),
        poster_path: row.get(3)?,
        rating: row.get(4)?,
        overview: row.get(5)?,
    })
}

const MOVIE_CARD_COLS: &str =
    "tmdb_id, title, COALESCE(release_date, ''), poster_path, vote_average, overview";
const TV_CARD_COLS: &str =
    "tmdb_id, name, COALESCE(first_air_date, ''), poster_path, vote_average, overview";

pub fn collection_cards(
    conn: &Connection,
    collection_key: &str,
    limit: i64,
) -> rusqlite::Result<Vec<CatalogCard>> {
    let mut stmt = conn.prepare(
        "SELECT m.tmdb_id, m.title, m.release_date, m.poster_path, m.vote_average, m.overview
         FROM catalog_collection_items i
         JOIN catalog_collections c ON c.id = i.collection_id
         JOIN movies m ON m.tmdb_id = i.tmdb_id
         WHERE c.collection_key = ?1 AND i.media_type = 'movie'
         ORDER BY i.position LIMIT ?2",
    )?;
    let mut cards: Vec<CatalogCard> = stmt
        .query_map(params![collection_key, limit], |r| card_from_row(r, "movie"))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT t.tmdb_id, t.name, t.first_air_date, t.poster_path, t.vote_average, t.overview
         FROM catalog_collection_items i
         JOIN catalog_collections c ON c.id = i.collection_id
         JOIN tv_shows t ON t.tmdb_id = i.tmdb_id
         WHERE c.collection_key = ?1 AND i.media_type = 'tv'
         ORDER BY i.position LIMIT ?2",
    )?;
    let tv_cards = stmt
        .query_map(params![collection_key, limit], |r| card_from_row(r, "tv"))?
        .collect::<Result<Vec<_>, _>>()?;
    cards.extend(tv_cards);
    // Merge movie+tv positions loosely: SQL order per type then interleave is overkill;
    // position order within each type is preserved, movies first (acceptable ordering).
    Ok(cards)
}

pub fn recently_added(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<CatalogCard>> {
    let mut cards: Vec<CatalogCard> = {
        let mut stmt = conn.prepare(&format!(
            "SELECT {MOVIE_CARD_COLS} FROM movies ORDER BY created_at DESC, id DESC LIMIT ?1"
        ))?;
        let rows: Vec<CatalogCard> = stmt
            .query_map([limit], |r| card_from_row(r, "movie"))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {TV_CARD_COLS} FROM tv_shows ORDER BY created_at DESC, id DESC LIMIT ?1"
    ))?;
    let tv: Vec<CatalogCard> = stmt
        .query_map([limit], |r| card_from_row(r, "tv"))?
        .collect::<Result<Vec<_>, _>>()?;
    cards.extend(tv);
    Ok(cards)
}

pub fn search_movies(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<CatalogCard>> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(&format!(
        "SELECT {MOVIE_CARD_COLS} FROM movies
         WHERE title LIKE ?1 ESCAPE '\\' OR COALESCE(original_title, '') LIKE ?1 ESCAPE '\\'
         ORDER BY COALESCE(popularity, 0) DESC LIMIT ?2"
    ))?;
    stmt.query_map(params![pattern, limit], |r| card_from_row(r, "movie"))
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
}

pub fn search_tv(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<CatalogCard>> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(&format!(
        "SELECT {TV_CARD_COLS} FROM tv_shows
         WHERE name LIKE ?1 ESCAPE '\\' OR COALESCE(original_name, '') LIKE ?1 ESCAPE '\\'
         ORDER BY COALESCE(popularity, 0) DESC LIMIT ?2"
    ))?;
    stmt.query_map(params![pattern, limit], |r| card_from_row(r, "tv"))
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
}

pub fn search_documentaries(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<CatalogCard>> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn.prepare(&format!(
        "SELECT m.tmdb_id, m.title, COALESCE(m.release_date, ''), m.poster_path, m.vote_average, m.overview
         FROM movies m
         JOIN movie_genres mg ON mg.movie_id = m.id
         JOIN genres g ON g.id = mg.genre_id AND g.tmdb_id = 99 AND g.media_type = 'movie'
         WHERE (?1 = '%' OR m.title LIKE ?1 ESCAPE '\\')
         ORDER BY COALESCE(m.popularity, 0) DESC LIMIT ?2"
    ))?;
    stmt.query_map(params![pattern, limit], |r| card_from_row(r, "movie"))
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MovieDetail {
    #[serde(flatten)]
    pub card: CatalogCard,
    pub original_title: Option<String>,
    pub release_date: Option<String>,
    pub runtime_minutes: Option<i64>,
    pub vote_count: Option<i64>,
    pub original_language: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<super::tmdb::CastMember>,
    pub crew: Vec<super::tmdb::CrewMember>,
    pub details_fetched: bool,
}

pub fn movie_detail(conn: &Connection, tmdb_id: i64) -> rusqlite::Result<Option<MovieDetail>> {
    let mut stmt = conn.prepare(
        "SELECT tmdb_id, title, COALESCE(release_date,''), poster_path, vote_average, overview,
                original_title, release_date, runtime_minutes, vote_count, original_language,
                backdrop_path
         FROM movies WHERE tmdb_id = ?1",
    )?;
    let Some(detail) = stmt
        .query_row([tmdb_id], |r| {
            let card = card_from_row(r, "movie")?;
            Ok(MovieDetail {
                card,
                original_title: r.get(6)?,
                release_date: r.get(7)?,
                runtime_minutes: r.get(8)?,
                vote_count: r.get(9)?,
                original_language: r.get(10)?,
                backdrop_path: r.get(11)?,
                details_fetched: false,
                genres: Vec::new(),
                cast: Vec::new(),
                crew: Vec::new(),
            })
        })
        .optional()?
    else {
        return Ok(None);
    };
    let mut detail = detail;
    detail.details_fetched = detail_fetched(conn, "movie", tmdb_id)?;

    let mut genre_stmt = conn.prepare(
        "SELECT g.name FROM movie_genres mg JOIN genres g ON g.id = mg.genre_id
         WHERE mg.movie_id = (SELECT id FROM movies WHERE tmdb_id = ?1) ORDER BY g.name",
    )?;
    detail.genres = genre_stmt
        .query_map([tmdb_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    if detail.details_fetched {
        let cast_json: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [format!("tmdb:movie:{tmdb_id}:credits")],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if let Some(json) = cast_json {
            if let Ok(credits) = serde_json::from_str::<super::tmdb::Credits>(&json) {
                detail.cast = credits.cast;
                detail.crew = credits.crew;
            }
        }
    }
    Ok(Some(detail))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SeasonRow {
    pub tmdb_id: i64,
    pub season_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EpisodeRow {
    pub tmdb_id: Option<i64>,
    pub episode_number: i64,
    pub season_number: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub still_path: Option<String>,
    pub vote_average: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TvDetail {
    #[serde(flatten)]
    pub card: CatalogCard,
    pub original_name: Option<String>,
    pub first_air_date: Option<String>,
    pub vote_count: Option<i64>,
    pub original_language: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: Vec<String>,
    pub seasons: Vec<SeasonRow>,
    pub cast: Vec<super::tmdb::CastMember>,
    pub number_of_seasons: Option<i64>,
    pub details_fetched: bool,
}

pub fn tv_detail(conn: &Connection, tmdb_id: i64) -> rusqlite::Result<Option<TvDetail>> {
    let mut stmt = conn.prepare(
        "SELECT tmdb_id, name, COALESCE(first_air_date,''), poster_path, vote_average, overview,
                original_name, first_air_date, vote_count, original_language, backdrop_path
         FROM tv_shows WHERE tmdb_id = ?1",
    )?;
    let Some(detail) = stmt
        .query_row([tmdb_id], |r| {
            let card = card_from_row(r, "tv")?;
            Ok(TvDetail {
                card,
                original_name: r.get(6)?,
                first_air_date: r.get(7)?,
                vote_count: r.get(8)?,
                original_language: r.get(9)?,
                backdrop_path: r.get(10)?,
                details_fetched: false,
                genres: Vec::new(),
                seasons: Vec::new(),
                cast: Vec::new(),
                number_of_seasons: None,
            })
        })
        .optional()?
    else {
        return Ok(None);
    };
    let mut detail = detail;
    detail.details_fetched = detail_fetched(conn, "tv", tmdb_id)?;

    let mut genre_stmt = conn.prepare(
        "SELECT g.name FROM tv_show_genres tg JOIN genres g ON g.id = tg.genre_id
         WHERE tg.tv_show_id = (SELECT id FROM tv_shows WHERE tmdb_id = ?1) ORDER BY g.name",
    )?;
    detail.genres = genre_stmt
        .query_map([tmdb_id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut season_stmt = conn.prepare(
        "SELECT tmdb_id, season_number, name, overview, poster_path, air_date, episode_count
         FROM tv_seasons WHERE tv_show_id = (SELECT id FROM tv_shows WHERE tmdb_id = ?1)
         ORDER BY season_number",
    )?;
    detail.seasons = season_stmt
        .query_map([tmdb_id], |r| {
            Ok(SeasonRow {
                tmdb_id: r.get(0)?,
                season_number: r.get(1)?,
                name: r.get(2)?,
                overview: r.get(3)?,
                poster_path: r.get(4)?,
                air_date: r.get(5)?,
                episode_count: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    detail.number_of_seasons = Some(detail.seasons.len() as i64);

    if detail.details_fetched {
        let credits_json: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                [format!("tmdb:tv:{tmdb_id}:credits")],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if let Some(json) = credits_json {
            if let Ok(credits) = serde_json::from_str::<super::tmdb::Credits>(&json) {
                detail.cast = credits.cast;
            }
        }
    }
    Ok(Some(detail))
}

pub fn season_episodes(
    conn: &Connection,
    tv_tmdb_id: i64,
    season_number: i64,
) -> rusqlite::Result<Vec<EpisodeRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.tmdb_id, e.episode_number, e.season_number, e.name, e.overview, e.air_date,
                e.still_path, e.vote_average
         FROM tv_episodes e
         JOIN tv_seasons s ON s.id = e.season_id
         JOIN tv_shows t ON t.id = s.tv_show_id
         WHERE t.tmdb_id = ?1 AND s.season_number = ?2
         ORDER BY e.episode_number",
    )?;
    let episodes: Vec<EpisodeRow> = stmt
        .query_map(params![tv_tmdb_id, season_number], |r| {
            Ok(EpisodeRow {
                tmdb_id: r.get(0)?,
                episode_number: r.get(1)?,
                season_number: r.get(2)?,
                name: r.get(3)?,
                overview: r.get(4)?,
                air_date: r.get(5)?,
                still_path: r.get(6)?,
                vote_average: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(episodes)
}

pub fn store_credits(conn: &Connection, key: &str, json: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )?;
    Ok(())
}

/// Marks a title whose full details (runtime, genres, credits) have been synchronized.
pub fn mark_details_fetched(conn: &Connection, media_type: &str, tmdb_id: i64) -> rusqlite::Result<()> {
    let key = format!("tmdb:{media_type}:{tmdb_id}:detail");
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES (?1, '1')
         ON CONFLICT(key) DO UPDATE SET value = '1'",
        params![key],
    )?;
    Ok(())
}

fn detail_fetched(conn: &Connection, media_type: &str, tmdb_id: i64) -> rusqlite::Result<bool> {
    let key = format!("tmdb:{media_type}:{tmdb_id}:detail");
    Ok(conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [&key],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .is_some())
}

pub fn detail_fetched_public(conn: &Connection, media_type: &str, tmdb_id: i64) -> bool {
    detail_fetched(conn, media_type, tmdb_id).unwrap_or(false)
}

/// Clears synchronized catalog data + image cache records, never personal data
/// (history/favorites/library/torrents/settings are untouched except TMDB credit blobs).
pub fn clear_catalog(conn: &Connection) -> rusqlite::Result<u64> {
    let mut removed: u64 = 0;
    removed += conn.execute("DELETE FROM catalog_collection_items", [])? as u64;
    removed += conn.execute("DELETE FROM catalog_collections", [])? as u64;
    removed += conn.execute("DELETE FROM tv_episodes", [])? as u64;
    removed += conn.execute("DELETE FROM tv_seasons", [])? as u64;
    removed += conn.execute("DELETE FROM movie_genres", [])? as u64;
    removed += conn.execute("DELETE FROM tv_show_genres", [])? as u64;
    removed += conn.execute("DELETE FROM genres", [])? as u64;
    removed += conn.execute("DELETE FROM movies", [])? as u64;
    removed += conn.execute("DELETE FROM tv_shows", [])? as u64;
    removed += conn.execute("DELETE FROM app_settings WHERE key LIKE 'tmdb:%'", [])? as u64;
    Ok(removed)
}

pub fn sync_status(conn: &Connection) -> rusqlite::Result<SyncStatus> {
    let mut stmt = conn.prepare(
        "SELECT status, started_at, completed_at, records_processed, error_message
         FROM sync_logs ORDER BY id DESC LIMIT 1",
    )?;
    let last = stmt
        .query_row([], |r| {
            Ok(LastSync {
                status: r.get(0)?,
                started_at: r.get(1)?,
                completed_at: r.get(2)?,
                records_processed: r.get(3)?,
                error_message: r.get(4)?,
            })
        })
        .optional()?;
    let mut stmt = conn.prepare(
        "SELECT collection_key, title, media_type, sort_order, last_synced_at
         FROM catalog_collections ORDER BY sort_order",
    )?;
    let collections = stmt
        .query_map([], |r| {
            Ok(CollectionStatus {
                key: r.get(0)?,
                title: r.get(1)?,
                media_type: r.get(2)?,
                sort_order: r.get(3)?,
                last_synced_at: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0))?;
    let tv: i64 = conn.query_row("SELECT COUNT(*) FROM tv_shows", [], |r| r.get(0))?;
    Ok(SyncStatus {
        has_api_key: true, // overwritten by caller when relevant
        movies,
        tv_shows: tv,
        collections,
        last_sync: last,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionStatus {
    pub key: String,
    pub title: String,
    pub media_type: String,
    pub sort_order: i64,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LastSync {
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub records_processed: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    pub has_api_key: bool,
    pub movies: i64,
    pub tv_shows: i64,
    pub collections: Vec<CollectionStatus>,
    pub last_sync: Option<LastSync>,
}
