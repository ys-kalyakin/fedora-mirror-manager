# Design

## Context

Tauri 2 workspace (`fmm-core`, `fmm-apply`, `fmm-app`) + Vite/TS UI; no
`.github/` yet; git remote not yet added (target:
`git@github.com:ys-kalyakin/fedora-mirror-manager.git`). The app already
resolves the privileged helper as `FMM_APPLY_HELPER` → helper next to the
running executable → `/usr/libexec/fmm-apply`
(`src-tauri/src/commands.rs:415`), and embeds the polkit policy at compile
time (`include_str!` of `packaging/polkit/…`). `tauri.conf.json` currently
declares `bundle.targets: "all"` and no `externalBin`; `fmm-apply` is not a
dependency of `fmm-app`, so `tauri build` does not build it. Motivation: see
proposal.md — Why; behavior contract: specs/ci-release/spec.md.

## Goals / Non-Goals

**Goals:**
- Two GitHub Actions workflows: `ci.yml` (push/PR gates) and `release.yml`
  (tag `v*` → AppImage + RPM + SHA256SUMS → GitHub Release).
- Released AppImage ships `fmm-apply` next to the app binary so the existing
  one-click helper install works with zero code changes to the app.
- `npm run tauri build` keeps working locally without remembering extra
  steps.

**Non-Goals:**
- Code signing / notarization of artifacts.
- aarch64, Windows, macOS builds.
- Auto-update (no tauri-updater), changelog curation, `cargo fmt` gate.
- Publishing to COPR/Flathub; repo visibility settings.

## Decisions

1. **Two workflow files, not one.** `ci.yml` triggers on `push` (main) and
   `pull_request` with default (read-only) token permissions; `release.yml`
   triggers only on `push: tags: ['v*']` and declares
   `permissions: contents: write` for creating releases. GitHub does not
   support per-job triggers, so a single file would need guarded steps and a
   write token on every PR run — rejected.

2. **Runner: `ubuntu-24.04`.** The project targets WebKitGTK 4.1 (README's
   dnf list); ubuntu-24.04 provides `libwebkit2gtk-4.1-dev` via apt, while
   22.04 only ships 4.0. System packages: the standard Tauri Linux set
   (`libwebkit2gtk-4.1-dev`, `librsvg2-dev`, `build-essential`, `curl`,
   `wget`, `file`, `libssl-dev`, `libayatana-appindicator3-dev`, `libxdo-dev`,
   and `patchelf` — required by the AppImage bundler).

3. **Helper in the AppImage via `bundle.externalBin`, not resources or a
   separate download.** `externalBin: ["binaries/fmm-apply"]` makes Tauri
   place the staged file `src-tauri/binaries/fmm-apply-x86_64-unknown-linux-gnu`
   next to the app binary inside the image (`usr/bin/`), which is exactly the
   location `helper_source_with` already checks — no app-code changes. The
   policy is already embedded in the binary. Alternatives rejected: bundling
   as a `resources` file (wrong directory; would need a resolution-order
   change in code), asking users to download the helper separately (breaks
   the one-click flow the project is committed to).

4. **Helper staging is part of the build command chain.** New
   `scripts/stage-helper.sh` (bash, `set -euo pipefail`): `cargo build
   --release -p fmm-apply`, then copy the result to
   `src-tauri/binaries/fmm-apply-<host-triple>` (triple from `rustc -vV`).
   `tauri.conf.json`'s `beforeBuildCommand` changes to a root npm script
   (`prebundle` = stage-helper + `npm --prefix ui run build`) so `npm run
   tauri build` self-stages the helper in CI and locally. `src-tauri/binaries/`
   is gitignored. Dev flow (`tauri dev`) is untouched — it keeps resolving
   the helper from `target/debug/` per the README. Alternative rejected:
   building the helper inside `beforeBuildCommand` of the UI package (mixes
   cargo into the UI toolchain and hides the step).

5. **Release workflow builds `--bundles appimage rpm` and renames assets.**
   Tauri's default names contain spaces (`Fedora Mirror Manager_0.1.0_amd64.AppImage`);
   the workflow renames to `fedora-mirror-manager_<tag>_linux_x86_64.AppImage`
   and `…_.rpm` (version taken from the tag, not the crate, so assets stay
   consistent even if the workspace version lags). Renaming is safe: the
   AppImage is a self-contained file and the app has no updater. `deb` is
   dropped from release assets on purpose (Fedora-focused project).
   Checksums: single `SHA256SUMS` generated with `sha256sum` over both
   renamed assets.

6. **Publishing: `softprops/action-gh-release@v2` with
   `generate_release_notes: true`.** Handles re-runs cleanly (overwrites
   existing assets on the same tag) without hand-rolled `gh release create
   || gh release upload --clobber` logic. Actions are pinned to major tags;
   the GITHUB_TOKEN suffices (no PAT).

7. **Toolchain and caching.** `dtolnay/rust-toolchain@stable` (respects
   `rust-toolchain.toml`'s `channel = "stable"`), `Swatinem/rust-cache@v2`,
   `actions/setup-node@v4` (Node 20, npm cache keyed on
   `ui/package-lock.json`), installs via `npm ci`. Gates mirror the README
   exactly: `cargo test --workspace`, `cargo clippy --workspace -- -D
   warnings`, `npm --prefix ui run build`. The release job reuses the same
   setup and then runs `npm run tauri build -- --bundles appimage rpm`
   (which re-runs the UI build — accepted cost, keeps one code path).

## Risks / Trade-offs

- [Runner apt packages drift or break webkit2gtk build] → Pin
  `ubuntu-24.04`, declare all system packages in one explicit apt step;
  the clean-checkout requirement in the spec forces the workflow to stay
  self-contained.
- [First CI run is red because `clippy -D warnings` has never run in CI
  before] → During apply, run the three gates locally first (the toolchain
  on this machine runs via the toolchain bin path, not rustup proxies) and
  fix or explicitly defer any pre-existing clippy findings before pushing.
- [Tag version vs crate version drift (assets named by tag, binary embeds
  crate version)] → Release checklist: bump `version` in the workspace
  Cargo.toml (and ui/package.json) as part of tagging; auto-sync is a
  possible follow-up, not v1.
- [`externalBin` makes **every** cargo build of `fmm-app` fail if the sidecar
  is absent — not only bundling: `tauri-build` (build.rs) verifies the
  declared sidecar path, so `cargo test`/`cargo clippy` break on a fresh
  checkout too] → `beforeBuildCommand` stages it for bundle builds (decision
  4), the CI test job runs `npm run stage-helper` explicitly before the
  cargo gates, and the README's tests section documents the one-time staging
  requirement. `tauri dev` is unaffected (no tauri-build sidecar check
  beyond the same rule — the dev helper keeps resolving from
  `target/debug/`).
- [Publishing an accidental `v*` tag] → Release workflow has no delete or
  edit powers beyond its own release; deleting a bad tag/release on GitHub
  is a two-click manual rollback.

## Migration Plan

Add workflow files + staging script + config edits as one change, push to
main, confirm the first CI run is green, then cut the first real tag
(coordinated with the user — pushing tags and the initial push of `origin`
are outward-facing actions). Rollback: delete the workflows; releases
already published remain valid artifacts.

## Open Questions

(none — remaining choices, e.g. exact action SHAs vs major tags, are
implementation details deferred to tasks)
