# Database

Status: Phase 10 COMPLETE — rusqlite (bundled SQLite, no system dependency) with versioned schema at `src-tauri/src/storage/mod.rs`. Phase 2 (discovery) added schema v2 — see [PHASE2_DISCOVERY.md](PHASE2_DISCOVERY.md).

## Implemented entities (v1)

| Table | Purpose |
|---|---|
| torrents | info_hash (unique), name, state, output_folder, added_at |
| torrent_files | per-torrent file index/path/size/video/selection (FK cascade) |
| playback_history | media_key UNIQUE upsert, position/duration, last_played_at (indexed) |
| library_items | media_key unique, title, media_type, added_at |
| favorites | media_key unique, title, created_at |
| app_settings | key/value store (Settings UI persists here; `tmdb:%` keys hold catalog detail markers + credits blobs) |
| catalog_cache | source payload cache (reserved for remote providers) |
| schema_version | migration anchor (v2 since Phase 2) |

Media key convention: `{info_hash}:{file_index}` for torrent video; `file:{path}` for local media;
`tmdb:movie:{id}` / `tmdb:tv:{id}` (optionally `:{season}:{episode}`) for catalog items.

## Implemented entities (v2 — Phase 2 discovery)

| Table | Purpose |
|---|---|
| movies | TMDB movie metadata, tmdb_id UNIQUE, title/popularity indexes |
| tv_shows | TMDB show metadata, tmdb_id UNIQUE, name/popularity indexes |
| genres | TMDB genres, UNIQUE(tmdb_id, media_type) |
| movie_genres / tv_show_genres | relationship join tables (composite PK, FK CASCADE) |
| tv_seasons | seasons per show, UNIQUE(tv_show_id, season_number), FK CASCADE |
| tv_episodes | episodes per season, UNIQUE(season_id, episode_number), FK CASCADE |
| catalog_collections | home sections (collection_key UNIQUE: trending_movies, popular_tv, documentaries, …) |
| catalog_collection_items | ordered membership, UNIQUE(collection_id, tmdb_id), FK CASCADE |
| sync_logs | provider sync runs: status running/completed/failed/interrupted, records, errors |

Migration policy: forward-only batches applied by version check; v1 tables untouched.
Upserts are idempotent by TMDB id; row ids resolved by natural-key SELECT (never last_insert_rowid
on the DO UPDATE path). Clearing the catalog (`catalog_clear`) removes v2 data + `tmdb:%` settings
keys and image cache only — personal tables are preserved.

## Engine state boundary

- Piece bitfields/availability: owned by librqbit (in-memory + its own JSON fastresume under `engine/session/`). Confirmed live: app restart restored torrents with partial downloads (93% Sintel resume observed).

## Technology decision

rusqlite + parking_lot mutex — embedded, synchronous access from Tauri commands, zero external services. WAL + foreign_keys ON at open. In-memory mode used by tests.

## Tests

`tests/storage_test.rs`: history upsert dedupe, favorites unique toggle, settings roundtrip, torrent upsert idempotency (4 tests).

