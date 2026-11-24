use librqbit::{Api, Session, SessionOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const FILE_SIZE: usize = 65536;

fn pattern_byte(i: usize) -> u8 {
    ((i * 31 + 7) % 251) as u8
}

async fn http_get(
    port: u16,
    path: &str,
    host: &str,
    range: Option<&str>,
) -> anyhow::Result<(u16, String, Vec<u8>)> {
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if let Some(r) = range {
        req.push_str(&format!("Range: {r}\r\n"));
    }
    req.push_str("\r\n");

    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;

    let text_end = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("no header terminator"))?;
    let head = String::from_utf8_lossy(&buf[..text_end]).to_string();
    let body = buf[text_end + 4..].to_vec();

    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("no status line"))?;
    Ok((status, head, body))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_server_serves_token_range_and_host_checks() {
    let dir =
        std::env::temp_dir().join(format!("streamnest-stream-test-{}", std::process::id()));
    let data_dir = dir.join("data");
    std::fs::create_dir_all(&data_dir).expect("create data dir");

    let expected: Vec<u8> = (0..FILE_SIZE).map(pattern_byte).collect();
    let file_path = data_dir.join("sample.bin");
    std::fs::write(&file_path, &expected).expect("write sample file");

    let session = Session::new_with_opts(
        dir.clone(),
        SessionOptions {
            ..Default::default()
        },
    )
    .await
    .expect("session init");

    let (_created, handle) = session
        .create_and_serve_torrent(
            &file_path,
            librqbit::CreateTorrentOptions {
                name: Some("sample.bin"),
                ..Default::default()
            },
        )
        .await
        .expect("create and serve torrent");

    handle
        .wait_until_completed()
        .await
        .expect("hash check completes");

    let api = Api::new(session, None);
    let info = streamnest_lib::streaming::spawn(std::sync::Arc::new(api))
        .await
        .expect("streaming server spawn");

    let id = handle.id();

    // 1. Wrong token must be rejected.
    let (status, _, _) = http_get(info.port, "/deadbeef/0/0", &format!("127.0.0.1:{}", info.port), None)
        .await
        .expect("request completes");
    assert_eq!(status, 404, "wrong token must be rejected");

    // 2. Wrong Host must be rejected.
    let (status, _, _) = http_get(
        info.port,
        &format!("/{}/{}/0", info.token, id),
        "evil.example.com:80",
        None,
    )
    .await
    .expect("request completes");
    assert_eq!(status, 403, "foreign Host must be rejected");

    // 3. Valid Range request must return exactly the requested bytes.
    let path = format!("/{}/{}/0", info.token, id);
    let (status, head, body) = http_get(
        info.port,
        &path,
        &format!("127.0.0.1:{}", info.port),
        Some("bytes=100-1099"),
    )
    .await
    .expect("request completes");
    assert_eq!(status, 206, "range request must yield 206");
    assert!(
        head.contains(&format!("bytes 100-1099/{FILE_SIZE}")),
        "Content-Range header must be exact, got: {head}"
    );
    assert_eq!(body.len(), 1000, "body length must match range");
    assert_eq!(&body[..], &expected[100..1100], "payload must match source bytes");

    // 4. Open-ended Range must stream from offset to end.
    let (status, _, body) = http_get(
        info.port,
        &path,
        &format!("127.0.0.1:{}", info.port),
        Some("bytes=65000-"),
    )
    .await
    .expect("request completes");
    assert_eq!(status, 206);
    assert_eq!(body.len(), FILE_SIZE - 65000);
    assert_eq!(&body[..], &expected[65000..]);

    std::fs::remove_dir_all(dir).ok();
}
