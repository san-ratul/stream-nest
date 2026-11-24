# Development Progress Journal

Chronological log. Latest first. Includes: phase status, files touched, tests executed + results, blockers.

---

## 2026-11-24 — Phases 2–4 complete

**Phase 2 (torrent management UX):** live peers panel on details (verified PeerStatsSnapshot shape from source: peers map → counters/state/conn_kind/client_name), file selection persistence via update_only_files, stream handoff to player store, download manager totals/filters/actions.

**Phase 3 (streaming state UX):** buffering detection from time-pos stalls (≥2 s while unpaused), buffer-ahead display via mpv demuxer-cache-time, engine download speed shown during buffering. All values from real engine/player properties — never fabricated.

**Phase 4 (player controls):** keyboard shortcuts (Space/←/→/↑/↓/M/F), fullscreen via Tauri window API (capability `core:window:allow-set-fullscreen` added), speed presets, aspect ratio presets, mute, audio track + subtitle track selectors from live `track-list`, **external SRT import** via new `import_subtitle` command (filename sanitization, extension allowlist srt/ass/vtt/ssa, 2 MiB cap, written to app-data subtitles dir, loaded through mpv sub-add). Resume playback explicitly deferred to Phase 6 (needs SQLite history).

**Tests:** `cargo test` → engine_test 1 passed, streaming_test 1 passed. `npm run build` clean (181 KB JS). Watcher rebuild clean.

---

## 2026-11-24 — Phase 1 complete (application foundation)

- vue-router@4 added (hash history); routes for all 10 PRD screens + `/dev` harness.
- `src/layouts/AppShell.vue`: sidebar nav (Alt+1..8 shortcuts), engine health indicator, active-download badge.
- Stores: `stores/torrents.js` (engine event subscription `engine:torrents`/`engine:ready`/`engine:error`, command wrappers, formatters), `stores/player.js` (current stream), `stores/settings.js` (PRD §3.7 fields; local persistence now, SQLite migration at Phase 8 — documented).
- Components: `AppState.vue` (loading/error/empty), `TorrentRow.vue` (progress/speeds/state badges).
- Pages: Home (dashboard: recent + active torrents, honest Phase 6 placeholder for continue-watching), Add Torrent (magnet validation + file import via base64 + advanced initial peers + engine-ready gating), Torrent Details (async metadata polling, file list with video detection, selection, stream action, pause/resume/delete/forget), Download Manager (totals, filters, per-torrent actions), Player (store-driven, honest empty state), Settings (full PRD §3.7 form), Discover/Library/Favorites (honest phase-pending states), About (policies + license table), Dev Harness (moved off the boot path — no auto-run in normal startup).
- `npm run build` → 54 modules, clean. App boots on Windows; engine ready; session persistence restored both test torrents (Sintel resumed at 93% — live M5 evidence).

**Note:** video-visibility full-shell screenshot pending user glance (focus-steal protection blocked background capture); production build + boot logs green.

---

## 2026-11-24 — M4 complete (progressive streaming ACCEPTED, video rendering fixed)

**M4 run:** Sintel.2010.1080p.mkv (1.1 GB, 888 s, MKV/H.264) via local throttled seeder (3 MB/s upload) → engine → Range server → libmpv. Full evidence in STREAMING_ENGINE.md.

**Results:** first frame 276–374 ms with download ~6 min remaining (true progressive); deep seeks to 35%/70% stall ~5 s for piece fetch then resume smoothly; CPU ~7% of one core at 1080p hwdec; RAM ~390 MB; MKV duration 888.032 s parsed; hwdec=d3d11va throughout.

**Bugs found and fixed this milestone:**
1. Double `invoke_handler` call replaced the command list (frontend got "Command health not found") → single `generate_handler!` list with `#[cfg(debug_assertions)]` entry.
2. Duplicate `let ranE2e` declarations broke App.vue compile → removed.
3. Opaque `html/body` background hid mpv video (user report "no video only sound") → transparent body (plugin contract).
4. `vo=gpu-next` presents black on embedded foreign HWND → switched to `vo=gpu`, verified by on-screen screenshot.
5. DHT persisted-port bind conflict (os 10048) froze engine init twice → debug builds: no DHT + loopback-only listener; release keeps persistence (documented).
6. Second binary made `cargo run` ambiguous → `default-run = "streamnest"`.
7. E: disk full from target/ (17.4 GB) → `.cargo/config.toml` moves target dir to `C:\rust-targets\streamnest`.

**Harness added:** `src-tauri/src/bin/seeder.rs` (create + seed + throttle upload_bps, writes .torrent files); dev E2E auto-run panel; `dev_read_test_torrent` (debug-only); Play/Pause/Seek/Volume UI + first-frame/seek telemetry; PrintWindow/foreground screenshot verification method.

**Blockers:** none. (Windows Firewall prompt on release-style listeners documented as user-consent item.)

---

## 2026-11-24 — M3 complete (player ACCEPTED)

- tauri-plugin-libmpv v0.3.2 integrated (npm `tauri-plugin-libmpv-api` + crate + `libmpv:default` capability).
- Native DLLs installed via `npx tauri-plugin-libmpv-api setup-lib` (LGPL libmpv-2.dll 2026-11-23 build + libmpv-wrapper.dll) → src-tauri/lib/, bundled as resources.
- Window set transparent; `src/composables/usePlayer.js` + `src/pages/PlayerPage.vue` built (init/loadfile/pause/seek/volume/observe).
- `dev_log` command added (frontend diagnostics channel).

**Tests executed:**
- Live playback verification (automated probes, 1 Hz): fresh boot → loadfile Big_Buck_Bunny_1080_10s.mp4 → time-pos advanced 0.73→9.98 (exact 1 s/s), `hwdec=d3d11va`, `fmt=h264`, EOF hold at 9.98 with keep-open — PASSED.
- Instance lifecycle: HMR reload destroyed + re-created instance cleanly — PASSED.
- Capability enforcement: mpv init was correctly blocked before adding `libmpv:default`; allowed after — PASSED (confirms Tauri permission model active).

**Blockers:** none.

---

## 2026-11-24 — M1 + M2 complete

**M1 (librqbit engine):**
- `src-tauri/src/engine.rs` — Session facade (JSON persistence + fastresume + DHT), EngineState managed via setup hook, async init, `engine:ready` / `engine:error` events.
- `src-tauri/src/commands.rs` — 14 commands: engine_status, torrent_add_uri, torrent_add_file_base64, torrent_list, torrent_details, torrent_stats, torrent_peer_stats, torrent_pause/start/forget/delete, torrent_update_only_files, session_stats, stream_url.
- `src-tauri/src/events.rs` — 1 Hz `engine:torrents` stats broadcaster.
- Deps added: librqbit 9.1.0, base64, parking_lot 0.12, tokio, anyhow.

**M2 (progressive streaming):**
- `src-tauri/src/streaming.rs` — custom axum server: binds 127.0.0.1:0, random UUID token in path, Host allowlist (127.0.0.1/localhost only), byte-exact Range semantics (200/206/416), mime via engine, ReaderStream body.
- `stream_url` command returns `http://127.0.0.1:{port}/{token}/{id}/{file_id}` for libmpv.

**Tests executed:**
- `cargo test --test engine_test` → 1 passed (session boots, JSON persistence, empty list).
- `cargo test --test streaming_test` → 1 passed (token 404, foreign-Host 403, Range 206 exact Content-Range + byte-exact payload, open-ended range).
- App run: `torrent engine initialized` in logs; window up (PID 7376) — PASSED.

**Decision recorded:** no rqbit HttpApi server; custom streaming server only (TD-005).

---

## 2026-11-24 — Environment & scaffold (M0)

**Completed:**
- Environment inspection + installs (see ENVIRONMENT.md): rustup 1.29.3, Rust 1.100.0 stable-msvc, VS 2022 Build Tools (MSVC 14.44.35207), Windows SDK 10.0.26100, all verified.
- Rust MSVC smoke test: build + run + user32 link — PASSED.
- Git repo initialized (branch `main`).
- Research locked: TD-001 librqbit, TD-002 tauri-plugin-libmpv (see TECHNICAL_DECISIONS.md).
- Scaffold written: Tauri 2 + Vue 3 + Tailwind 4 + Vite; npm deps installed (56 pkgs); icon set generated; `health` command wired for UI<->backend sanity check.
- Documentation set created (.docs/, 13 documents).

**Tests executed:**
- `cargo check` → exit 0 (443 packages, 40.75s) — PASSED
- `cargo build` (debug) → exit 0 — PASSED
- `npm run build` (vite production) → PASSED (12 modules, dist emitted)
- M0 launch test: `npm run tauri dev` → `streamnest.exe` process running, `MainWindowTitle='StreamNest'` — PASSED
- `npx esbuild --version` → 0.28.2 (allowScripts postinstall blocked, binary OK) — PASSED

**M0 verdict: ACCEPTED.** App builds and launches on Windows with backend health command wired.

**Blockers:** none open. MSVC requirement resolved via winget elevation.

---

## 2026-11-24 — M1 started: librqbit engine integration

- Verified librqbit 9.x API from rqbit source (cloned master): Session::new_with_opts, Api facade (api_torrent_list_ext, api_add_torrent, api_torrent_details, api_stats_v1, api_peer_stats, api_torrent_action_*, api_torrent_action_update_only_files, api_session_stats), Api::new(session, None) with default features, ApiError usable as command error (rqbit desktop precedent), api_stream (M2, public, feature-independent).
- Decision: custom thin streaming server at M2 using Api::api_stream + token-in-path + Host/Origin checks (TD-005); rqbit's HttpApi server will not run.
- Adding deps: librqbit 9 (default features), base64 0.22, parking_lot 1.

---
