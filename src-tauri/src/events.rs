use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::engine::EngineState;

const STATS_INTERVAL: Duration = Duration::from_secs(1);

pub fn spawn_stats_broadcaster(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<EngineState>();
        let mut interval = tokio::time::interval(STATS_INTERVAL);
        loop {
            interval.tick().await;
            let shared = match state.shared() {
                Some(s) => s,
                None => continue,
            };
            let list = shared
                .api
                .api_torrent_list_ext(librqbit::api::ApiTorrentListOpts { with_stats: true });
            let _ = handle.emit("engine:torrents", &list);
        }
    });
}
