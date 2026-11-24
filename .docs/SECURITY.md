# Security

Status: Phase 9 hardening complete for MVP scope; enforcement verified through tests where practical.

## Implemented controls (verified)

| Control | Implementation | Verification |
|---|---|---|
| Streaming server binds 127.0.0.1 only | `streaming.rs`: `TcpListener::bind("127.0.0.1:0")` | logs show 127.0.0.1 binding |
| Random endpoint token | UUID v4 simple (32 hex) per app run; path segment required | streaming_test: wrong token → 404 |
| Host header validation | allowlist `127.0.0.1:{port}` / `localhost:{port}` only | streaming_test: foreign Host → 403 |
| Range semantics | strict parser: malformed prefix/end-before-start/start≥len → 416; open-ended + suffix per rqbit semantics | 8 unit tests on parse_range |
| Subtitle import sanitization | filename-only extraction (path traversal components stripped), hidden-file rejection, extension allowlist (srt/ass/vtt/ssa), 2 MiB cap | 5 unit tests incl. traversal cases |
| Tauri capabilities minimal | `core:default` + window fullscreen + `libmpv:default` only; no fs/shell/dialog plugins | capabilities/default.json audit |
| No arbitrary frontend FS access | frontend reaches disk only through typed commands (import_subtitle, dev_read_test_torrent[debug-only]) | command review |
| Debug-only dev surface | `dev_read_test_torrent` compiled out of release (`#[cfg(debug_assertions)]`) | lib.rs cfg gate |
| Settings/DB under app data dir | `%APPDATA%/app.streamnest.desktop/` | runtime paths logged |
| No telemetry / no history egress | no HTTP client calls except BitTorrent/tracker/DHT via engine | code review; library has no outbound analytics |
| Path traversal from torrent metadata | delegated to librqbit storage layer (sanitizes paths internally); app never constructs paths from raw metadata | engine boundary |
| Untrusted metadata parsing | bencode handled by librqbit (fuzz-hardened upstream); app stores only engine-provided structs | engine boundary |

## Threat-model notes

- Token-in-path is the primary streaming-auth defense; Host check blocks misrouted requests. Browser drive-by requests cannot know the token. Timing-safe comparison deliberately not used: token is a fresh 128-bit UUID per run, threat is loopback-only, and observable timing over loopback is impractical. Revisit if the server ever binds beyond loopback.
- Release builds bind public torrent listeners (BitTorrent requirement) → Windows Firewall prompt on first run is expected user consent; debug builds avoid it entirely (TD-008).
- CSP (tauri.conf.json) restricts sources; media/connect limited to `self` + `127.0.0.1:*`.

## Deferred (documented, not silent)

- Code signing (no certificate) — SmartScreen warning on installer.
- Secure auto-updater (Tauri updater) — post-MVP; installer is the only update channel now.
- engine:torrents events carry no sensitive data beyond torrent names the user added locally.
