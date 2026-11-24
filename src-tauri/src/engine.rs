use std::{path::PathBuf, sync::Arc};

use librqbit::{
    Api,
    DhtSessionConfig,
    ListenerMode,
    ListenerOptions,
    Session,
    SessionOptions,
    SessionPersistenceConfig,
    dht::DhtPersistenceConfig,
};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager};

use crate::{events, streaming};

pub struct EngineShared {
    pub api: Api,
    #[allow(dead_code)]
    pub download_dir: PathBuf,
    pub streaming: streaming::StreamingInfo,
}

pub struct EngineState {
    pub inner: Arc<RwLock<Option<Arc<EngineShared>>>>,
    pub init_error: Arc<RwLock<Option<String>>>,
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
            init_error: Arc::new(RwLock::new(None)),
        }
    }

    pub fn shared(&self) -> Option<Arc<EngineShared>> {
        self.inner.read().clone()
    }
}

pub(crate) async fn build_session(data_dir: PathBuf) -> anyhow::Result<Arc<EngineShared>> {
    let download_dir = data_dir.join("downloads");
    let engine_dir = data_dir.join("engine");
    let session_dir = engine_dir.join("session");
    std::fs::create_dir_all(&download_dir)?;
    std::fs::create_dir_all(&session_dir)?;

    let listen = if cfg!(debug_assertions) {
        Some(ListenerOptions {
            mode: ListenerMode::TcpOnly,
            listen_addr: (std::net::Ipv4Addr::LOCALHOST, 0).into(),
            ..Default::default()
        })
    } else {
        None
    };

    let dht = if cfg!(debug_assertions) {
        None
    } else {
        Some(DhtSessionConfig {
            persistence: Some(DhtPersistenceConfig {
                config_filename: Some(engine_dir.join("dht.json")),
                ..Default::default()
            }),
            ..Default::default()
        })
    };

    let session = Session::new_with_opts(
        download_dir.clone(),
        SessionOptions {
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(session_dir),
            }),
            fastresume: true,
            listen,
            dht,
            ..Default::default()
        },
    )
    .await?;

    let api = Api::new(session, None);
    let streaming = streaming::spawn(Arc::new(api.clone())).await?;
    Ok(Arc::new(EngineShared {
        api,
        download_dir,
        streaming,
    }))
}

pub async fn init(handle: AppHandle) {
    let state = handle.state::<EngineState>();

    let data_dir = match handle.path().app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            let msg = format!("cannot resolve app data dir: {e}");
            error_and_emit(&handle, &state, &msg);
            return;
        }
    };

    match build_session(data_dir).await {
        Ok(shared) => {
            log::info!("torrent engine initialized");
            *state.inner.write() = Some(shared);
            let _ = handle.emit("engine:ready", ());
            events::spawn_stats_broadcaster(handle);
        }
        Err(e) => {
            let msg = format!("torrent engine init failed: {e:#}");
            error_and_emit(&handle, &state, &msg);
        }
    }
}

fn error_and_emit(handle: &AppHandle, state: &EngineState, msg: &str) {
    log::error!("{msg}");
    *state.init_error.write() = Some(msg.to_string());
    let _ = handle.emit("engine:error", msg.to_string());
}
