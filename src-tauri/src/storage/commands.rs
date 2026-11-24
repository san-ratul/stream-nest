use serde::Serialize;

use crate::storage::Db;

#[derive(Serialize)]
pub struct CommandErrorDb {
    pub message: String,
}

impl From<rusqlite::Error> for CommandErrorDb {
    fn from(e: rusqlite::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

pub type DbResult<T> = Result<T, CommandErrorDb>;

#[derive(Serialize, Clone)]
pub struct HistoryRow {
    pub id: i64,
    pub media_key: String,
    pub title: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub last_played_at: String,
}

#[derive(Serialize, Clone)]
pub struct LibraryRow {
    pub id: i64,
    pub media_key: String,
    pub title: String,
    pub media_type: String,
    pub added_at: String,
}

#[derive(Serialize, Clone)]
pub struct FavoriteRow {
    pub id: i64,
    pub media_key: String,
    pub title: String,
    pub created_at: String,
}

#[tauri::command]
pub fn db_upsert_torrent(
    db: tauri::State<'_, Db>,
    info_hash: String,
    name: String,
    output_folder: Option<String>,
) -> DbResult<i64> {
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO torrents(info_hash, name, output_folder) VALUES (?1, ?2, ?3)
         ON CONFLICT(info_hash) DO UPDATE SET name = excluded.name, output_folder = excluded.output_folder",
        rusqlite::params![info_hash, name, output_folder],
    )?;
    let id = conn.query_row(
        "SELECT id FROM torrents WHERE info_hash = ?1",
        [&info_hash],
        |r| r.get(0),
    )?;
    Ok(id)
}

#[tauri::command]
pub fn history_upsert(
    db: tauri::State<'_, Db>,
    media_key: String,
    title: String,
    position_seconds: f64,
    duration_seconds: f64,
) -> DbResult<()> {
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO playback_history(media_key, title, position_seconds, duration_seconds)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(media_key) DO UPDATE SET
           title = excluded.title,
           position_seconds = excluded.position_seconds,
           duration_seconds = excluded.duration_seconds,
           last_played_at = datetime('now')",
        rusqlite::params![media_key, title, position_seconds, duration_seconds],
    )?;
    Ok(())
}

#[tauri::command]
pub fn history_list(db: tauri::State<'_, Db>, limit: i64) -> DbResult<Vec<HistoryRow>> {
    let conn = db.0.lock();
    let mut stmt = conn.prepare(
        "SELECT id, media_key, title, position_seconds, duration_seconds, last_played_at
         FROM playback_history ORDER BY last_played_at DESC, id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                media_key: r.get(1)?,
                title: r.get(2)?,
                position_seconds: r.get(3)?,
                duration_seconds: r.get(4)?,
                last_played_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn history_remove(db: tauri::State<'_, Db>, id: i64) -> DbResult<()> {
    let conn = db.0.lock();
    conn.execute("DELETE FROM playback_history WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn library_add(
    db: tauri::State<'_, Db>,
    media_key: String,
    title: String,
    path: Option<String>,
    media_type: Option<String>,
) -> DbResult<()> {
    let media_type = media_type
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "torrent_video".to_string());
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO library_items(media_key, title, media_type, path) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(media_key) DO UPDATE SET title = excluded.title, path = excluded.path, media_type = excluded.media_type",
        rusqlite::params![media_key, title, media_type, path],
    )?;
    Ok(())
}

#[tauri::command]
pub fn library_list(db: tauri::State<'_, Db>) -> DbResult<Vec<LibraryRow>> {
    let conn = db.0.lock();
    let mut stmt = conn.prepare(
        "SELECT id, media_key, title, media_type, added_at FROM library_items ORDER BY added_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LibraryRow {
                id: r.get(0)?,
                media_key: r.get(1)?,
                title: r.get(2)?,
                media_type: r.get(3)?,
                added_at: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn library_remove(db: tauri::State<'_, Db>, media_key: String) -> DbResult<()> {
    let conn = db.0.lock();
    conn.execute("DELETE FROM library_items WHERE media_key = ?1", [&media_key])?;
    Ok(())
}

#[tauri::command]
pub fn favorites_add(db: tauri::State<'_, Db>, media_key: String, title: String) -> DbResult<()> {
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO favorites(media_key, title) VALUES (?1, ?2)
         ON CONFLICT(media_key) DO UPDATE SET title = excluded.title",
        rusqlite::params![media_key, title],
    )?;
    Ok(())
}

#[tauri::command]
pub fn favorites_list(db: tauri::State<'_, Db>) -> DbResult<Vec<FavoriteRow>> {
    let conn = db.0.lock();
    let mut stmt = conn.prepare(
        "SELECT id, media_key, title, created_at FROM favorites ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FavoriteRow {
                id: r.get(0)?,
                media_key: r.get(1)?,
                title: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn favorites_remove(db: tauri::State<'_, Db>, media_key: String) -> DbResult<()> {
    let conn = db.0.lock();
    conn.execute("DELETE FROM favorites WHERE media_key = ?1", [&media_key])?;
    Ok(())
}

#[tauri::command]
pub fn settings_set(db: tauri::State<'_, Db>, key: String, value: String) -> DbResult<()> {
    let conn = db.0.lock();
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [&key, &value],
    )?;
    Ok(())
}

#[tauri::command]
pub fn settings_get_all(db: tauri::State<'_, Db>) -> DbResult<Vec<(String, String)>> {
    let conn = db.0.lock();
    let mut stmt = conn.prepare("SELECT key, value FROM app_settings")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
