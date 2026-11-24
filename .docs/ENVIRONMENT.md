# Environment Report

Date: 2026-11-24
Machine: Windows 11 (build 26300), Intel Core i7-14700K (20C/28T), 31.8 GB RAM
Disk: C: 115.9 GB free (toolchain home), E: 18.4 GB free (repository home)

## Reference machine for performance targets (Phase 12)

- CPU: i7-14700K
- RAM: 31.8 GB
- OS: Windows 11 build 26300 (Win 11 24H2 line)

## Toolchain status

| Component | Status | Version / Notes |
|---|---|---|
| Git | OK | 2.45.1.windows.1; identity configured (repo-level commits by mission mandate) |
| Node.js | OK | 24.21.0 (nvs-managed) |
| npm | OK | 11.19.0 |
| pnpm | Not installed | Not required (npm used) |
| Rust | OK (installed this session) | rustup 1.29.3; rustc/cargo 1.100.0 stable-x86_64-windows-msvc; minimal profile |
| MSVC Build Tools | OK (installed this session) | VS 2022 BuildTools; MSVC 14.44.35207 |
| Windows SDK | OK (installed this session) | 10.0.26100.0 |
| WebView2 Runtime | OK | 158.0.4370.33 (Evergreen, HKLM) |
| NSIS | Bundled by Tauri at build time | Auto-downloaded by @tauri-apps/cli bundler |

## Installation actions taken this session

1. `rustup-init -y --default-toolchain stable-msvc --profile minimal` (user-scope).
2. `winget install Microsoft.VisualStudio.2022.BuildTools` with VCTools workload + recommended components (compiler + SDK). Verified via vswhere: MSVC 14.44.35207 + Windows SDK 10.0.26100.0.
3. Fallback toolchain `stable-x86_64-windows-gnu` installed and kept available (not default).

## Toolchain verification

- Rust MSVC smoke build (`cargo build` + run) linking `user32.GetTickCount`: PASSED.
  - Confirms rustc, MSVC link.exe, and Windows SDK import libraries resolve correctly.

## Repository state at first inspection

- `E:\streamnest` did not exist as a project (verified empty directory), no git repo, no files at risk.
- Git repo initialized on branch `main` this session.

## npm notes

- npm 11 `allowScripts` policy blocked esbuild postinstall; verified esbuild binary functions via platform optional package (`npx esbuild --version` → 0.28.2). No approval needed.

## Known environment caveats

- E: drive has limited free space; **cargo target dir relocated to `C:\rust-targets\streamnest`** via committed `.cargo/config.toml` (E: filled at 17.4 GB of artifacts on 2026-11-24). Sintel/bbb test media (~1.2 GB) lives in gitignored `.testmedia/` on E:.
- Shell processes do not inherit the updated user PATH; all cargo invocations use the absolute path `%USERPROFILE%\.cargo\bin\cargo.exe`.
- `npm run tauri dev` must be launched with `%USERPROFILE%\.cargo\bin` prepended to PATH in the same process.
