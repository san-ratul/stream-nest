# Roadmap

Status legend: [ ] pending · [~] in progress · [x] complete · [!] blocked

## Phase 0 — Technical Validation

- [~] M0 Environment & scaffold (Tauri 2 + Vue 3 + Tailwind 4, app launches)
- [ ] M1 Torrent engine (librqbit): magnet, .torrent, metadata, file listing/selection, pause/resume, stats, session mgmt, Tauri events
- [ ] M2 Progressive streaming: HTTP Range, localhost binding, token auth, piece prioritization
- [ ] M3 Integrated player: tauri-plugin-libmpv evaluation, HWND fallback path
- [ ] M4 End-to-end streaming + benchmarks (local seeder harness, legal test torrents)
- [ ] M5 Recovery: persisted sessions, playback position, interrupted metadata, partial integrity
- [ ] M6 Technical report (STREAMING_ENGINE.md, PLAYER_INTEGRATION.md, TECHNICAL_DECISIONS.md updates)

## Phase 1 — Application Foundation

- [ ] Screens: Home, Discover, Add Torrent, Torrent Details, Player, Download Manager, Media Library, Favorites, Settings, About
- [ ] Dark-first design system, keyboard navigation, states (loading/empty/error/success)

## Phase 2 — Torrent Management UX
## Phase 3 — Streaming Engine UX (states: connecting/metadata/buffering/playing/reconnecting/unavailable/error)
## Phase 4 — Player Controls (subs, audio tracks, speed, shortcuts, resume, aspect)
## Phase 5 — Catalog Provider Architecture + legal test content
## Phase 6 — Local Library (SQLite: history, continue watching, favorites)
## Phase 7 — Download Manager + cache management
## Phase 8 — Settings persistence
## Phase 9 — Security Hardening
## Phase 10 — Database schema + migrations (7 entities)
## Phase 11 — Testing (Rust unit/integration, Range, recovery, security, frontend, build)
## Phase 12 — Performance benchmarks vs targets
## Phase 13 — Windows Release (NSIS installer, LGPL notices, icon, metadata)

## Benchmark targets (agreed reference machine: i7-14700K / 31.8 GB / Win 11 26300)

- First frame: ≤ ~15 s on well-seeded legal test torrent
- Seek-to-resume: ≤ ~5 s when required pieces available
- Steady playback CPU: ~15% target
- Validation targets, not guarantees; actuals documented honestly.
