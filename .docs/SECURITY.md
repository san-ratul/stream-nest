# Security

Status: requirements locked; enforcement lands across M2 (streaming auth), Phase 9 (hardening), Phase 11 (security tests).

## Mandatory controls (from PRD §8)

1. Local-first; no mandatory account; no telemetry.
2. Streaming server binds 127.0.0.1 ONLY.
3. Random per-session token in URL path; requests without it rejected.
4. HTTP `Host` and `Origin` validation on the streaming server.
5. Torrent filename sanitization + path traversal protection (files stay inside the configured cache/download roots).
6. Untrusted torrent metadata parsed defensively (bencode structures bounded; no code execution paths).
7. Tauri capabilities restricted per feature; frontend has no arbitrary FS access.
8. Settings/DB under user app-data dir with restrictive defaults.
9. Diagnostics opt-in only.
10. No torrent history leaves the machine.

## Verification plan (Phase 11 security tests)

- Range/auth: request without token → 401/404; forged Host → rejected; cross-origin request → rejected.
- Path traversal: crafted filenames/paths from hostile .torrent files rejected (unit tests with adversarial fixtures).
- Capabilities audit: review `src-tauri/capabilities/*.json` against features actually used.
