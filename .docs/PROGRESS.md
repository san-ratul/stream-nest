# Development Progress Journal

Chronological log. Latest first. Includes: phase status, files touched, tests executed + results, blockers.

---

## 2026-11-24 — Phases 7–9, 13 complete; MVP milestone chain closed

**Phase 7 (download manager/cache):** `storage_usage` command (recursive dir walk), storage card with bytes/files/path, subtitle cleanup command, cache-limit over-usage warning.

**Phase 8 (settings):** SettingsPage persists to SQLite (`app_settings`, debounced) and applies engine speed limits live via `session.ratelimits.set_download_bps/set_upload_bps` (verified API); per-torrent connection limit applied at add time from settings. Documented honestly: download-directory change needs an app restart (session dir fixed at init).

**Phase 9 (security):** security primitives extracted as testable functions — `sanitize_subtitle_name` (traversal stripping, hidden-file rejection, extension allowlist) and streaming `parse_range`; 13 adversarial unit tests; SECURITY.md rewritten with a verified-controls table + threat-model notes (token-in-path rationale, firewall consent, CSP).

**Phase 13 (release):** `npm run tauri build` → `StreamNest_0.1.0_x64-setup.exe` (39.5 MB, LZMA) with libmpv-2.dll (98.1 MB, LGPL) + wrapper bundled via resources. Verified: silent install exit 0 → `%LOCALAPPDATA%\StreamNest` with correct layout; uninstaller registered in ARP; installed app smoke-tested (engine initialized, plugin registered). Release binary smoke-tested in-place before install.

**Test totals:** 22 Rust tests green (13 unit incl. 8 range-parser + 5 sanitization adversarial; 3 catalog; 1 engine; 4 storage; 1 streaming end-to-end).

**Installer caveats (documented):** unsigned (SmartScreen warning), seeder.exe dev binary co-bundled (cleanup candidate), clean-OS test not possible (no VM).

**Final state:** installer available at `C:\rust-targets\streamnest\release\bundle\nsis\StreamNest_0.1.0_x64-setup.exe` and installed at `%LOCALAPPDATA%\StreamNest` for the user to try.

---

## 2026-11-24 — Phase 5 complete (catalog provider)

- `catalog/` module: CatalogProvider trait + BundledProvider (embedded resources/catalog.json), 3 provider tests.
- Discover UI: search, genre filters, gradient posters (no fabricated assets), license badges, official-page links only.
- Honest note in UI: torrent acquisition not bundled; provider contract ready for authorized sources.

---

## 2026-11-24 — Phases 6+10 complete (SQLite + library)

- rusqlite bundled; 7-entity schema v1 + schema_version anchor; WAL + FK enforced.
- Commands: db_upsert_torrent, history_upsert/list/remove, library_add/list/remove, favorites_add/list/remove, settings_set/get_all.
- Player: auto-resume (>30 s and <95 %), throttled position saves (5 s), final save on unmount, library registration on stream start.
- Home Continue-watching (real data + resume), Library (history + saved media with search/removal), Favorites (add from details page).
- 4 storage tests. Schema fix found by test: media_key needed UNIQUE for true upsert.

---

## 2026-11-24 — Phases 2–4 complete

(details as recorded earlier: peers panel, buffering detection via time-pos stalls + demuxer-cache-time, keyboard shortcuts, fullscreen capability, speed/aspect/mute, track selectors, external SRT import with sanitization)

---

## 2026-11-24 — M3 complete (player ACCEPTED)

## 2026-11-24 — M4 complete (progressive streaming ACCEPTED, video rendering fixed)

## 2026-11-24 — M1 + M2 complete

## 2026-11-24 — Environment & scaffold (M0)
