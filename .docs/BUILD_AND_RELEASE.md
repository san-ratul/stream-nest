# Build and Release

Status: Phase 13 COMPLETE — installer built and verified on this machine.

## Verified release pipeline (2026-11-24)

- Command: `npm run tauri build` (beforeBuildCommand runs `npm run build`; cargo release profile with `lto = "thin"`, `codegen-units = 1`, `strip = true`).
- Artifacts:
  - `C:\rust-targets\streamnest\release\streamnest.exe` (21 MB)
  - `C:\rust-targets\streamnest\release\bundle\nsis\StreamNest_0.1.0_x64-setup.exe` (39.5 MB)
- NSIS: Tauri-managed; WebView2 downloadBootstrapper mode; per-user install to `%LOCALAPPDATA%\StreamNest`.
- Native deps included via `bundle.resources: ["lib/**/*"]`: `libmpv-2.dll` (LGPL build, 98.1 MB) + `libmpv-wrapper.dll` — verified present in installer output and in the installed directory.

## Verification performed

1. Release binary smoke test (release\ layout): app stays alive; log shows `Plugin registered`, `torrent engine initialized`, streaming server bound to 127.0.0.1.
2. Silent install (`setup.exe /S`, exit 0): files verified under `%LOCALAPPDATA%\StreamNest` including `lib\` DLLs; uninstaller registered in Add/Remove Programs (DisplayName StreamNest, 0.1.0).
3. Installed app smoke test: runs from install dir; engine initialized; plugin registered. DLL lazy-load on player open follows the same `exe_dir/lib` convention verified in dev.

## Known installer caveats (honest)

- NOT code-signed (no certificate): Windows SmartScreen will warn. Documented in KNOWN_LIMITATIONS.md.
- `seeder.exe` (dev harness binary) is also bundled by the NSIS packer because it lives in the same Cargo package. Harmless but unnecessary — removal candidate for post-MVP packaging polish (exclude via `bundle` config or move harness to a separate crate).
- Clean-OS verification not performed (no VM available) — tested on the dev machine only.

## Licensing notices

- `.docs/CARGO_DEPENDENCIES.txt`: full Cargo dependency inventory (608 crates, name+version).
- `.docs/THIRD_PARTY_NOTICES.md`: component licenses (librqbit MIT, tauri-plugin-libmpv MPL-2.0, libmpv-2.dll LGPL, Tauri MIT/Apache-2.0, Vue/Vite/Tailwind MIT, test media CC-BY Blender Foundation).
- The LGPL libmpv-2.dll is dynamically linked and user-replaceable, satisfying LGPL for closed distribution; the open-source core is MIT/Apache-2.0 with proprietary components server-side only (PRD §5).
