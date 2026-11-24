use streamnest_lib::storage::Db;

fn db() -> Db {
    Db::open_in_memory().expect("in-memory db opens")
}

#[test]
fn history_upsert_and_list() {
    let db = db();
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO playback_history(media_key, title, position_seconds, duration_seconds) VALUES ('t:abc:0', 'Test Movie', 120.5, 900.0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO playback_history(media_key, title, position_seconds, duration_seconds) VALUES ('t:abc:0', 'Test Movie', 300.0, 900.0)
         ON CONFLICT(media_key) DO UPDATE SET position_seconds = excluded.position_seconds, last_played_at = datetime('now')",
        [],
    )
    .unwrap();
    drop(conn);

    let rows = {
        let conn = db.0.lock();
        let mut stmt = conn
            .prepare("SELECT media_key, position_seconds FROM playback_history WHERE media_key = 't:abc:0'")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(rows.len(), 1, "upsert must not create duplicate rows");
    assert!((rows[0].1 - 300.0).abs() < 0.001);
}

#[test]
fn favorites_toggle_via_unique_key() {
    let db = db();
    {
        let conn = db.0.lock();
        conn.execute("INSERT INTO favorites(media_key, title) VALUES ('t:xyz:1', 'Fav Movie')", []).unwrap();
        conn.execute(
            "INSERT INTO favorites(media_key, title) VALUES ('t:xyz:1', 'Fav Movie Renamed')
             ON CONFLICT(media_key) DO UPDATE SET title = excluded.title",
            [],
        )
        .unwrap();
    }
    let conn = db.0.lock();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "unique media_key must keep one row");
    let title: String = conn
        .query_row("SELECT title FROM favorites", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "Fav Movie Renamed");
}

#[test]
fn settings_roundtrip() {
    let db = db();
    {
        let conn = db.0.lock();
        conn.execute(
            "INSERT INTO app_settings(key, value) VALUES ('theme', 'dark')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO app_settings(key, value) VALUES ('theme', 'midnight')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();
    }
    let conn = db.0.lock();
    let theme: String = conn
        .query_row("SELECT value FROM app_settings WHERE key = 'theme'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(theme, "midnight");
}

#[test]
fn torrent_upsert_idempotent() {
    let db = db();
    {
        let conn = db.0.lock();
        for _ in 0..2 {
            conn.execute(
                "INSERT INTO torrents(info_hash, name, output_folder) VALUES ('aaaa', 'A Movie', '/tmp/x')
                 ON CONFLICT(info_hash) DO UPDATE SET name = excluded.name",
                [],
            )
            .unwrap();
        }
    }
    let conn = db.0.lock();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM torrents", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
