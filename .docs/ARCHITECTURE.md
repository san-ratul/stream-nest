# Architecture

## Overview

StreamNest is a Windows-first desktop application. Tauri 2 hosts the OS window and WebView2 (Vue 3 UI). A Rust backend in the same process owns the torrent engine, the local streaming server, persistence, and the embedded libmpv player.

```
+----------------------------------------------------------+
| Tauri 2 window (Win32)                                   |
|  +----------------------------------------------------+  |
|  | WebView2 (Vue 3 + Tailwind 4 UI)                   |  |
|  |   pages: Home/Discover/AddTorrent/Details/Player/  |  |
|  |          Downloads/Library/Favorites/Settings/About|  |
|  +------------------------^---------------------------+  |
|                           | invoke() / events            |
|  +------------------------+---------------------------+  |
|  | Rust core (src-tauri/src)                          |  |
|  |  torrent/   librqbit Session facade, files, stats  |  |
|  |  streaming/ localhost HTTP Range adapter + token   |  |
|  |  player/    libmpv embed, playback control         |  |
|  |  storage/   SQLite (metadata only), settings       |  |
|  |  catalog/   provider trait + legal sources         |  |
|  |  events/    engine -> UI event bridge              |  |
|  +----------------------------------------------------+  |
+----------------------------------------------------------+
        |                            |
  BitTorrent network          filesystem (cache + downloads)
```

## Key properties

- **Engine-owned piece state.** librqbit manages pieces, priorities for streamed ranges, fastresume. StreamNest never persists piece data in SQLite (PRD §6.4).
- **Playback path.** UI gets a `http://127.0.0.1:<port>/<token>/stream/<...>` URL; libmpv plays it; engine serves bytes with Range support and prioritizes requested ranges. Seek = Range request = reprioritization.
- **Local-first, account-optional.** No network egress except BitTorrent traffic, tracker/DHT, and (later, Phase 5) user-approved catalog fetches. No telemetry.
- **Security model.** Streaming endpoint bound to 127.0.0.1 with per-session random token; Host/Origin validation; strict Tauri capabilities; sanitized filenames; path-traversal protection. Details in SECURITY.md.

## Module map (src-tauri/src)

Planned; created incrementally per milestone:

| Module | Responsibility | Introduced |
|---|---|---|
| `lib.rs` | Tauri builder, command registration | M0 |
| `commands/` | Tauri command handlers (thin, delegate to facades) | M1 |
| `torrent/` | librqbit session facade, metadata, file selection, stats | M1 |
| `streaming/` | Range adapter, token auth, binding | M2 |
| `player/` | mpv integration wrapper | M3 |
| `storage/` | SQLite, settings, cache mgmt | Phase 6/8/10 |
| `catalog/` | Provider trait, legal sources | Phase 5 |

## Frontend structure (src/)

| Dir | Responsibility |
|---|---|
| `components/` | Reusable UI (player controls, torrent list, cards) |
| `pages/` | The 10 PRD screens |
| `stores/` | App state (reactive stores, no external state lib unless justified) |
| `composables/` | Reusable logic (useTorrentEvents, usePlayer, useSettings) |
| `services/` | Tauri invoke/event wrappers |

## Data flow rules

1. UI never touches the filesystem directly; every privileged operation is a Tauri command.
2. Engine events fan out as Tauri events on channels the UI subscribes to (no polling for stats where events suffice).
3. All settings flow through the settings store persisted in SQLite (Phase 8/10).
