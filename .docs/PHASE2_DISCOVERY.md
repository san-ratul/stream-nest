# Phase 2 — Movie & TV Discovery (TMDB Integration)

Status: IMPLEMENTED on `feature/phase-2-movie-tv-discovery`. Metadata discovery, local catalog
synchronization, search, details pages, authorized content-source architecture.

## Principles

1. **Metadata ≠ rights.** TMDB supplies metadata only. A TMDB record is never treated as
   streamable. Playback requires an explicitly verified source (see `src-tauri/resources/authorized_sources.json`).
2. **Local-first.** All discovery UI reads the local SQLite catalog. The TMDB API is only
   contacted by the Rust backend during synchronization, on-demand details refresh, and image proxying.
3. **Credential isolation.** The API key never reaches the frontend, never appears in logs or
   errors, and is never committed (`.env` is gitignored; `.env.example` is the template).

## Architecture

```
Vue (frontend)                    Rust backend
────────────────────────────      ───────────────────────────────────────
HomePage / SearchPage      ──►    metadata::catalog_home / catalog_search
MovieDetailsPage           ──►    metadata::movie_details (on-demand refresh)
TvDetailsPage / episodes   ──►    metadata::tv_details / season_episodes
SettingsPage sync section  ──►    metadata::catalog_status / catalog_sync_start
poster <img> tags          ──►    http://127.0.0.1:{port}/{token}/tmdb/...  (image proxy)

metadata/
  config.rs   .env loader (TMDB_API_KEY; env var takes precedence)
  tmdb.rs     TmdbClient (HttpFetcher trait -> reqwest in prod, mock in tests)
  repo.rs     SQLite catalog repository (movies, tv, genres, seasons, episodes, collections)
  sync.rs     Synchronization engine + SyncState progress
  images.rs   axum image proxy with bounded disk cache
  sources.rs  ContentSourceProvider trait + BundledContentSources (curated JSON)
```

## TMDB API usage (official endpoints only)

- `/configuration` — image base URL + size lists (cached in `TmdbState.image_config`)
- `/trending/movie/day`, `/trending/tv/day`
- `/movie/popular|top_rated|upcoming|now_playing`
- `/tv/popular|top_rated`
- `/discover/movie|tv` — "latest" sections (date + vote-count filters) and Documentaries (`with_genres=99`)
- `/genre/movie/list`, `/genre/tv/list`
- `/movie/{id}?append_to_response=credits`, `/tv/{id}?append_to_response=credits`
- `/tv/{id}/season/{n}`
- `/search/multi` (adult excluded) — reserved for optional remote search; UI search is local-first
- Images: `https://image.tmdb.org/t/p/{size}/{path}` — fetched **only** by the backend proxy

Authentication: documented v3 `api_key` query parameter. Errors are mapped to
`TmdbError::{Auth, RateLimited, NotFound, InvalidResponse, Network, Timeout}`; 5xx and timeouts
retry up to 3 attempts with backoff; 429 honors `Retry-After` (capped at 30 s).

## API key configuration

1. Create `E:\streamnest\.env` (gitignored) with:
   ```
   TMDB_API_KEY=your_key_here
   ```
2. `.env.example` ships with an empty `TMDB_API_KEY=`.
3. Get a key at https://www.themoviedb.org/settings/api (free).
4. Restart the app. Settings → "Catalog & TMDB" shows `TMDB configured` when loaded.
5. Without a key the app runs in local-only mode: existing MVP features are unaffected and the
   UI shows an explanatory banner instead of empty discovery sections.

**Distribution note:** a key embedded in a shipped desktop app is NOT secret — anyone can
extract it from the binary. For a distributed build, run a server-side proxy that holds the key
and issues metadata/images to clients. StreamNest's architecture isolates the key in the Rust
backend, which is correct for local development; document this limitation before shipping.

## Database changes (schema v2)

New tables in `SCHEMA_V2` (`src-tauri/src/storage/mod.rs`), applied forward-only after `SCHEMA_V1`;
existing MVP tables are untouched. Reverse migration = drop the new tables (documented, not automated).

| Table | Notes |
|---|---|
| movies | `tmdb_id UNIQUE`, indexes on title + popularity |
| tv_shows | `tmdb_id UNIQUE`, indexes on name + popularity |
| genres | `UNIQUE(tmdb_id, media_type)`, CHECK movie/tv |
| movie_genres / tv_show_genres | composite PKs, FK CASCADE both sides |
| tv_seasons | `UNIQUE(tv_show_id, season_number)`, FK CASCADE |
| tv_episodes | `UNIQUE(season_id, episode_number)`, FK CASCADE, season index |
| catalog_collections | `collection_key UNIQUE` (trending_movies, popular_tv, documentaries, …) |
| catalog_collection_items | `UNIQUE(collection_id, tmdb_id)`, position index |
| sync_logs | status CHECK running/completed/failed/interrupted; stale `running` rows are marked `interrupted` by the next run |

Details-state markers (`tmdb:{type}:{id}:detail`) and credits blobs live in `app_settings`
under `tmdb:%` keys; `catalog_clear` deletes them together with the catalog rows.

**Idempotency rule:** every id-returning upsert resolves row ids by natural key SELECT after the
write — never `last_insert_rowid()`, which is unreliable on the `ON CONFLICT DO UPDATE` path.

## Synchronization flow

1. `catalog_sync_start` spawns `sync::run_sync` (async); UI polls `catalog_status`.
2. Image configuration → genres (movie+tv) → 11 collections in defined order.
3. Per collection: fetch up to N pages (`catalog.sync.pages` setting, 1–5, default 2 = 40 items),
   skip adult entries, commit items + collection membership **in one transaction** per collection.
   A failing collection is recorded and the sync continues; local data is never corrupted.
4. `sync_logs` row tracks start/end/status/records/errors. Cancel is checked between steps.
5. Re-running is safe: upserts are idempotent by TMDB id; collection membership is replaced, not appended.
6. Bulk sync stores list-level metadata only. Runtime/credits/seasons/episodes are fetched
   on demand (first details-page visit or manual refresh) and persisted for offline reuse.

## Authorized content sources

- `ContentSourceProvider` trait (`metadata/sources.rs`): `sources_for(media_type, tmdb_id)`.
- Bundled provider reads `resources/authorized_sources.json` — currently the four Blender
  Foundation open movies with **verified TMDB id mappings** (checked against the live API) and
  CC-BY license + official project page + attribution.
- All bundled entries are page-level (`playable_in_app: false`): details pages show an
  honest "No authorized in-app source" state with official-source links instead of fake play buttons.
- Adding an entry requires verified distribution rights AND a verified TMDB id mapping.
- Piracy-oriented sources are out of scope by design and MUST NOT be added.

## Image handling

- Local proxy: `GET /{token}/tmdb/{kind}/{size}/{file}` on 127.0.0.1 (random per-run token,
  Host header check — same model as the streaming server).
- Path validation: single filename, alphanumeric + `.jpg/.jpeg/.png/.webp/.svg`, no traversal.
- Disk cache under `{app data}/images` with LRU-by-mtime eviction at 512 MB (bounded growth).
- CSP: `img-src` gained `http://127.0.0.1:*` (local only). The frontend never contacts TMDB.

## Error handling

- Client errors surface in the UI (Settings sync panel, details pages) with retry actions.
- `TmdbError::Auth` messages never echo request material (the api_key travels in URLs, so
  reqwest errors are stripped of URLs before formatting).
- Missing optional TMDB fields default through serde; a bad payload maps to `InvalidResponse`.

## Testing

61 tests total (all green; existing 22 MVP tests untouched):

- `metadata::tmdb` (9, in-module, mocked `HttpFetcher`): success parse + defaults, auth failure
  (no key leakage), invalid JSON, 429 with Retry-After, 5xx retry count, pagination, image config defaults, credits parse, URL encoding.
- `tests/catalog_db_test.rs` (11): movie/tv upsert idempotency, runtime/genres/credits persistence,
  season+episode relationships, unique constraints, FK enforcement, collection ordering,
  search incl. documentaries, sync-log lifecycle (stale runs → interrupted), clear-catalog
  personal-data preservation, schema version idempotency.
- `tests/metadata_sync_test.rs` (10, mocked HTTP): full sync + re-sync idempotency, in-place
  updates, multi-page pagination, per-collection failure isolation, all-fail reporting without
  corruption, cancel, on-demand details + force refresh, season episodes fetch-once, local-only no-key behavior, sync status counts.

Tests require no real TMDB key or network access.

## Known limitations

- Search is local-only; remote TMDB search lands in the catalog only after a sync (the client
  method exists but the UI deliberately stays local-first).
- Ratings/images come from TMDB and are shown as metadata, never as availability guarantees.
- Episode "watched" badges derive from playback history (`tmdb:tv:{id}:{s}:{e}` keys); with no
  authorized playable source, nothing writes those keys today — the plumbing is ready for
  future verified sources.
- Bundled authorized sources are page-level; no in-app playback for catalog titles yet.
- A key in a distributed binary is not secret (see Distribution note above).
