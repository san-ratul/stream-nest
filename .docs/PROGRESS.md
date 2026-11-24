# Development Progress Journal

Chronological log. Latest first. Includes: phase status, files touched, tests executed + results, blockers.

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
