# Tasks

## 1. Environment prerequisites

- [x] 1.1 Install Rust stable via rustup and verify `cargo --version` and `rustc --version` report a stable toolchain
- [x] 1.2 Install Tauri 2 system dependencies on Fedora (`sudo dnf install webkit2gtk4.1-devel gtk3-devel glib2-devel dbus-devel librsvg2-devel libsoup3-devel`) and verify `pkg-config --exists webkit2gtk-4.1` succeeds

## 2. Workspace scaffold

- [ ] 2.1 Initialize git repository; add `.gitignore` (Rust `target/`, Node `node_modules/`, `dist/`, editor cruft) and `.editorconfig`; verify `git status` runs and build artifacts are reported as ignored
- [x] 2.2 Create workspace root `Cargo.toml` (members `crates/fmm-core`, `src-tauri`; shared `[workspace.dependencies]`; release profile), `rust-toolchain.toml` (channel `stable`), `rustfmt.toml`; verify `rustup show` selects stable and `cargo metadata --no-deps` parses
- [x] 2.3 Create `crates/fmm-core`: `lib.rs` with `pub fn version()` and stub modules `repo`, `mirror`, `error`, plus a unit test for `version()`; verify `cargo test -p fmm-core` passes and `cargo clippy -p fmm-core -- -D warnings` is clean

## 3. Tauri application shell (fmm-app)

- [x] 3.1 Scaffold `src-tauri` crate: `Cargo.toml` (Tauri 2 dependencies), `build.rs`, `tauri.conf.json` (identifier `org.fmm.desktop`, window title "Fedora Mirror Manager", dev/frontend URLs pointing at `ui/`), `capabilities/default.json`, and an application icon set generated from a placeholder PNG; verify `cargo check -p fmm-app` succeeds
- [x] 3.2 Implement `main.rs` / `lib.rs` `run()` registering the `app_status` command that returns the version from `fmm-core`; verify `cargo clippy -p fmm-app -- -D warnings` is clean

## 4. Frontend shell (ui/)

- [x] 4.1 Scaffold Vite + TypeScript project in `ui/` (`package.json` with a `tauri` script, `index.html`, `tsconfig.json`, `vite.config.ts` configured for Tauri); verify `npm install` and `npm run build` succeed
- [x] 4.2 Implement the shell UI: header with "Fedora Mirror Manager", placeholder mirror-list area, status area; `shell.ts` invokes `app_status` on load, renders the returned version, and shows an explicit error indication when the call fails; verify `npm run build` and `tsc --noEmit` pass

## 5. Integration verification

- [x] 5.1 Run `cargo test` and `cargo clippy --workspace -- -D warnings` at the workspace root; verify all pass
- [ ] 5.2 Run `npm run tauri dev` from `ui/` and confirm manually: the window opens titled "Fedora Mirror Manager", the shell layout (header, mirror-list placeholder, status area) is visible, the status area shows the backend version, and closing the window exits the process
- [x] 5.3 Write `README.md` (project purpose, prerequisites from group 1, dev/build commands, structure overview); verify each documented command runs as written
- [ ] 5.4 Commit the scaffold as the initial git commit; verify `git log` shows the commit and `git status` is clean

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result.
