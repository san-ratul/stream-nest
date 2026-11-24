# StreamNest PRD (working copy)

Authoritative source: user-provided PRD of 2026-11-24. This file mirrors it for the repository; product decisions that change PRD meaning must be recorded in TECHNICAL_DECISIONS.md.

## Vision

Lightweight, privacy-first desktop app that streams video through BitTorrent progressively (no full-download wait), with an integrated player, personal library, and a curated **legal** catalog. Local-first, no mandatory account.

## Positioning

"Your Media. Your Device. Your Control." Stremio-like streaming simplicity + personal library + lightweight desktop performance + optional premium cloud (out of MVP).

## Approved stack (MVP)

Tauri 2 + Vue 3 + JavaScript + Tailwind CSS 4 + Vite; Rust backend; librqbit (libtorrent-FFI fallback); libmpv via tauri-plugin-libmpv (HWND fallback); SQLite; filesystem storage. Cloud/payments/accounts: NOT in MVP.

## Functional requirements (MVP scope)

1. **Torrent management** — magnet input, .torrent import, metadata, file list, per-file selection, pause/resume, remove, speeds, peers/seeds, network limits.
2. **Streaming engine (core)** — progressive playback, playback-piece priority, random seek + reprioritize, adaptive buffering, continuity, graceful missing-piece handling, temp storage, bounded concurrency, **HTTP Range local interface**.
3. **Player** — play/pause, seek, volume, fullscreen, subtitle selection + external SRT, audio track selection, speed, resume, shortcuts, hardware acceleration. libmpv native rendering in-window.
4. **Legal catalog** — curated rights-verified sources only; provider/source attribution; license info; search/filter. No piracy indexes or unauthorized add-ons.
5. **Library** — recently played, history, favorites, continue watching, local organization, search/filter.
6. **Download manager** — active/completed, speeds, pause/resume, destination, storage usage, cleanup.
7. **Settings** — download dir, cache dir + size limit, speed limits, connection limit, player/subtitle prefs, privacy, startup behavior.

## Non-goals (MVP)

macOS/Linux, mobile, remote streaming, cloud torrent hosting, social, add-on ecosystem, cloud profiles, lifetime billing, payments/accounts/cloud.

## Screens

Home, Discover, Add Torrent, Torrent Details, Player, Download Manager, Library, Favorites, Settings, About.

## Security/privacy (mandatory)

Local-first; localhost-only streaming; random endpoint auth; strict request validation; filename sanitization; path traversal protection; restricted Tauri capabilities; no arbitrary frontend file access; opt-in diagnostics; no torrent-history collection; signed updates (post-MVP for signing cert); dependency review.

## Success definition (MVP release gate)

User can add supported torrent → playback before full download → reliable seek when pieces available → player inside main window → app usable without account → catalog sources verified → local data secure → install/update works → licensing obligations documented (see BUILD_AND_RELEASE.md).

## Content policy

User-provided magnets/torrents supported. Catalog limited to legally authorized content with documented rights and attribution. No piracy features.
