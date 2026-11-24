# Development Progress Journal

Chronological log. Latest first. Includes: phase status, files touched, tests executed + results, blockers.

---

## 2026-11-24 — Live UX debugging session with user (HEVC stream)

Fixed in order of user reports:

1. **Black video, real HEVC torrent** — root cause: AppShell opaque `<main>` over the mpv layer (plus the earlier opaque body). Fix: player route transparency. Verified: pixel metrics 0%→25%→100% non-black; screenshot shows episode rendering.
2. **Seek drag doesn't work** — `:value` binding fought the drag. Fix: scrubbing state isolation (`@pointerdown/@input/@pointerup`).
3. **Video cut off under sidebar** — mpv filled the whole window. Fix: `setVideoMarginRatio` computed from the player container rect (ResizeObserver + window resize + fullscreen re-apply). Verified via mpv property read-back: `video-margin-ratio-left = 0.175` = sidebar ratio.
4. **Play button desync** — no initial `pause` sync after reloads. Fix: 1 Hz read-back of `pause` in the probe + read-back after toggle.
5. **"aspect: fill" write failure** — `video-aspect-override` has no "stretch"; option removed.
6. **Volume track invisible** — with `appearance-none`, WebKit only paints `::-webkit-slider-runnable-track`; styled explicitly.
7. **See-through window ("where is video gone?")** — restore replayed a stale stream URL (per-run port/token) and the transparent route showed the desktop. Fix: torrent URLs rebuilt from identity on restore (never replayed); opaque fallback layer under the player until `vo-configured`; load-failure message after 8 s.

Also verified real-swarm behavior: 580 MB torrent downloaded, hash-verified, played from complete file; file content probed from disk (valid MKV, HEVC + EAC3 tracks, 96% entropy).

Final installer rebuilt with all fixes and silently reinstalled (installed exe @ 18:21). Commits: afa08ab → 871fc82.

---

## 2026-11-24 — Post-MVP live testing with user (real-swarm HEVC episode)

- User streamed a real 580 MB HEVC/x265 MKV via magnet. Pipeline worked end-to-end (metadata, file detection, stream URL, timeline advancing, buffering) — but video was black. Found and fixed TWO layering regressions:
  1. AppShell `<main>` opaque background covered the mpv layer (Phase 1 regression; player route now `bg-transparent`).
  2. Confirmed by pixel-variance screenshots: avgLum 11.1, stddev 14.1, 25.1% non-black after fix vs ~0% before; visual screenshot shows the episode rendering in-app.
- Verification tooling added: `playerRenderDiagnostics` (vo/current/frames/display-rate), `estimated-vf-fps` observation, `playerReinit` (destroy+re-init with overrides), `forceSoftwareDecode` fix button (hwdec=no + bgra8 swapchain), session-persistent player state (survives reloads), auto-resume seek hardened with readiness retry, dev auto-stream harness (auto-opens persisted torrent on boot).
- Real-bug found by user's screenshot: "invoke is not defined" on AddTorrent success path → missing import, fixed.
- File-integrity probe of the completed torrent (direct disk read): valid EBML magic, HEVC + EAC3 CodecIDs, 96% non-zero entropy — content genuine; hash-verified by engine.
- Release installer rebuilt with all fixes; silent reinstall updates the installed app.

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
