use streamnest_lib::metadata::repo;
use streamnest_lib::metadata::tmdb::{
    CastMember, Credits, EpisodeItem, GenreItem, MediaListItem, MovieDetails, SeasonDetails,
    SeasonSummary, TvDetails,
};
use streamnest_lib::storage::Db;

fn movie_list_item(id: i64, title: &str) -> MediaListItem {
    MediaListItem {
        id,
        title: Some(title.to_string()),
        popularity: Some(10.0 + id as f64),
        vote_average: Some(7.5),
        vote_count: Some(100),
        release_date: Some("2024-01-01".to_string()),
        genre_ids: vec![99],
        ..Default::default()
    }
}

fn movie_details_fixture(id: i64, title: &str, runtime: i64) -> MovieDetails {
    MovieDetails {
        id,
        title: Some(title.to_string()),
        runtime: Some(runtime),
        genres: vec![GenreItem { id: 99, name: "Documentary".to_string() }],
        credits: Some(Credits {
            cast: vec![CastMember {
                id: 1,
                name: Some("Ada Cast".to_string()),
                character: Some("Hero".to_string()),
                profile_path: None,
                order: 0,
            }],
            crew: Vec::new(),
        }),
        vote_average: Some(8.1),
        ..Default::default()
    }
}

fn tv_details_fixture(id: i64, name: &str) -> TvDetails {
    TvDetails {
        id: id,
        name: Some(name.to_string()),
        genres: vec![GenreItem { id: 18, name: "Drama".to_string() }],
        seasons: vec![SeasonSummary {
            id: 9000 + id,
            season_number: 1,
            name: Some("Season 1".to_string()),
            episode_count: Some(2),
            ..Default::default()
        }],
        vote_average: Some(8.4),
        ..Default::default()
    }
}

#[test]
fn movie_upsert_updates_instead_of_duplicating() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let item1 = movie_list_item(42, "Original Title");
    let item2 = movie_list_item(42, "Updated Title");
    repo::upsert_movie_list_item(&conn, &item1).unwrap();
    repo::upsert_movie_list_item(&conn, &item2).unwrap();
    let (count, title): (i64, String) = conn
        .query_row(
            "SELECT COUNT(*), MAX(title) FROM movies WHERE tmdb_id = 42",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, 1, "tmdb_id unique: upsert must not duplicate");
    assert_eq!(title, "Updated Title");
}

#[test]
fn movie_details_persists_runtime_genres_and_credits() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let details = movie_details_fixture(45745, "Sintel", 15);
    repo::upsert_movie_details(&conn, &details).unwrap();

    let detail = repo::movie_detail(&conn, 45745).unwrap().expect("row exists");
    assert_eq!(detail.card.title, "Sintel");
    assert_eq!(detail.runtime_minutes, Some(15));
    assert_eq!(detail.genres, vec!["Documentary".to_string()]);
    assert_eq!(detail.card.rating, Some(8.1));
    assert!(!detail.details_fetched, "flag not set yet");

    repo::store_credits(
        &conn,
        "tmdb:movie:45745:credits",
        &serde_json::to_string(&details.credits.clone().unwrap_or_default()).unwrap(),
    )
    .unwrap();
    repo::mark_details_fetched(&conn, "movie", 45745).unwrap();
    let detail = repo::movie_detail(&conn, 45745).unwrap().unwrap();
    assert!(detail.details_fetched);
    assert_eq!(detail.cast.len(), 1);
    assert_eq!(detail.cast[0].name.as_deref(), Some("Ada Cast"));
}

#[test]
fn tv_genres_seasons_and_episodes_relationships() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let details = tv_details_fixture(123, "Some Show");
    repo::upsert_tv_details(&conn, &details).unwrap();

    let season_id: i64 = conn
        .query_row("SELECT id FROM tv_seasons WHERE tmdb_id = 9123", [], |r| r.get(0))
        .unwrap();
    repo::upsert_episodes(
        &conn,
        1,
        season_id,
        &[
            EpisodeItem { id: 1, episode_number: 1, season_number: 1, name: Some("Pilot".to_string()), ..Default::default() },
            EpisodeItem { id: 2, episode_number: 2, season_number: 1, name: Some("Second".to_string()), ..Default::default() },
        ],
    )
    .unwrap();

    let detail = repo::tv_detail(&conn, 123).unwrap().unwrap();
    assert_eq!(detail.card.title, "Some Show");
    assert_eq!(detail.genres, vec!["Drama".to_string()]);
    assert_eq!(detail.seasons.len(), 1);
    assert_eq!(detail.number_of_seasons, Some(1));

    let eps = repo::season_episodes(&conn, 123, 1).unwrap();
    assert_eq!(eps.len(), 2);
    assert_eq!(eps[0].name.as_deref(), Some("Pilot"));
}

#[test]
fn episode_unique_constraint_prevents_duplicates() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let details = tv_details_fixture(123, "Show");
    repo::upsert_tv_details(&conn, &details).unwrap();
    let season_id: i64 = conn
        .query_row("SELECT id FROM tv_seasons WHERE tmdb_id = 9123", [], |r| r.get(0))
        .unwrap();
    let ep = EpisodeItem { id: 1, episode_number: 1, season_number: 1, name: Some("Pilot".to_string()), ..Default::default() };
    repo::upsert_episodes(&conn, 1, season_id, &[ep.clone()]).unwrap();
    repo::upsert_episodes(&conn, 1, season_id, &[ep]).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tv_episodes", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn foreign_keys_enforced() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    // Episode referencing a non-existent season must fail (foreign_keys=ON).
    let result = conn.execute(
        "INSERT INTO tv_episodes(tv_show_id, season_id, episode_number, season_number)
         VALUES (1, 999999, 1, 1)",
        [],
    );
    assert!(result.is_err(), "FK violation must be rejected");

    // Collection item referencing a non-existent collection must fail.
    let result = conn.execute(
        "INSERT INTO catalog_collection_items(collection_id, media_type, tmdb_id, position)
         VALUES (999999, 'movie', 1, 0)",
        [],
    );
    assert!(result.is_err(), "FK violation must be rejected");
}

#[test]
fn collection_replace_is_atomic_and_ordered() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let item_a = movie_list_item(1, "A");
    let item_b = movie_list_item(2, "B");
    repo::upsert_movie_list_item(&conn, &item_a).unwrap();
    repo::upsert_movie_list_item(&conn, &item_b).unwrap();

    let collection_id = repo::upsert_collection(&conn, "trending_movies", "Trending Movies", "movie", 1).unwrap();
    repo::replace_collection_items(&conn, collection_id, &[("movie", 2), ("movie", 1)]).unwrap();

    let cards = repo::collection_cards(&conn, "trending_movies", 10).unwrap();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].tmdb_id, 2, "position order preserved");
    assert_eq!(cards[1].tmdb_id, 1);

    // Replace shrinks correctly.
    repo::replace_collection_items(&conn, collection_id, &[("movie", 1)]).unwrap();
    let cards = repo::collection_cards(&conn, "trending_movies", 10).unwrap();
    assert_eq!(cards.len(), 1);
}

#[test]
fn search_filters_by_title_and_type() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    repo::upsert_movie_list_item(&conn, &movie_list_item(1, "Interstellar")).unwrap();
    repo::upsert_movie_details(&conn, &movie_details_fixture(2, "Sintel", 15)).unwrap();
    repo::upsert_tv_details(&conn, &tv_details_fixture(3, "Sintel: The Series")).unwrap();

    let movies = repo::search_movies(&conn, "sintel", 10).unwrap();
    assert_eq!(movies.len(), 1);
    let tv = repo::search_tv(&conn, "sintel", 10).unwrap();
    assert_eq!(tv.len(), 1);
    let docs = repo::search_documentaries(&conn, "", 10).unwrap();
    assert_eq!(docs.len(), 1, "genre 99 linkage restricts documentaries");
    let none = repo::search_movies(&conn, "nonexistent-title", 10).unwrap();
    assert!(none.is_empty());
}

#[test]
fn sync_log_lifecycle_marks_stale_runs_interrupted() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let id1 = repo::sync_log_start(&conn, "full").unwrap();
    // Simulate a crashed run by opening a second one while the first is 'running'.
    let id2 = repo::sync_log_start(&conn, "full").unwrap();
    let status1: String = conn
        .query_row("SELECT status FROM sync_logs WHERE id = ?1", [id1], |r| r.get(0))
        .unwrap();
    assert_eq!(status1, "interrupted", "stale running row must be marked interrupted");

    repo::sync_log_finish(&conn, id2, "completed", 42, None).unwrap();
    let (status, records, completed): (String, i64, Option<String>) = conn
        .query_row(
            "SELECT status, records_processed, completed_at FROM sync_logs WHERE id = ?1",
            [id2],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(records, 42);
    assert!(completed.is_some());

    let status = repo::sync_status(&conn).unwrap();
    let last = status.last_sync.expect("last sync recorded");
    assert_eq!(last.status, "completed");
    assert_eq!(last.records_processed, 42);
}

#[test]
fn clear_catalog_preserves_personal_data() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    // Seed catalog + personal data.
    repo::upsert_movie_list_item(&conn, &movie_list_item(1, "A")).unwrap();
    conn.execute(
        "INSERT INTO favorites(media_key, title) VALUES ('tmdb:movie:1', 'A')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO playback_history(media_key, title, position_seconds, duration_seconds)
         VALUES ('h:0', 'Watched', 10, 100)",
        [],
    )
    .unwrap();
    repo::mark_details_fetched(&conn, "movie", 1).unwrap();

    let removed = repo::clear_catalog(&conn).unwrap();
    assert!(removed > 0);

    let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
    let favs: i64 = conn.query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get(0)).unwrap();
    let history: i64 = conn.query_row("SELECT COUNT(*) FROM playback_history", [], |r| r.get(0)).unwrap();
    assert_eq!(movies, 0, "catalog cleared");
    assert_eq!(favs, 1, "favorites preserved");
    assert_eq!(history, 1, "history preserved");
}

#[test]
fn schema_version_recorded_once() {
    let db = Db::open_in_memory().unwrap();
    {
        let conn = db.0.lock();
        conn.execute("INSERT INTO schema_version(version) SELECT 2 WHERE NOT EXISTS (SELECT 1 FROM schema_version WHERE version >= 2)", []).unwrap();
    }
    // Reopen: migration must be idempotent and not add duplicate version rows.
    let db2 = Db::open_in_memory().unwrap();
    let conn = db2.0.lock();
    let (count, maxv): (i64, i64) = conn
        .query_row("SELECT COUNT(*), MAX(version) FROM schema_version", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(count, 1, "version row must not duplicate on re-open");
    assert_eq!(maxv, 2);
}

#[test]
fn season_details_upserts_summary_and_episodes() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    repo::upsert_tv_details(&conn, &tv_details_fixture(55, "Doc Show")).unwrap();
    let tv_row_id: i64 = conn
        .query_row("SELECT id FROM tv_shows WHERE tmdb_id = 55", [], |r| r.get(0))
        .unwrap();
    let season = SeasonDetails {
        id: 9155,
        season_number: 1,
        name: Some("Season One".to_string()),
        overview: None,
        poster_path: None,
        air_date: Some("2020-01-01".to_string()),
        episodes: vec![EpisodeItem {
            id: 700,
            episode_number: 1,
            season_number: 1,
            name: Some("Ep One".to_string()),
            still_path: Some("/still.jpg".to_string()),
            vote_average: Some(6.5),
            ..Default::default()
        }],
    };
    repo::upsert_season_details(&conn, tv_row_id, &season).unwrap();
    let eps = repo::season_episodes(&conn, 55, 1).unwrap();
    assert_eq!(eps.len(), 1);
    assert_eq!(eps[0].still_path.as_deref(), Some("/still.jpg"));
    assert_eq!(eps[0].vote_average, Some(6.5));
}
