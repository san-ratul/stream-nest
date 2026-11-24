# Testing

Status: plan locked; suite grows per milestone; honest results only.

## Categories (PRD Phase 11)

1. Rust unit tests — magnet parsing, file-type detection, token auth, sanitization, settings logic.
2. Torrent engine integration — add/pause/resume/status against local seeder harness.
3. HTTP Range tests — correctness of Range semantics on the streaming endpoint.
4. Streaming tests — progressive start, seek reprioritization, stall/recovery.
5. Player integration — manual + scripted checks on the M3 matrix.
6. SQLite persistence — migrations, CRUD, recovery of sessions/history.
7. Recovery — restart during playback/metadata; partial-file integrity.
8. Frontend — component-level where practical (Vitest if adopted; kept minimal per dependency discipline).
9. Security — token/Host/Origin/traversal adversarial tests.
10. Windows build — release build + NSIS installer on this machine.

## Failure scenarios covered

No seeders · peer disconnect · slow network · interrupted metadata · invalid magnet · corrupted .torrent · missing media file · seek into unavailable pieces · restart during playback · cache exhaustion.

## Deterministic harness

Local seeder (librqbit itself) serving legal fixtures (Blender open movies / archive.org public domain) with controllable bandwidth/pauses. Network tests marked `#[ignore]` by default where they need live swarms; run explicitly.
