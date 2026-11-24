# Database

Status: schema design scheduled for Phase 10 (implementation Phase 6+). Engine piece state never stored here.

## Planned entities (PRD §6.4)

| Table | Purpose |
|---|---|
| torrents | Torrent sessions (info-hash, name, state, destination, added_at) |
| torrent_files | Per-file metadata (index, path, size, video flag, selection) |
| playback_history | Positions, duration, timestamps, media ref |
| library_items | Local media library entries |
| favorites | User favorites |
| app_settings | Key/value settings |
| catalog_cache | Cached catalog metadata (source, fetched_at, payload) |

## Engine state boundary

- Piece bitfields/availability: owned by librqbit (in-memory + its own fastresume files).
- StreamNest persists only session references + user-facing state needed for recovery and library features.

## Technology choice

rusqlite vs sqlx-sqlite: decided at Phase 10 kickoff with a verification spike (async needs are minimal; likely rusqlite + a mutex/queue). Migration mechanism: schema_version table + ordered migrations.
