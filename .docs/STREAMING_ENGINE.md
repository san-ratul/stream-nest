# Streaming Engine

Status: M4 ACCEPTED. librqbit + custom Range server verified end-to-end with a throttled local seeder on real 1.1 GB content.

## Verified behavior (2026-11-24, this machine)

Test: Sintel.2010.1080p.mkv (1,180,090,590 bytes, 888.032 s, MKV/H.264) seeded locally by `src-tauri/src/bin/seeder.rs` with upload throttled to 3 MB/s. App added the torrent via `.torrent` file + `initial_peers=["127.0.0.1:6900"]`, then played `http://127.0.0.1:{port}/{token}/1/0` in libmpv.

| Metric | Measured | Target | Verdict |
|---|---|---|---|
| Time to first frame (mid-download, ~0.3% complete) | **276–374 ms** | ≤ ~15 s | PASS |
| Random seek to 35% (undownloaded region) | ~5 s piece-fetch stall, then smooth resume | ≤ ~5 s | PASS (borderline) |
| Random seek to 70% (undownloaded region) | stall then resume; same pattern | ≤ ~5 s | PASS |
| Seek when data available (post-download region) | property response 3–7 ms | — | n/a |
| Steady playback CPU (1080p, hwdec on) | ~7% of one core (process total ≈ 6–8 CPU-s over ~85 s wall) | ~15% reference | PASS |
| RAM (working set during playback) | ~390 MB (app process incl. webview + engine + mpv) | lightweight | OK |
| Codec/container coverage | H.264/AAC in MKV, duration 888.032 s read correctly | — | PASS |

## Behaviors observed

- Playback-piece prioritization: engine fills the head sequentially (mpv buffers ahead); a seek Range request triggers on-demand prioritization of that region — the 35%/70% stalls show the wait, then uninterrupted playback.
- Seek "resume" telemetry measures the time-pos property jump (3–7 ms); the honest end-to-end seek latency mid-download is the ~5 s stall — recorded separately above.
- 1 Hz `engine:torrents` stats broadcast ran alongside playback without visible cost.
- Session persistence (JSON + fastresume) restored prior torrents on boot; `torrent added` logged on resume.

## Harness

- `seeder.rs` args: `<media-dir> [port] [upload_bps]` — creates .torrent files into `<media-dir>/torrents/`, prints `TORRENT_FILE` / `SEEDER_READY` / `THROTTLED` markers, parks. Legal fixtures only (Blender Foundation CC-BY: Big Buck Bunny 10s clip, Sintel full movie).
- App dev E2E panel (`import.meta.env.DEV` only) auto-runs when engine becomes ready: reads the .torrent via `dev_read_test_torrent` (debug-only command), adds with initial peer, streams file 0, schedules seeks at 35%/70%.
- Visual verification: window screenshots via PrintWindow + foreground screen capture (PrintWindow alone cannot capture D3D flip-model child content — shows black even when video renders).

## Known constraints

- `vo=gpu` required (see PLAYER_INTEGRATION.md): gpu-next/libplacebo presents black on the embedded foreign HWND.
- Debug builds bind the torrent listener to 127.0.0.1 and disable DHT (deterministic dev runs, no Windows Firewall prompt); release builds use default listeners + persisted DHT (firewall prompt expected on first run — documented).
- No MSE/legacy encryption in librqbit (TD-001 fallback trigger remains documented).
