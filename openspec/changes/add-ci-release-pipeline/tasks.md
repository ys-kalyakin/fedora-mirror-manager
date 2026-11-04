# Tasks

## 1. Helper staging and bundle config

- [x] 1.1 Create `scripts/stage-helper.sh` (bash, `set -euo pipefail`): `cargo build --release -p fmm-apply`, resolve host triple via `rustc -vV`, copy the helper to `src-tauri/binaries/fmm-apply-<triple>`; add root npm script `stage-helper`. Verify: run `npm run stage-helper` from the repo root and confirm `src-tauri/binaries/fmm-apply-x86_64-unknown-linux-gnu` exists and is executable.
- [x] 1.2 Add `src-tauri/binaries/` to `.gitignore`. Verify: after staging, `git status --porcelain` lists nothing under `src-tauri/binaries/`.
- [x] 1.3 Add root npm script `prebundle` (stage-helper + `npm --prefix ui run build`); in `src-tauri/tauri.conf.json` set `build.beforeBuildCommand` to `{ "script": "npm run prebundle", "cwd": ".." }` and add `bundle.externalBin: ["binaries/fmm-apply"]`. Verify: `npm run tauri build -- --bundles appimage rpm` from the repo root succeeds and produces `target/release/bundle/appimage/*.AppImage` and `target/release/bundle/rpm/*.rpm`; extract the AppImage (`*.AppImage --appimage-extract`, needs `patchelf`/linuxdeploy — install `patchelf` via dnf if the bundler asks) and confirm `usr/bin/fmm-apply` sits next to the app binary. Fallback if local AppImage bundling is blocked by missing system packages: verify with `--bundles rpm` locally and check the AppImage contents once CI builds it in task 4.4.
- [x] 1.4 Update `README.md`: replace the "Release build" section with the bundle command, add a short "Download releases" section (AppImage = download, `chmod +x`, run; **Install helper** inside the app; RPM = `dnf install`), and the CI badge for `ci.yml`. Verify: every command in the new sections runs or matches the artifacts produced in 1.3.

## 2. CI workflow

- [x] 2.1 Create `.github/workflows/ci.yml`: triggers on `push` (main) and `pull_request`; steps — checkout, apt install of the Tauri Linux package set on `ubuntu-24.04` (`libwebkit2gtk-4.1-dev`, `librsvg2-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libayatana-appindicator3-dev`, `libxdo-dev`, `patchelf`), `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `actions/setup-node@v4` (Node 20, npm cache keyed on `ui/package-lock.json`), `npm ci --prefix ui`, then the three README gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run build`. Verify: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"` parses, and the gate commands are byte-identical to the README's.

## 3. Release workflow

- [x] 3.1 Create `.github/workflows/release.yml`: trigger `push: tags: ['v*']`, job-level `permissions: contents: write`; same environment setup as ci.yml; then `npm run tauri build -- --bundles appimage rpm`; rename assets to `fedora-mirror-manager_${GITHUB_REF_NAME#v}_linux_x86_64.AppImage` / `.rpm` from `target/release/bundle/{appimage,rpm}`; generate `SHA256SUMS` with `sha256sum` over the two renamed files; publish with `softprops/action-gh-release@v2` (`generate_release_notes: true`, the three files). Verify: YAML parses; run the rename + `SHA256SUMS` snippet locally against the artifacts from 1.3 and confirm `sha256sum -c SHA256SUMS` passes.

## 4. Enable, verify, first release (outward-facing — confirm each push/tag with the user)

- [x] 4.1 Run all three gates locally before pushing (`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run build`; on this machine cargo lives in the toolchain bin path, not rustup proxies). Fix trivial findings; anything pre-existing that should not block CI gets an explicit decision before 4.2. Verify: all three exit 0.
- [x] 4.2 Add the remote `git@github.com:ys-kalyakin/fedora-mirror-manager.git` as `origin` and push main (with the user's go-ahead). Verify: the push lands and a CI run starts on GitHub for the pushed commit.
- [ ] 4.3 Watch the first CI run to green. Verify: `gh run watch` (or the Actions page) shows the run for the head commit green with all three gates executed.
- [ ] 4.4 First release: bump the workspace version + `ui/package.json` version as the release checklist requires, tag `v0.1.0`, push the tag (with the user's go-ahead). Verify: the GitHub Release page offers the AppImage, the RPM, and `SHA256SUMS`; `sha256sum -c` matches on a downloaded copy; the downloaded AppImage launches and its **Install helper** flow installs `/usr/libexec/fmm-apply` and the policy after one authorization.

## Workflow follow-up

- After the release verifies, archive the change per the project's review requirements.
- If the RPM was not install-tested in 4.4, verify `dnf install ./fedora-mirror-manager_*.rpm` on a spare Fedora x86_64 environment before relying on that scenario.
