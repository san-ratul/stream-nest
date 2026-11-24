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

- Window: `transparent: true`; player surfaces leave transparency so the mpv layer shows through; all non-player UI paints opaque backgrounds.
- mpv initial options: `vo=gpu-next`, `hwdec=auto-safe`, `keep-open=yes`, `force-window=yes`.
- DLLs bundled via `bundle.resources: ["lib/**/*"]`: libmpv-2.dll = zhongfly LGPL build (2026-11-23), libmpv-wrapper.dll = plugin's wrapper.
- Frontend wrapper: `src/composables/usePlayer.js` (init/loadfile/pause/seek/volume/observe + dev telemetry).

## Positioning

Plugin exposes `set_video_margin_ratio` for insetting the video region; used by Phase 4/5 layouts if the player is embedded rather than full-window.

## Manual QA items (deferred to Phase 4 with real controls)

- Fullscreen transition, window resize during playback, DPI change mid-play, multi-display moves.
- External SRT (sub-add), audio track (aid), playback speed (speed property), resume (start property).

## Fallback status

Strategy B (custom child-HWND embed) not needed; retained as documented contingency in TECHNICAL_DECISIONS.md TD-002.
