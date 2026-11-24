# Streaming Engine

Status: design locked (TD-005); implementation M2; measurements M4.

## Approach

librqbit streams a selected file via its Range-capable HTTP interface, prioritizing pieces for requested ranges and blocking until pieces are available. StreamNest wraps it:

- Bind 127.0.0.1 only; random token path segment; Host/Origin checks (Phase 9).
- UI + player consume `http://127.0.0.1:<port>/<token>/stream/...`.
- Seek = new Range request = engine reprioritization (no custom scheduler).
- Concurrency: bounded session count with resource limits (default cap decided at Phase 3).

## Playback state machine (Phase 3 UI contract)

`connecting → retrieving_metadata → preparing_stream → buffering → playing ⇄ buffering → reconnecting | unavailable_source | error`

Buffer/availability numbers come from engine stats (have bytes, needed bytes, speed) — never fabricated.

## Benchmarks (M4, reference machine i7-14700K)

Targets: first frame ≤ ~15 s well-seeded; seek-to-resume ≤ ~5 s when pieces available; steady CPU ~15%. Actuals recorded here after M4.
