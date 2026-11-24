# Technical Decisions Log

Format: ID | Date | Decision | Rationale | Evidence | Review trigger

## TD-001 | 2026-11-24 | Torrent engine: librqbit primary

- Rationale: native Rust (no FFI), progressive streaming + HTTP Range endpoint built in, MIT license, active (crate 9.1.0, 217k downloads, half in last 90 days), supports DHT/PEX/UDP trackers/magnet/uTP (BEP-29 via librqbit-utp)/fastresume/rate limits/selective files. Ships its own Tauri desktop app (precedent).
- Evidence: crates.io metadata, rqbit README (verified 2026-11-24), libtorrent ecosystem scan (no maintained Rust bindings for libtorrent-rasterbar; all GitHub Rust projects hobbyist/abandoned).
- Fallback: libtorrent-rasterbar via cxx FFI. Trigger: PoC shows unacceptable peer connectivity or throughput vs local-seeder baseline. Known librqbit gap: no MSE/legacy encryption (may reduce reachability on some ISPs); acceptable for MVP, logged as limitation.

## TD-002 | 2026-11-24 | Player: tauri-plugin-libmpv first

- Rationale: MPL-2.0 Tauri 2 plugin embedding libmpv; Windows explicitly "fully tested"; uses transparent-window technique (HTML controls over native video); ships LGPL libmpv-2.dll (zhongfly mpv-dev-lgpl build) satisfying hybrid licensing review; vo=gpu-next + hwdec=auto-safe for hardware decode.
- Evidence: plugin README + crates.io (v0.3.2), Tauri+mpv precedent apps (Ventic 185★ torrent-streaming+Tauri+mpv; soia, skrin, mochi, ParallaxTV, L-MPV).
- Fallback: custom child-HWND embed via libmpv `wid` + windows-rs. Long-term option: render-API into wgpu texture (experimental `mpv-wgpu-player` proves path; not MVP).
- Risks: single-maintainer plugin (fork/vendor if adopted), transparent-window DPI/fullscreen edge cases to verify in M3.

## TD-003 | 2026-11-24 | WebView2 <video> rejected for playback

- No MKV container support, no reliable HEVC, insufficient subtitle/audio-track APIs vs PRD §3.3. libmpv covers all player requirements (SRT import via sub-add, audio track via aid, resume via start property).

## TD-004 | 2026-11-24 | Toolchain: stable-msvc primary, stable-gnu kept as fallback

- MSVC Build Tools + Windows SDK installed via winget this session (user granted elevation through OS prompt). GNU toolchain retained for resilience.

## TD-005 | 2026-11-24 | Streaming architecture: engine-owned piece priority + Range adapter

- librqbit streams via its own Range-capable endpoint with streamed-piece prioritization. StreamNest adds: localhost-only binding, random token auth, Host/Origin validation (Phase 9), session-state persistence around the engine. No custom piece scheduler (PRD §6.4 note respected).

## TD-006 | 2026-11-24 | Frontend: Vue 3 + JavaScript + Tailwind CSS 4 via @tailwindcss/vite

- Per approved stack. Composition API only. No TypeScript.

## TD-007 | 2026-11-24 | MVP dependency discipline

- M0: tauri + serde + serde_json + log/env_logger only. librqbit added at M1, mpv integration at M3, SQLite (rusqlite vs sqlx — decided at Phase 10 with verification) later. No unnecessary dependencies.
