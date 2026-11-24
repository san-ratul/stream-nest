use librqbit::{Api, Session, SessionOptions, SessionPersistenceConfig};

#[tokio::test]
async fn session_boots_and_lists_empty() {
    let dir = std::env::temp_dir().join(format!("streamnest-engine-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let session = Session::new_with_opts(
        dir.clone(),
        SessionOptions {
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(dir.join("session")),
            }),
            fastresume: true,
            ..Default::default()
        },
    )
    .await
    .expect("session init must succeed");

    let api = Api::new(session, None);
    let list = api.api_torrent_list_ext(librqbit::api::ApiTorrentListOpts { with_stats: true });
    assert!(
        list.torrents.is_empty(),
        "fresh session must list no torrents"
    );

    std::fs::remove_dir_all(dir).ok();
}
