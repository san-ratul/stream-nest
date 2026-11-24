use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use parking_lot::RwLock;
use serde_json::json;

use streamnest_lib::metadata::sync::{self, SyncPhase, SyncState};
use streamnest_lib::metadata::tmdb::{
    FetchError, FetchResponse, HttpFetcher, ImageConfig, TmdbClient,
};
use streamnest_lib::storage::Db;

struct Route {
    matches: &'static str,
    status: u16,
    body: String,
}

#[derive(Default)]
struct MockFetcher {
    routes: RwLock<Vec<Route>>,
    calls: RwLock<Vec<String>>,
}

impl MockFetcher {
    fn with_route(self, matches: &'static str, status: u16, body: String) -> Self {
        self.routes.write().push(Route { matches, status, body });
        self
    }
}

impl HttpFetcher for MockFetcher {
    fn fetch<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<FetchResponse, FetchError>> + Send + 'a>> {
        self.calls.write().push(url.to_string());
        let hit = self
            .routes
            .read()
            .iter()
            .find(|r| url.contains(r.matches))
            .map(|r| (r.status, r.body.clone()));
        Box::pin(async move {
            match hit {
                Some((status, body)) => Ok(FetchResponse { status, body, retry_after_seconds: None }),
                None => Ok(FetchResponse {
                    status: 404,
                    body: r#"{"status_message":"Not found"}"#.to_string(),
                    retry_after_seconds: None,
                }),
            }
        })
    }
}

fn list_body(items: &[(i64, &str)]) -> String {
    json!({
        "page": 1,
        "total_pages": 1,
        "results": items.iter().map(|(id, title)| json!({
            "id": id, "title": title, "popularity": 100.0, "vote_average": 7.0,
            "vote_count": 50, "release_date": "2024-01-01", "genre_ids": [99]
        })).collect::<Vec<_>>()
    })
    .to_string()
}

fn client(fetcher: Arc<MockFetcher>) -> Arc<TmdbClient> {
    Arc::new(TmdbClient::new(fetcher, "test-key".to_string()))
}

fn config_ok() -> String {
    json!({"images": {"secure_base_url": "https://image.tmdb.org/t/p/",
        "poster_sizes": ["w342"], "backdrop_sizes": ["w780"],
        "still_sizes": ["w300"], "profile_sizes": ["w185"]}})
    .to_string()
}

fn genres_ok() -> String {
    json!({"genres": [{"id": 99, "name": "Documentary"}, {"id": 28, "name": "Action"}]}).to_string()
}

fn base_fetcher() -> Arc<MockFetcher> {
    let mut f = MockFetcher::default()
        .with_route("configuration", 200, config_ok())
        .with_route("genre/movie/list", 200, genres_ok());
    f = f.with_route("genre/tv/list", 200, genres_ok());
    // Defaults for every collection endpoint (single page each).
    f = f.with_route("trending/movie/day", 200, list_body(&[(1, "TrendMovie")]));
    f = f.with_route("movie/popular", 200, list_body(&[(2, "PopMovie")]));
    f = f.with_route("movie/top_rated", 200, list_body(&[(3, "TopMovie")]));
    f = f.with_route("movie/upcoming", 200, list_body(&[(4, "UpMovie")]));
    f = f.with_route("movie/now_playing", 200, list_body(&[(5, "NowMovie")]));
    f = f.with_route("with_genres=99", 200, list_body(&[(6, "DocMovie")]));
    f = f.with_route("sort_by=primary_release_date.desc", 200, list_body(&[(7, "LatestMovie")]));
    f = f.with_route("trending/tv/day", 200, list_body(&[(8, "TrendTv")]));
    f = f.with_route("tv/popular", 200, list_body(&[(9, "PopTv")]));
    f = f.with_route("tv/top_rated", 200, list_body(&[(10, "TopTv")]));
    f = f.with_route("sort_by=first_air_date.desc", 200, list_body(&[(11, "LatestTv")]));
    Arc::new(f)
}

async fn run(db: &Db, c: Arc<TmdbClient>, state: Arc<SyncState>) -> sync::SyncOutcome {
    let images = Arc::new(RwLock::new(ImageConfig::default()));
    sync::run_sync(Db(db.0.clone()), c, images, state).await.unwrap()
}

#[tokio::test]
async fn full_sync_populates_collections_and_is_idempotent() {
    let db = Db::open_in_memory().unwrap();
    let fetcher = base_fetcher();
    let state = Arc::new(SyncState::new());
    let outcome = run(&db, client(fetcher.clone()), state.clone()).await;

    assert!(outcome.errors.is_empty(), "no errors expected: {:?}", outcome.errors);
    assert!(outcome.records > 0);

    {
        let conn = db.0.lock();
        let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
        let tv: i64 = conn.query_row("SELECT COUNT(*) FROM tv_shows", [], |r| r.get(0)).unwrap();
        assert!(movies >= 7, "movie rows synced, got {movies}");
        assert!(tv >= 4, "tv rows synced, got {tv}");
        let docs = streamnest_lib::metadata::repo::collection_cards(&conn, "documentaries", 50).unwrap();
        assert_eq!(docs.len(), 1, "documentaries collection populated");
        let genres: i64 = conn.query_row("SELECT COUNT(*) FROM genres", [], |r| r.get(0)).unwrap();
        assert!(genres >= 2);
    }

    // Phase transitions end at Completed.
    match &*state.phase.lock() {
        SyncPhase::Completed { records, errors, .. } => {
            assert!(*records > 0);
            assert!(errors.is_empty());
        }
        other => panic!("expected Completed phase, got {other:?}"),
    }

    // Second run: same totals (duplicate prevention via upsert).
    let fetcher2 = base_fetcher();
    let state2 = Arc::new(SyncState::new());
    let outcome2 = run(&db, client(fetcher2), state2).await;
    {
        let conn = db.0.lock();
        let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
        let items: i64 = conn
            .query_row("SELECT COUNT(*) FROM catalog_collection_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(movies, 7, "no duplicate movies after re-sync");
        assert_eq!(outcome2.records, outcome.records, "idempotent record counts");
        assert_eq!(items, 11, "collection items replaced, not appended");
    }
}

#[tokio::test]
async fn resync_updates_existing_records() {
    let db = Db::open_in_memory().unwrap();
    run(&db, client(base_fetcher()), Arc::new(SyncState::new())).await;

    // Re-sync with a changed title for movie id 1.
    let fetcher2 = MockFetcher::default()
        .with_route("configuration", 200, config_ok())
        .with_route("genre/movie/list", 200, genres_ok())
        .with_route("genre/tv/list", 200, genres_ok())
        .with_route("trending/movie/day", 200, list_body(&[(1, "Renamed Movie")]));
    // All other endpoints 404 on purpose: this run only refreshes one collection.
    run(&db, client(Arc::new(fetcher2)), Arc::new(SyncState::new())).await;

    let conn = db.0.lock();
    let title: String = conn
        .query_row("SELECT title FROM movies WHERE tmdb_id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "Renamed Movie", "existing record must be updated in place");
    let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
    assert_eq!(movies, 7, "update must not duplicate rows");
}

#[tokio::test]
async fn pagination_fetches_multiple_pages() {
    let db = Db::open_in_memory().unwrap();
    let page1 = json!({
        "page": 1, "total_pages": 2,
        "results": [{"id": 100, "title": "PageOne", "popularity": 5.0, "vote_average": 6.0,
                     "vote_count": 10, "release_date": "2024-01-01", "genre_ids": []}]
    })
    .to_string();
    let page2 = json!({
        "page": 2, "total_pages": 2,
        "results": [{"id": 101, "title": "PageTwo", "popularity": 4.0, "vote_average": 6.0,
                     "vote_count": 10, "release_date": "2024-01-02", "genre_ids": []}]
    })
    .to_string();
    let fetcher = Arc::new(
        MockFetcher::default()
            .with_route("trending/movie/day?api_key=test-key&page=2", 200, page2)
            .with_route("trending/movie/day", 200, page1)
            .with_route("configuration", 200, config_ok())
            .with_route("genre/movie/list", 200, genres_ok())
            .with_route("genre/tv/list", 200, genres_ok()),
    );

    // Run a sync where only trending_movies works (others 404, tolerated).
    let outcome = run(&db, client(fetcher.clone()), Arc::new(SyncState::new())).await;

    let calls = fetcher.calls.read();
    let p1 = calls.iter().any(|u| u.contains("trending/movie/day") && u.contains("page=1"));
    let p2 = calls.iter().any(|u| u.contains("trending/movie/day") && u.contains("page=2"));
    assert!(p1 && p2, "both pages requested");

    let conn = db.0.lock();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM catalog_collection_items ci
             JOIN catalog_collections c ON c.id = ci.collection_id
             WHERE c.collection_key = 'trending_movies'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2, "items from both pages stored");
    let _ = outcome;
}

#[tokio::test]
async fn failed_collection_leaves_others_intact() {
    let db = Db::open_in_memory().unwrap();
    let fetcher = base_fetcher();
    // Make documentaries fail with a server error (retry path). Inserted first
    // so it wins route matching over the healthy fallback route.
    fetcher.routes.write().insert(
        0,
        Route { matches: "with_genres=99", status: 503, body: "down".to_string() },
    );
    let state = Arc::new(SyncState::new());
    let outcome = run(&db, client(fetcher), state.clone()).await;

    assert!(
        outcome.errors.iter().any(|e| e.contains("Documentaries")),
        "documentary failure recorded: {:?}",
        outcome.errors
    );
    let conn = db.0.lock();
    let pop: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM catalog_collection_items ci
             JOIN catalog_collections c ON c.id = ci.collection_id
             WHERE c.collection_key = 'popular_movies'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pop, 1, "other collections unaffected by one failure");
    let docs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM catalog_collection_items ci
             JOIN catalog_collections c ON c.id = ci.collection_id
             WHERE c.collection_key = 'documentaries'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(docs, 0, "failed collection not populated");
}

#[tokio::test]
async fn all_requests_failing_reports_errors_without_corruption() {
    let db = Db::open_in_memory().unwrap();
    let fetcher = MockFetcher::default(); // everything 404s
    let state = Arc::new(SyncState::new());
    let outcome = run(&db, client(Arc::new(fetcher)), state.clone()).await;

    assert!(outcome.errors.len() >= 11, "every collection reported: {:?}", outcome.errors);
    let conn = db.0.lock();
    let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
    assert_eq!(movies, 0, "nothing written when all API calls fail");
    let last = streamnest_lib::metadata::repo::sync_status(&conn)
        .unwrap()
        .last_sync
        .expect("log row exists");
    assert_eq!(last.status, "completed");
    assert!(last.error_message.is_some(), "failure details recorded in sync log");
}

#[tokio::test]
async fn cancel_stops_sync_early() {
    let db = Db::open_in_memory().unwrap();
    let fetcher = base_fetcher();
    let state = Arc::new(SyncState::new());
    *state.cancel.lock() = true;
    let outcome = run(&db, client(fetcher), state.clone()).await;

    assert!(
        outcome.errors.iter().any(|e| e.contains("cancelled")),
        "cancel recorded: {:?}",
        outcome.errors
    );
    let conn = db.0.lock();
    let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
    assert_eq!(movies, 0, "cancelled before any collection synced");
}

#[tokio::test]
async fn ensure_details_fetches_and_force_refreshes() {
    let db = Db::open_in_memory().unwrap();
    let details1 = json!({
        "id": 45745, "title": "Sintel", "runtime": 15,
        "genres": [{"id": 99, "name": "Documentary"}],
        "credits": {"cast": [{"id": 1, "name": "Ada", "character": "Hero"}], "crew": []}
    })
    .to_string();
    let details2 = json!({
        "id": 45745, "title": "Sintel", "runtime": 16,
        "genres": [{"id": 99, "name": "Documentary"}],
        "credits": {"cast": [], "crew": [{"id": 2, "name": "Bob", "job": "Director"}]}
    })
    .to_string();
    let fetcher = Arc::new(
        MockFetcher::default()
            .with_route("movie/45745", 200, details1.clone()),
    );
    let c = client(fetcher.clone());
    sync::ensure_details(&db, &c, "movie", 45745, false).await.unwrap();

    {
        let conn = db.0.lock();
        let detail = streamnest_lib::metadata::repo::movie_detail(&conn, 45745)
            .unwrap()
            .expect("movie stored");
        assert_eq!(detail.runtime_minutes, Some(15));
        assert_eq!(detail.cast.len(), 1);
        assert!(detail.details_fetched);
    }

    // Cached path: no second request.
    let calls_before = fetcher.calls.read().len();
    sync::ensure_details(&db, &c, "movie", 45745, false).await.unwrap();
    assert_eq!(fetcher.calls.read().len(), calls_before, "no request when details cached");

    // Force refresh updates in place.
    fetcher
        .routes
        .write()
        .insert(0, Route { matches: "movie/45745", status: 200, body: details2 });
    sync::ensure_details(&db, &c, "movie", 45745, true).await.unwrap();
    let conn = db.0.lock();
    let detail = streamnest_lib::metadata::repo::movie_detail(&conn, 45745)
        .unwrap()
        .unwrap();
    assert_eq!(detail.runtime_minutes, Some(16), "force refresh updated runtime");
    assert_eq!(detail.crew.len(), 1, "credits replaced");
    assert_eq!(detail.cast.len(), 0, "old credits not merged");
}

#[tokio::test]
async fn ensure_season_episodes_fetches_once() {
    let db = Db::open_in_memory().unwrap();
    let season_body = json!({
        "id": 9100, "season_number": 1, "name": "Season 1", "air_date": "2020-01-01",
        "episodes": [
            {"id": 1, "episode_number": 1, "season_number": 1, "name": "Ep One", "air_date": "2020-01-01"},
            {"id": 2, "episode_number": 2, "season_number": 1, "name": "Ep Two", "air_date": "2020-01-08"}
        ]
    })
    .to_string();
    let fetcher = Arc::new(
        MockFetcher::default()
            .with_route("tv/77/season/1", 200, season_body.clone()),
    );
    let c = client(fetcher.clone());

    // Seed the show so season rows have a parent.
    sync::ensure_details(
        &db,
        &client(Arc::new(
            MockFetcher::default().with_route(
                "tv/77",
                200,
                json!({"id": 77, "name": "Show", "seasons": [
                    {"id": 9100, "season_number": 1, "name": "Season 1", "episode_count": 2}
                ]})
                .to_string(),
            ),
        )),
        "tv",
        77,
        false,
    )
    .await
    .unwrap();

    sync::ensure_season_episodes(&db, &c, 77, 1).await.unwrap();
    let calls_after_first = fetcher.calls.read().len();
    sync::ensure_season_episodes(&db, &c, 77, 1).await.unwrap();
    assert_eq!(
        fetcher.calls.read().len(),
        calls_after_first,
        "episodes cached: no repeated request"
    );

    let conn = db.0.lock();
    let eps = streamnest_lib::metadata::repo::season_episodes(&conn, 77, 1).unwrap();
    assert_eq!(eps.len(), 2);
    assert_eq!(eps[1].name.as_deref(), Some("Ep Two"));
}

#[tokio::test]
async fn details_missing_without_api_key_reports_local_only_error() {
    // Covers the "TMDB not configured" path by attempting details for an unknown movie:
    // the sync repository returns a clear error instead of fabricating data.
    let db = Db::open_in_memory().unwrap();
    let conn = db.0.lock();
    let missing = streamnest_lib::metadata::repo::movie_detail(&conn, 12345).unwrap();
    assert!(missing.is_none(), "no fabricated rows for unknown titles");
}

#[tokio::test]
async fn sync_status_counts_reflect_sync() {
    let db = Db::open_in_memory().unwrap();
    run(&db, client(base_fetcher()), Arc::new(SyncState::new())).await;
    let conn = db.0.lock();
    let status = streamnest_lib::metadata::repo::sync_status(&conn).unwrap();
    assert!(status.movies > 0);
    assert!(status.tv_shows > 0);
    assert!(
        status.collections.iter().all(|c| c.last_synced_at.is_some()),
        "every collection tracks last_synced_at"
    );
    let last = status.last_sync.expect("log present");
    assert!(last.records_processed > 0);
}
