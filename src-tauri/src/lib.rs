pub mod catalog;
mod commands;
mod engine;
mod events;
pub mod metadata;
pub mod streaming;
pub mod storage;

use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
fn health() -> String {
    "streamnest-ok".to_string()
}

#[tauri::command]
fn dev_log(message: String, level: Option<String>) {
    match level.as_deref() {
        Some("warn") => log::warn!("[frontend] {message}"),
        Some("error") => log::error!("[frontend] {message}"),
        _ => log::info!("[frontend] {message}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_libmpv::init())
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(engine::EngineState::new());
            app.manage(catalog::CatalogState {
                provider: Box::new(catalog::provider::BundledProvider::new()),
            });
            let data_dir = app.path().app_data_dir()?;
            let db = storage::Db::open(data_dir.join("streamnest.db")).map_err(|e| e.to_string())?;
            app.manage(db);

            // TMDB metadata stack: client only when a key is configured; the key
            // never reaches the frontend and never appears in logs.
            let api_key = metadata::config::load_tmdb_api_key();
            let client = match api_key {
                Some(key) => match metadata::build_client(key) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        log::warn!("TMDB client init failed: {e}");
                        None
                    }
                },
                None => {
                    log::info!("TMDB_API_KEY not configured; catalog discovery runs in local-only mode");
                    None
                }
            };
            let tmdb_state = metadata::TmdbState {
                client,
                image_config: Arc::new(parking_lot::RwLock::new(metadata::tmdb::ImageConfig::default())),
                images: parking_lot::RwLock::new(None),
                sync: Arc::new(metadata::sync::SyncState::new()),
                sources: Box::new(metadata::sources::BundledContentSources::new()),
                data_dir: data_dir.clone(),
            };
            app.manage(tmdb_state);

            // Local image proxy (127.0.0.1, token path, bounded cache).
            let handle_for_images = handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = handle_for_images.state::<metadata::TmdbState>();
                let cache_dir = state.data_dir.join("images");
                match metadata::images::spawn(
                    cache_dir,
                    state.image_config.clone(),
                    512, // MB; bounded by design
                )
                .await
                {
                    Ok(info) => {
                        *state.images.write() = Some(info);
                    }
                    Err(e) => log::error!("image proxy failed to start: {e}"),
                }
            });

            tauri::async_runtime::spawn(engine::init(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            dev_log,
            catalog::catalog_list,
            catalog::catalog_get,
            metadata::catalog_image_url,
            metadata::catalog_home,
            metadata::catalog_search,
            metadata::movie_details,
            metadata::tv_details,
            metadata::season_episodes,
            metadata::catalog_status,
            metadata::catalog_sync_start,
            metadata::catalog_sync_cancel,
            metadata::catalog_clear,
            metadata::catalog_set_pages,
            storage::commands::db_upsert_torrent,
            storage::commands::history_upsert,
            storage::commands::history_list,
            storage::commands::history_remove,
            storage::commands::library_add,
            storage::commands::library_list,
            storage::commands::library_remove,
            storage::commands::favorites_add,
            storage::commands::favorites_list,
            storage::commands::favorites_remove,
            storage::commands::settings_set,
            storage::commands::settings_get_all,
            commands::engine_status,
            commands::torrent_add_uri,
            commands::torrent_add_file_base64,
            commands::torrent_list,
            commands::torrent_details,
            commands::torrent_stats,
            commands::torrent_peer_stats,
            commands::torrent_pause,
            commands::torrent_start,
            commands::torrent_forget,
            commands::torrent_delete,
            commands::torrent_update_only_files,
            commands::session_stats,
            commands::stream_url,
            commands::import_subtitle,
            commands::storage_usage,
            commands::clear_subtitles,
            commands::apply_engine_limits,
            #[cfg(debug_assertions)]
            commands::dev_read_test_torrent,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
