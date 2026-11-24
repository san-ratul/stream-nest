# Development Progress Journal

Chronological log. Latest first. Includes: phase status, files touched, tests executed + results, blockers.

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
