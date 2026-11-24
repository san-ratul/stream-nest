# Player Integration

Status: strategy locked (TD-002); implementation M3; this file records the evaluation outcome and constraints.

## Plan

1. Evaluate `tauri-plugin-libmpv` (MPL-2.0, v0.3.2): embed via transparent window; requires libmpv-2.dll (LGPL, zhongfly mpv-dev-lgpl build) + libmpv-wrapper.dll bundled under `src-tauri/lib/`.
2. Verification matrix (M3): render in-window, play/pause, seek, volume, fullscreen, resize, DPI change, hwdec active (`hwdec-current`), audio track selection, external SRT.
3. Fallback if quality bar fails: custom child-HWND embed (`wid`) via windows-rs + libmpv C API.

## Licensing

- libmpv-2.dll: LGPL build → dynamically linked, replaceable; notices shipped with installer (Phase 13).
- Plugin: MPL-2.0 file-level copyleft; if modified, changes to plugin files stay open; vendoring allowed.

## Findings

(to be filled at M3 completion)
