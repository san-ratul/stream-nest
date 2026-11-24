use std::{net::Ipv4Addr, path::PathBuf, time::Duration};

use librqbit::{
    ListenerMode,
    ListenerOptions,
    Session,
    SessionOptions,
};

const VIDEO_EXTS: [&str; 5] = ["mp4", "mkv", "avi", "webm", "mov"];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let media_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: seeder <media-dir> [port]"),
    );
    let port: u16 = std::env::args()
        .nth(2)
        .and_then(|p| p.parse().ok())
        .unwrap_or(6900);

    let out_dir = media_dir.join("torrents");
    std::fs::create_dir_all(&out_dir)?;

    let session = Session::new_with_opts(
        media_dir.clone(),
        SessionOptions {
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpOnly,
                listen_addr: (Ipv4Addr::UNSPECIFIED, port).into(),
                ..Default::default()
            }),
            dht: None,
            ..Default::default()
        },
    )
    .await?;

    let mut seeded = 0;
    for entry in std::fs::read_dir(&media_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !VIDEO_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf8 filename")
            .to_string();

        let (created, handle) = session
            .create_and_serve_torrent(
                &path,
                librqbit::CreateTorrentOptions {
                    name: Some(&name),
                    ..Default::default()
                },
            )
            .await?;

        let upload_bps = std::env::args()
            .nth(3)
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0);
        if let Some(bps) = upload_bps {
            session.ratelimits.set_upload_bps(Some(std::num::NonZeroU32::new(bps).unwrap()));
            println!("THROTTLED\tupload_bps={bps}");
        }

        handle
            .wait_until_completed()
            .await
            .expect("hash check completes");

        let bytes = created.as_bytes()?;
        let out_path = out_dir.join(format!("{name}.torrent"));
        std::fs::write(&out_path, &bytes)?;
        seeded += 1;
        println!("TORRENT_FILE\t{}", out_path.display());
    }

    println!("SEEDER_READY\tport={port}\tcount={seeded}");

    tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
    Ok(())
}
