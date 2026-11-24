# Known Limitations

Honest ledger. Updated whenever a limitation is discovered or resolved.

## Current

1. No code signing — Windows SmartScreen will warn on the unsigned installer (no certificate exists).
2. librqbit lacks MSE/legacy transport encryption — some ISP-throttled environments may see reduced peer reachability vs qBittorrent/libtorrent clients.
3. tauri-plugin-libmpv is a single-maintainer project — mitigation: vendored/forked if adopted at M3.
4. Windows-only MVP — macOS/Linux not targeted.
5. Clean-OS installer verification depends on VM availability; otherwise tested on this machine only.
6. E: drive free space (18.4 GB) constrains keeping many historical build artifacts.

(Resolved items move to the bottom with date.)
