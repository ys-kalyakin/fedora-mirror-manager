# Proposal

## Why

The repository currently has no CI and no releases: every check runs only on a
developer machine, and the published GitHub repository (`ys-kalyakin/fedora-mirror-manager`)
offers nothing a Fedora user can download and run. The app is at a stage where
it should be installable by non-developers, and the project needs a guaranteed
green state (tests, clippy, UI build) on every push and pull request.

## What Changes

- Add GitHub Actions CI that runs on every push and pull request and executes
  the project's own quality gates from the README: `cargo test --workspace`,
  `cargo clippy --workspace -- -D warnings`, and the UI production build
  (`tsc && vite build`).
- Add a tag-driven release pipeline: pushing a `v*` tag builds the app in
  release mode on Linux x86_64, produces an AppImage (the primary
  ready-to-run artifact) plus an RPM package, generates SHA-256 checksums,
  and publishes a GitHub Release with those assets and auto-generated notes.
- Bundle the privileged helper `fmm-apply` inside the AppImage (next to the
  app executable, via Tauri `externalBin`) so the existing one-click
  **Install helper** flow works from the released artifact without any
  build tools on the user's machine.
- Stage the helper binary for bundling through a small helper-staging script
  and document the release workflow in the README.

Assumptions (recorded, not user-confirmed in detail): Linux x86_64 only; no
code signing; releases are published (not draft) automatically on `v*` tags.

## Capabilities

### New Capabilities

- `ci-release`: Automated verification and distribution — CI checks on every
  push/pull request, and tag-driven GitHub Releases carrying a runnable
  AppImage (with bundled helper), an RPM, and checksums.

### Modified Capabilities

(none — the `helper-setup` resolution order already accepts a helper next to
the app executable; bundling the helper into the AppImage satisfies the
existing requirements without changing them)

## Impact

- New: `.github/workflows/ci.yml`, `.github/workflows/release.yml` (or a
  single combined workflow), helper staging script (e.g. `scripts/`).
- Modified: `src-tauri/tauri.conf.json` (`bundle.externalBin` for
  `fmm-apply`), `.gitignore` (staged sidecar binaries), `README.md`
  (release/download section, CI badge).
- Local builds: `npm run tauri build` now requires the helper to be staged
  first (same one command the app's docs already recommend building); the
  dev flow (`tauri dev`) is unaffected.
- CI environment: GitHub-hosted `ubuntu-24.04` runner needs the standard
  Tauri Linux system packages (webkit2gtk-4.1, librsvg, patchelf, …).
