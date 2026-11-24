# Roadmap

Status legend: [ ] pending · [~] in progress · [x] complete · [!] blocked

## Phase 0 — Technical Validation — [x] COMPLETE

- [x] M0 Environment & scaffold (Tauri 2 + Vue 3 + Tailwind 4, app launches)
- [x] M1 Torrent engine (librqbit): magnet, .torrent, metadata, file listing/selection, pause/resume, stats, session mgmt, Tauri events
- [x] M2 Progressive streaming: HTTP Range, localhost binding, token auth, piece prioritization
- [x] M3 Integrated player: tauri-plugin-libmpv ACCEPTED (vo=gpu fix)
- [x] M4 End-to-end streaming + benchmarks (1.1 GB throttled seeder; first frame 276–374 ms mid-download)
- [x] M5 Recovery: fastresume verified live (93% Sintel resume); session persistence in SQLite
- [x] M6 Technical report (STREAMING_ENGINE.md, PLAYER_INTEGRATION.md, TECHNICAL_DECISIONS.md)

## Phase 1 — Application Foundation — [x] COMPLETE
- [x] Screens: Home, Discover, Add Torrent, Torrent Details, Player, Download Manager, Media Library, Favorites, Settings, About (+ /dev harness)
- [x] Dark-first design system, keyboard navigation (Alt+1..8), states (loading/empty/error/success)

## Phase 2 — Torrent Management UX — [x] COMPLETE
## Phase 3 — Streaming Engine UX (buffering/cache reporting) — [x] COMPLETE
## Phase 4 — Player Controls — [x] COMPLETE (resume playback completed in Phase 6)
## Phase 5 — Catalog Provider Architecture + legal content — [x] COMPLETE
## Phase 6 — Local Library (SQLite: history, continue watching, favorites) — [x] COMPLETE
## Phase 7 — Download Manager + cache management — [x] COMPLETE
## Phase 8 — Settings persistence (SQLite) + engine limit application — [x] COMPLETE
## Phase 9 — Security Hardening — [x] COMPLETE (13 adversarial tests, SECURITY.md review)
## Phase 10 — Database schema + migrations (7 entities) — [x] COMPLETE
## Phase 11 — Testing — [x] MVP SUITE COMPLETE (22 tests; live-swarm failure scenarios deferred as #[ignore]-style manual items)
## Phase 12 — Performance benchmarks vs targets — [x] MEASURED (see STREAMING_ENGINE.md)
## Phase 13 — Windows Release — [x] COMPLETE (installer built + verified; unsigned)

## Phase 14 — Movie & TV Discovery (TMDB) — [x] IMPLEMENTED (branch feature/phase-2-movie-tv-discovery)

- [x] TMDB client (official API, retries, rate-limit, timeouts, mockable; 61 tests green)
- [x] Schema v2: movies, tv_shows, genres (+joins), tv_seasons, tv_episodes, catalog_collections(+items), sync_logs
- [x] Sync engine: 11 collections, paginated, per-collection transactions, cancel + resume-safe, sync_logs
- [x] Local image proxy (127.0.0.1 + token, bounded 512 MB cache), CSP img-src local-only relaxation
- [x] Home discovery sections + Continue Watching preserved; Search (debounced, local-first, filters)
- [x] Movie details (cast/crew, sources), TV details (seasons/episodes, watched badges)
- [x] Authorized content-source provider trait + curated rights-verified entries (no fake playables)
- [x] Settings sync panel: status, progress, cancel, scope (pages), clear catalog (personal data kept)
- [ ] Future: in-app playback via additional rights-verified sources; remote TMDB search as opt-in

## Benchmark targets vs measured (reference: i7-14700K / 31.8 GB / Win 11 26300)

- First frame: target ≤ ~15 s well-seeded → measured 276–374 ms (1.1 GB @ 3 MB/s throttle) — PASS
- Seek-to-resume: target ≤ ~5 s when pieces available → measured ~5 s piece-fetch stall mid-download; property response 3–7 ms — PASS (borderline)
- Steady playback CPU: target ~15% → measured ~7% of one core (1080p hwdec) — PASS
