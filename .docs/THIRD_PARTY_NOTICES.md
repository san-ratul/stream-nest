# THIRD-PARTY NOTICES

StreamNest includes third-party software. Full machine-readable inventories: `CARGO_DEPENDENCIES.txt` (608 Cargo crates) and `package.json` (npm).

## Runtime components

| Component | License | Role | Source |
|---|---|---|---|
| librqbit (+ librqbit-* crates) | MIT | BitTorrent engine | github.com/ikatson/rqbit |
| tauri-plugin-libmpv | MPL-2.0 | mpv embedding plugin | github.com/nini22P/tauri-plugin-libmpv |
| libmpv-2.dll | LGPL-2.1-or-later (LGPL build) | media playback | github.com/zhongfly/mpv-winbuild (mpv-dev-lgpl) |
| libmpv-wrapper.dll | MPL-2.0 | libmpv wrapper | github.com/nini22P/libmpv-wrapper |
| Tauri (tauri, tauri-* crates) | MIT OR Apache-2.0 | desktop framework | tauri.app |
| wry / tao / tauri-runtime-wry | MIT OR Apache-2.0 | window/webview runtime | |
| Vue 3 | MIT | UI framework | |
| vue-router | MIT | routing | |
| Vite / @vitejs/plugin-vue | MIT | build tooling | |
| Tailwind CSS 4 | MIT | styling | |
| rusqlite (+ bundled SQLite) | MIT / SQLite public domain | local database | |
| axum / tokio / tokio-util | MIT | streaming HTTP server | |

## Media attribution (test fixtures)

- Big Buck Bunny, Sintel, Tears of Steel, Cosmos Laundromat — (c) Blender Foundation, CC-BY. blender.org

## License notes

- libmpv-2.dll is distributed as a separate DLL and is replaceable by the user (LGPL compliance).
- MPL-2.0 components are file-level copyleft; if StreamNest modifies plugin files, those modifications remain open.
- The proprietary/commercial StreamNest components (accounts, billing, cloud sync) are server-side and not part of this desktop distribution.
