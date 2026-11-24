pub mod catalog;
mod commands;
mod engine;
mod events;
pub mod streaming;

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
            tauri::async_runtime::spawn(engine::init(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            dev_log,
            catalog::catalog_list,
            catalog::catalog_get,
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
            #[cfg(debug_assertions)]
            commands::dev_read_test_torrent,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
