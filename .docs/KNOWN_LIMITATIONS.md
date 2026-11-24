# Known Limitations

Honest ledger. Updated whenever a limitation is discovered or resolved.

## Current

1. No code signing — Windows SmartScreen will warn on the unsigned installer (no certificate exists).
2. librqbit lacks MSE/legacy transport encryption — some ISP-throttled environments may see reduced peer reachability vs qBittorrent/libtorrent clients.
3. tauri-plugin-libmpv is a single-maintainer project — mitigation: vendored/forked if adopted at M3 (adopted; vendoring decision at Phase 9 security review).
4. Windows-only MVP — macOS/Linux not targeted.
5. Clean-OS installer verification depends on VM availability; otherwise tested on this machine only.
6. Release builds bind public torrent listeners → Windows Firewall prompt on first run (user consent required; debug builds avoid this via loopback-only listeners).
7. `vo=gpu-next` cannot be used for embedded playback (black surface on foreign HWND) — locked to `vo=gpu` (TD-008).
8. Seek telemetry measures property-response latency; honest mid-download seek latency is the piece-fetch stall (~5 s measured, documented in STREAMING_ENGINE.md).
9. E: drive free space constrains repo-resident artifacts; cargo target dir lives on C: (TD-009).

(Resolved items move to the bottom with date.)

## Resolved

- 2026-11-24: MSVC toolchain absent → installed via winget (user-approved elevation).
- 2026-11-24: opaque body background hid mpv video → transparent body + vo=gpu fix.
