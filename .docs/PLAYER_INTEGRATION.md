# Player Integration

Status: M3 ACCEPTED with tauri-plugin-libmpv (Strategy A). Full controls land in Phase 4.

## Evaluation outcome

`tauri-plugin-libmpv` v0.3.2 (MPL-2.0) adopted. Verified working on this machine (2026-11-24):

| Criterion | Result |
|---|---|
| Plugin loads libmpv-wrapper.dll + libmpv-2.dll | OK (auto-copied to target dir in dev) |
| mpv instance in main window (wid mode, transparent webview) | OK |
| Playback | OK — 10 s 1080p H.264 clip played 0→9.98 s smoothly (probe telemetry 1 Hz) |
| Hardware decoding | OK — `hwdec-current = d3d11va` (auto-safe) |
| Codec/container detection | OK — `video-format = h264` |
| EOF behavior (keep-open) | OK — holds last frame |
| Instance lifecycle (init/destroy/HMR reload) | OK — clean destroy logged |
| Capability gating | `libmpv:default` added to capabilities/default.json |

## Configuration shipped

- Window: `transparent: true`; **html/body background must be transparent** (style.css) — an opaque body hides the mpv layer entirely (found via user report: "sound but no video"; screen capture confirmed).
- **AppShell content area must be transparent on the player route** (`:class="route.path === '/player' ? 'bg-transparent' : 'bg-[#0b0f17]'"`). The Phase 1 shell refactor introduced an opaque `<main>` that covered the mpv layer and produced "audio only, black video" for ALL content — diagnosed via pixel-variance screenshots after decoder/swapchain experiments (hwdec=no, bgra8) all failed to change anything. Rule: **no opaque HTML surface may cover the player region.**
- **vo=gpu, NOT gpu-next**: libplacebo (gpu-next) presents a black surface when embedded into a foreign HWND on Windows; classic `vo=gpu` (D3D11) renders correctly in wid mode. Verified by on-screen screenshot. Keep for all future mpv option work.
- mpv initial options: `vo=gpu`, `hwdec=auto-safe`, `keep-open=yes`, `force-window=yes`, `d3d11-output-format=bgra8` (8-bit swapchain hardening for 10-bit HEVC sources).
- DLLs bundled via `bundle.resources: ["lib/**/*"]`: libmpv-2.dll = zhongfly LGPL build (2026-11-23), libmpv-wrapper.dll = plugin's wrapper.
- Frontend wrapper: `src/composables/usePlayer.js` (init/loadfile/pause/seek/volume/observe + first-frame/seek/render telemetry, re-init support).
- Tauri capability `libmpv:default` required in capabilities/default.json.

## Positioning

Plugin exposes `set_video_margin_ratio` for insetting the video region; used by Phase 4/5 layouts if the player is embedded rather than full-window.

## Manual QA items (deferred to Phase 4 with real controls)

- Fullscreen transition, window resize during playback, DPI change mid-play, multi-display moves.
- External SRT (sub-add), audio track (aid), playback speed (speed property), resume (start property).

## Fallback status

Strategy B (custom child-HWND embed) not needed; retained as documented contingency in TECHNICAL_DECISIONS.md TD-002.
