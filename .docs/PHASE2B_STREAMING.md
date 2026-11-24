# Phase 2B — End-to-End Movie & TV Streaming

Status: IMPLEMENTED on `feature/phase-2-movie-tv-discovery`. Connects the existing torrent engine,
streaming server and libmpv player to the Phase 2 authorized-source architecture.

## Principles

- **Reuse only.** No second torrent engine, streaming server, player or history store. The
  streaming workflow orchestrates the existing services (`engine.rs`, `streaming.rs`,
  `usePlayer.js`, `history_upsert`).
- **Authorized sources only.** Playback requires an entry in
  `resources/authorized_sources.json` with a verified rights basis. TMDB metadata never makes a
  title playable. Piracy indexes are excluded by design.
- **Progressive streaming.** Playback starts as soon as data flows; full download is never
  required. Seeking into incomplete regions uses the existing librqbit piece-prioritization path.

## Architecture map (what reuses what)

| Concern | Existing service | Streaming integration |
|---|---|---|
| Torrent engine | `engine.rs` `EngineShared` (librqbit Api) | `playback::prepare_core` adds via `api_add_torrent` |
| Streaming server | `streaming.rs` (axum, token + Range/206) | `stream_url` command reused as-is |
| Player | `usePlayer.js` + `PlayerPage.vue` | `openInPlayer` with a `tmdbKey` context |
| History | `history_upsert` / `history_list` | keys `tmdb:movie:{id}` / `tmdb:tv:{id}:{s}:{e}` |
| Sources | `metadata/sources.rs` `ContentSourceProvider` | extended `AuthorizedSource` (below) |
| Video selection | new `metadata/video.rs` | classification + episode matching |
| Sessions | new `playback::PlaybackRegistry` | ownership tracking only |

## Source provider extension (`AuthorizedSource`)

New optional fields (serde defaults; older JSON still parses):

- `source_kind`: `official_torrent` | `official_page` | `dev_fixture`
- `magnet_uri`: rights-holder-published magnet (btih hex or base32)
- `torrent_file` + `initial_peers`: dev-fixture local torrent (deterministic seeder on 127.0.0.1:6900)
- `quality` / `size_label` / `format` / `language`: shown as-is; omitted when absent ("Unknown" is never fabricated)
- `available`: runtime flag — dev fixtures are hidden when their torrent file does not exist

Current curated entries (all CC-BY, Blender Foundation):

| Title | TMDB | Source |
|---|---|---|
| Big Buck Bunny | 10378 | official torrent (btih `dd8255ec…`) + local dev fixture (10 s sample) |
| Sintel | 45745 | official torrent (btih `08ada5a7…`) + local dev fixture |
| Tears of Steel | 133701 | official torrent (btih `209c8226…`) |
| Cosmos Laundromat | 358332 | official page only (no verified in-app torrent) |

The btih hashes above are the officially published Blender Foundation torrents (widely used as
canonical legal test torrents). Attribution and license links are shown on every source card.

## Playback flow (`composables/playback.js` + `playback.rs`)

```
Details page ──Watch──► SourcePanel (provider/quality/size/language/rights)
    │ select source
    ▼
playback_prepare_uri / _file            (Rust, emits "playback:stage": connecting|metadata|files)
    ├─ add torrent (reuse if already managed by the engine)
    ├─ wait metadata (poll torrent details, 120 s timeout, honest error on timeout)
    ├─ classify files (video.rs): videos kept, sample/trailer/featurette/bonus filtered
    └─ newly-added torrent: restrict only_files to the video (+ .srt/.ass/.vtt/.ssa)
    ▼
single video  → auto-select        multiple videos → explicit file chooser (never silent)
TV episode    → episode_matches (S01E01/S1E1/1x01 tokens, confidence ≥ 80 + clear winner → auto;
                otherwise an explicit match/file chooser)
    ▼
playback_begin (registry) → stream_url → openInPlayer({src, title, infoHash, fileId, tmdbKey})
    ▼
PlayerPage: libmpv loads the stream; progress saved via history_upsert (existing 5 s-delta logic)
```

### Errors (user-facing, mapped in `friendlyError`)

| Situation | Message |
|---|---|
| Invalid magnet/source | "This source is invalid" |
| Metadata timeout / no peers | "Could not retrieve source metadata." / "No peers are currently available…" |
| No video file | "No playable video file found in this source" |
| Engine not ready | "The torrent engine is still starting…" |
| Missing dev fixture | "Dev fixture torrent file is missing." |

Detailed technical errors stay in logs; tokens and full magnet URLs are never logged.

## Session ownership & cleanup

- `PlaybackRegistry` (managed state) records `{media_key, info_hash, file_id, source_id}` per playback.
- **Playback never stops, pauses or re-configures torrents.** Ending playback only removes the
  registry entry (PlayerPage `onUnmounted` → `playback_end`) and saves progress. User-managed
  downloads continue untouched; the torrent remains in the engine session (existing fastresume model).
- Duplicate prevention: librqbit returns the already-managed torrent for repeated adds; prepare
  detects reuse and **does not touch** `only_files` of user-managed torrents.
- Playback-owned fresh torrents (e.g. season packs) restrict `only_files` to the streamed video +
  subtitle side files, applied after the torrent leaves the `initializing` state (librqbit rejects
  the update while initializing).
- Restart recovery: torrents persist via the existing session JSON; the player rebuilds stream URLs
  from torrent identity (existing `restorePlayer`); TMDB context survives via `sessionStorage`.

## Playback progress keys

| Content | media_key | Written by |
|---|---|---|
| Torrent video (MVP) | `{info_hash}:{file_index}` | PlayerPage |
| Catalog movie | `tmdb:movie:{id}` | PlayerPage (`tmdbKey`) |
| Catalog episode | `tmdb:tv:{id}:{season}:{episode}` | PlayerPage (`tmdbKey`) |

- Saved every ≥ 5 s delta, on pause, on close, and on app close (existing logic, reused).
- Completion threshold: **≥ 95 % of duration marks an episode watched** (documented; a row above
  95 % disappears from Continue Watching but stays in history).
- Continue Watching (Home): poster + episode tag (`S01E02`) + remaining time via
  `catalog_card_lookup` (local-only). Clicking a TMDB row re-opens the details page — the source is
  re-verified there, and the player auto-resumes from the saved position because the history key
  matches. Torrent rows keep the MVP direct-resume behavior.
- Next episode: the player shows "Next episode ▶" only when the current key is
  `tmdb:tv:{id}:{s}:{e}`; it routes to the show's details page for that episode (authorized source
  re-check + episode match), never silently switching files.

## Player UI additions

- Buffering: existing mpv cache-time indicator; torrent truth (percent, ↓/↑ speeds, buffer-ahead
  seconds) shown as a compact chip from the existing 1 Hz `engine:torrents` broadcast — no fake
  percentages, no extra polling.
- Controls, keyboard shortcuts, tracks, subtitles: unchanged MVP behavior.

## Testing

78 automated tests green (22 pre-existing MVP tests untouched), including:

- `playback.rs` in-process pipeline test (deterministic, no network): seeder session creates a
  season-pack torrent (episode + sample + cover) → `prepare_core` → metadata → classification
  (extras filtered) → fresh-add `only_files` restriction → duplicate add reuse (selection preserved)
  → episode match confidence → registry begin/end (double-end no-op).
- `video.rs`: extension checks, extras markers, SxxExx/S1E1/1x01/double-episode tokens, episode
  matching with confidence + ambiguity rules.
- `sources.rs`: magnet entries well-formed, Blender films have playable official torrents,
  unavailable dev fixtures hidden, page-level fallback present.
- Existing streaming tests (Range/206/token) unchanged.

## Manual QA (performed with the local deterministic seeder, legal content)

1. `cargo run --bin seeder E:\streamnest\.testmedia 6900` → `SEEDER_READY`.
2. Dev app → Sintel details (TMDB 45745) → SourcePanel shows the dev fixture (labeled) → Watch.
3. Stages: connecting → metadata → files → stream starts before download completes.
4. Pause/resume, ±10 s seek, seek into undownloaded region (buffering indicator reflects reality).
5. Progress persists; Continue Watching shows the row with poster; Resume re-enters the flow.
6. Recovery: close player/app mid-stream, relaunch — no corrupted state, torrent session intact.

## Known limitations

- No authorized TV sources exist yet: episode Play shows the honest unavailable state. The
  episode-matching machinery is fully implemented and tested (season packs via the in-process test).
- Official torrents rely on public swarms + trackers in the magnet + DHT (release build); peers
  vary by title/time. Errors surface honestly with retry.
- `dev_read_test_torrent` is debug-only; dev fixtures cannot stream in release builds.
- Watched-status derives from position/duration history only; no separate watched table.
