# Database

Status: Phase 10 COMPLETE — rusqlite (bundled SQLite, no system dependency) with versioned schema at `src-tauri/src/storage/mod.rs`.

## Implemented entities (v1)

| Table | Purpose |
|---|---|
| torrents | info_hash (unique), name, state, output_folder, added_at |
| torrent_files | per-torrent file index/path/size/video/selection (FK cascade) |
| playback_history | media_key UNIQUE upsert, position/duration, last_played_at (indexed) |
| library_items | media_key unique, title, media_type, added_at |
| favorites | media_key unique, title, created_at |
| app_settings | key/value store (Settings UI persists here) |
| catalog_cache | source payload cache (reserved for remote providers) |
| schema_version | migration anchor (v1) |

Media key convention: `{info_hash}:{file_index}` for torrent video; `file:{path}` for local media.

## Engine state boundary

- Piece bitfields/availability: owned by librqbit (in-memory + its own JSON fastresume under `engine/session/`). Confirmed live: app restart restored torrents with partial downloads (93% Sintel resume observed).

## Technology decision

rusqlite + parking_lot mutex — embedded, synchronous access from Tauri commands, zero external services. WAL + foreign_keys ON at open. In-memory mode used by tests.

## Tests

`tests/storage_test.rs`: history upsert dedupe, favorites unique toggle, settings roundtrip, torrent upsert idempotency (4 tests).

