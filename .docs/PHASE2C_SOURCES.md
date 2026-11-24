# Phase 2C — Universal Source Management & Manual Input

Status: IMPLEMENTED on `feature/phase-2-movie-tv-discovery`. Manual magnet/URL/file/page source
input, persisted source library, secure remote torrent fetching, TMDB association.

Extends [PHASE2B_STREAMING.md](PHASE2B_STREAMING.md) — the playback pipeline, session registry,
progress keys and player are reused unchanged.

## Input types (unified Add Source, `/add`)

| Tab | Validation | Backend |
|---|---|---|
| Magnet link | `magnet:?xt=urn:btih:` hex (40) or base32 (32) | `source_add_magnet` → btih parsed, `dn`/`tr` extracted |
| Torrent URL | http(s) only | `source_add_torrent_url` → SSRF-safe fetcher (below) → bencode validation |
| Torrent file | bencode validation | `source_add_file_base64`; a copy is persisted, the original file is untouched |
| Provider page | adapter registry | `source_resolve_page` — currently Internet Archive item pages only |

Unsupported provider pages return exactly:
*"This provider page is not supported. Try a direct Magnet Link or .torrent URL."*

**No copyright classification**: manual sources are never scanned, fingerprinted or blocked by
content type. Technical/security validation only. Association with a TMDB movie/episode is
user-driven metadata, not a rights statement.

## Secure torrent URL fetching (`src-tauri/src/fetch.rs`)

1. Scheme allowlist: http/https only (`file:`, `ftp:` rejected).
2. DNS resolution is validated: loopback, RFC1918, link-local (incl. `169.254.169.254` cloud
   metadata), CGNAT 100.64/10, benchmark 198.18/15, multicast, reserved, IPv6 ULA/fe80::/10 and
   IPv4-mapped IPv6 are refused.
3. **Connection pinning**: the request goes only to the DNS-vetted address
   (`reqwest.resolve(host, addr)`), defeating DNS rebinding; SNI/hostname stays intact.
4. Redirects are followed manually (policy = none), max 5 (configurable), each hop re-resolving
   scheme + DNS + IP class; relative locations supported.
5. Response streamed with a hard byte cap (default 10 MiB, configurable) + declared-size precheck.
6. Timeouts: connect 10 s, total 30 s (configurable in Settings → Source management).
7. TLS verification is never disabled.
8. Payload validated as bencode metainfo via librqbit (`torrent_from_bytes` + `validate()` —
   includes path-traversal rejection inside the torrent).
9. Persisted copies live under `{app data}/sources/{epoch}_{basename}` (sanitized name; no
   traversal, no hidden files). Downloaded files are never executed.

## Source library (`media_sources`, schema v3)

- Table `media_sources`: `info_hash UNIQUE` (nullable until metadata arrives), `source_type`
  CHECK (magnet/torrent_url/torrent_file/provider_page/official), display name, original input
  (magnet/URL/persisted copy path), `provider_key`, TMDB association (`tmdb_media_type`, `tmdb_id`,
  `season_number`, `episode_number`), `file_index`, `total_size`, `trackers`, `metadata_json`,
  `date_added`, `last_used_at`. Indexes on association + recency. Reverse migration: `DROP TABLE`.
- Duplicate handling: upsert by info hash — re-adding updates the record (name/metadata/association)
  instead of duplicating; the engine reuses the active torrent (librqbit AlreadyManaged).
- History: `last_used_at` updated on every prepare/play; list sorted by last-used then added.
  Separated from playback history (`playback_history` table) by design.
- Association: at add-time (details pages pass `?type=&id=&season=&episode=`), via the Sources
  page editor (searches the local catalog), or cleared. Episode association preserved for
  `tmdb:tv:{id}:{s}:{e}` progress keys.

## Commands

`source_add_magnet`, `source_add_torrent_url`, `source_add_file_base64`, `source_resolve_page`,
`source_list`, `source_get`, `source_for_tmdb`, `source_remove`, `source_associate`, `source_touch`,
`source_check_engine` (is the torrent already active → fast Continue-Watching reuse), and
`playback_prepare_record` (adds the stored source to the engine via the existing `prepare_core`;
paths are resolved inside Rust, never accepted from the frontend).

## Pages

- **Add Source** (`/add`, nav "Add Source"): 4 tabs, live validation states, fetching spinner,
  saved preview (name/hash/size/trackers where known), metadata retrieval step with the detected
  video list, file chooser for ambiguity, Start streaming. Context chip shows the TMDB link.
- **Sources** (`/sources`, nav "Sources"): filters (all / recently added / recently used / linked),
  Play, details toggle, copy magnet, retry metadata, associate/clear link, remove.
- **Details pages**: "Your linked sources" block (ManualSources component) on movie + TV pages,
  episode-level "+ Source" button, add-source flows pre-associated.
- **Settings → Source management**: max URL size, max redirects, timeout.

## Automatic discovery scope

The only automatic provider is the existing curated `ContentSourceProvider` (Blender official
torrents + fixtures) shown on details pages. **No piracy-index scrapers and no generic website
scraping exist or will be added**; the provider architecture accepts future explicitly-permitted
integrations (official APIs / authorized catalogs).

## Testing

89 automated tests green (22 pre-existing MVP tests untouched). New coverage:
- `fetch.rs`: forbidden-IP matrix (v4/v6 incl. v4-mapped + cloud metadata), scheme validation,
  redirect resolution + scheme rejection, bencode validation accept/reject cases.
- `source_library.rs`: duplicate upsert, association + episode linkage, touch, remove, magnet
  param extraction (dn/trackers + URL decoding), filename sanitization.
- Regression: engine/pipeline/streaming/catalog/metadata suites unchanged and passing.

## Known limitations

- Internet Archive is the only supported provider-page adapter; it requires the item to expose a
  `.torrent` file in its metadata. IA contains mixed-rights content — the user chooses the item
  manually; no rights assertions are made by the app.
- No automatic provider-search beyond the curated catalog (permitted integrations only).
- Torrent URL fetching has no persistent allowlist/blocklist UI beyond the settings above.
- Sources whose metadata was never retrieved show "hash unknown" until first use.
