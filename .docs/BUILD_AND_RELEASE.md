# Build and Release

Status: draft; finalized at Phase 13.

## Dev build

- `npm install`
- `npm run tauri dev` (vite :5173 + cargo debug)
- `npm run build` / `cargo build --release` for artifacts.

## Release

- Tauri NSIS target (tauri.conf.json `bundle.targets: ["nsis"]`).
- WebView2: downloadBootstrapper install mode.
- Native deps bundled: `libmpv-2.dll` (LGPL build) + wrapper (per PLAYER_INTEGRATION.md), resource-configured under `bundle.resources`.
- Icons/metadata: generated set (assets/icon-source.png → src-tauri/icons/), version fields in tauri.conf.json.
- Licensing notices: THIRD-PARTY-NOTICES with libmpv LGPL text, plugin MPL-2.0, librqbit MIT, and full Rust dependency inventory (`cargo license` style listing).
- Signing: NOT claimed — no certificate exists; SmartScreen warning expected and documented in KNOWN_LIMITATIONS.md.

## Verification

- Installer built and executed on this machine; clean-VM test documented if available (likely not — logged honestly).
