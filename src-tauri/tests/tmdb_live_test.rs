//! Live integration tests against the real TMDB API.
//! These run ONLY when invoked explicitly (they are #[ignore]) and use the
//! key from .env. The default suite must stay network-free.

use std::sync::Arc;

use parking_lot::RwLock;

use streamnest_lib::metadata::sync::{self, SyncPhase, SyncState};
use streamnest_lib::metadata::tmdb::{ImageConfig, TmdbClient};
use streamnest_lib::metadata::{build_client, config};
use streamnest_lib::storage::Db;

fn live_client() -> Option<Arc<TmdbClient>> {
    let key = config::load_tmdb_api_key()?;
    Some(build_client(key).expect("client build"))
}

#[tokio::test]
#[ignore = "live API test; run explicitly with: cargo test --test tmdb_live_test -- --ignored"]
async fn live_configuration_and_list_pages() {
    let Some(client) = live_client() else {
        panic!("TMDB_API_KEY not configured; add .env to run live tests");
    };
    let cfg = client.configuration().await.expect("configuration");
    assert!(cfg.secure_base_url.starts_with("https://image.tmdb.org"));
    assert!(!cfg.poster_sizes.is_empty());

    let page = client.list_page("trending/movie/day", 1).await.expect("trending page");
    assert!(!page.results.is_empty(), "trending results non-empty");
    let first = &page.results[0];
    assert!(first.display_title().len() > 0);

    let details = client.movie_details(first.id).await.expect("movie details");
    assert_eq!(details.id, first.id);
    assert!(!details.display_title().is_empty());
}

#[tokio::test]
#[ignore = "live API test; run explicitly with: cargo test --test tmdb_live_test -- --ignored"]
async fn live_full_sync_populates_real_catalog() {
    let Some(client) = live_client() else {
        panic!("TMDB_API_KEY not configured; add .env to run live tests");
    };
    let db = Db::open_in_memory().unwrap();
    let state = Arc::new(SyncState::new());
    let images = Arc::new(RwLock::new(ImageConfig::default()));
    let outcome = sync::run_sync(Db(db.0.clone()), client, images, state.clone()).await;
    // Network hiccups make strict no-error assertions flaky; require substantial progress.
    let outcome = outcome.expect("sync completes");
    eprintln!("sync outcome errors: {:?}", outcome.errors);
    assert!(outcome.records > 100, "records synced: {}", outcome.records);

    {
        let conn = db.0.lock();
        let movies: i64 = conn.query_row("SELECT COUNT(*) FROM movies", [], |r| r.get(0)).unwrap();
        let tv: i64 = conn.query_row("SELECT COUNT(*) FROM tv_shows", [], |r| r.get(0)).unwrap();
        let collections: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT c.collection_key) FROM catalog_collections c
                 JOIN catalog_collection_items i ON i.collection_id = c.id",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(movies > 50, "movies: {movies}");
        assert!(tv > 30, "tv: {tv}");
        assert!(collections >= 9, "populated collections: {collections}");

        let docs = streamnest_lib::metadata::repo::collection_cards(&conn, "documentaries", 10).unwrap();
        assert!(!docs.is_empty(), "documentaries collection populated");
    }

    let phase = state.phase.lock().clone();
    match &phase {
        SyncPhase::Completed { errors, .. } => {
            assert!(
                errors.len() <= 2,
                "unexpected error volume on a healthy connection: {errors:?}"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}
