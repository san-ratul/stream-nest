use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use librqbit::api::TorrentIdOrHash;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

pub struct StreamingInfo {
    pub port: u16,
    pub token: Arc<String>,
}

struct ServerState {
    api: Arc<librqbit::Api>,
    token: String,
    port: u16,
}

pub async fn spawn(api: Arc<librqbit::Api>) -> anyhow::Result<StreamingInfo> {
    let token = Arc::new(uuid::Uuid::new_v4().simple().to_string());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    let state = Arc::new(ServerState {
        api,
        token: (*token).clone(),
        port,
    });

    let app = Router::new()
        .route("/{token}/{id}/{file_id}", get(stream_file))
        .with_state(state);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("streaming server terminated: {e}");
        }
    });

    log::info!("streaming server listening on 127.0.0.1:{port}");
    Ok(StreamingInfo { port, token })
}

fn host_allowed(host: Option<&str>, port: u16) -> bool {
    match host {
        Some(h) => h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}"),
        None => false,
    }
}

fn parse_range(value: Option<&str>, len: u64) -> Result<Option<(u64, u64)>, StatusCode> {
    let Some(v) = value else {
        return Ok(None);
    };
    let Some(spec) = v.strip_prefix("bytes=") else {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    };
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    };
    let Ok(start) = start_raw.parse::<u64>() else {
        return Ok(None);
    };
    let end = if end_raw.is_empty() {
        len
    } else {
        match end_raw.parse::<u64>() {
            Ok(e) => (e + 1).min(len),
            Err(_) => return Err(StatusCode::RANGE_NOT_SATISFIABLE),
        }
    };
    if start >= len || end <= start {
        return Err(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    Ok(Some((start, end)))
}

async fn stream_file(
    State(state): State<Arc<ServerState>>,
    Path((token, id_raw, file_id)): Path<(String, String, usize)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if token != state.token {
        return Err(StatusCode::NOT_FOUND);
    }
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok());
    if !host_allowed(host, state.port) {
        return Err(StatusCode::FORBIDDEN);
    }
    let id = match TorrentIdOrHash::parse(&id_raw) {
        Ok(id) => id,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    let mut stream = state
        .api
        .api_stream(id, file_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let len = stream.len();

    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    if let Ok(mime) = state.api.torrent_file_mime_type(id, file_id) {
        if let Ok(v) = mime.parse() {
            resp_headers.insert(header::CONTENT_TYPE, v);
        }
    }

    let range = parse_range(headers.get(header::RANGE).and_then(|h| h.to_str().ok()), len)?;

    let status;
    let body;
    match range {
        Some((start, end)) => {
            status = StatusCode::PARTIAL_CONTENT;
            stream
                .seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            resp_headers.insert(
                header::CONTENT_RANGE,
                format!("bytes {}-{}/{}", start, end - 1, len)
                    .parse()
                    .unwrap(),
            );
            resp_headers.insert(
                header::CONTENT_LENGTH,
                (end - start).to_string().parse().unwrap(),
            );
            let taken = stream.take(end - start);
            body = axum::body::Body::from_stream(ReaderStream::with_capacity(taken, 65536));
        }
        None => {
            status = StatusCode::OK;
            resp_headers.insert(header::CONTENT_LENGTH, len.to_string().parse().unwrap());
            body = axum::body::Body::from_stream(ReaderStream::with_capacity(stream, 65536));
        }
    }

    Ok((status, resp_headers, body))
}
