# Tasks

## 1. Backend: staged install sources (src-tauri/src/commands.rs)

- [x] 1.1 Replace `write_policy_temp` with a per-run staging directory: create a unique `fmm-install-<pid>-<n>` directory in `std::env::temp_dir()` (create-new loop, same pattern as today's policy file), stage the embedded policy at mode 0644 inside it, and add a staging step that copies the resolved helper to mode 0644 in the same directory; both steps fail with the `io` code before any authorization, and the staging tests cover the helper copy, the policy copy, and the unique-name loop — verify with `cargo test -p fmm-app` (workspace `cargo test`).
- [x] 1.2 Rewire `install_with` to: resolve the helper source, stage helper (skipped when the source already is `/usr/libexec/fmm-apply`) and policy into one staging directory, run the single `pkexec /bin/sh -c` chain built from the staged paths, and clean up the whole staging directory with `remove_dir_all` on every exit path (success, pkexec failure, staging failure); `helper_missing` and the one-invocation behavior are unchanged — verify with `cargo test` and the updated stub-pkexec flow tests (one pkexec call, report paths, cleanup leaves no `fmm-install-<pid>-*` behind).
- [x] 1.3 Update the `install_script` tests to assert the script references the staged source paths and never the resolved (bundle) source path, while keeping destinations, modes, `&&` join, quoting, and the reinstall-skip behavior identical — verify with `cargo test`.
- [x] 1.4 Map pkexec outcomes in `run_pkexec`: exit 127 → `denied` (authorization not obtained), any other non-zero exit → `install_failed` with the trimmed stderr as the message, spawn failure → `denied`; add stub-pkexec tests for a dismissed run (empty stdout, exit 127) and a failed privileged step (exit 3 with stderr) asserting the distinct codes and messages — verify with `cargo test`.

## 2. UI: honest failure message (ui/src/apply.ts)

- [x] 2.1 Add an `install_failed` case to `errorMessage()` that returns `err.message` so the error dialog shows the underlying failure after an approved authorization; `denied` keeps the existing "not authorized" text — verify with `npm --prefix ui run build` (type-check) and by inspecting the mapping.

## 3. End-to-end verification

- [x] 3.1 `cargo fmt --check`, `cargo clippy`, and full `cargo test` are clean, and `npm --prefix ui run build` succeeds.
- [x] 3.2 Manual AppImage check on this host (default `/etc/fuse.conf`, no `user_allow_other`): build the bundle with `npm run tauri build -- --bundles appimage`, make sure `/usr/libexec/fmm-apply` and the policy are absent, run the AppImage, click **Install helper**, confirm exactly one password prompt appears and the completion dialog lists both installed paths; repeat once to confirm the refresh path still succeeds with one prompt.
- [x] 3.3 Manual failure-path check: temporarily make the privileged step fail (for example stage a bogus helper via `FMM_APPLY_HELPER`), approve the prompt, and confirm the error dialog shows the underlying `install` error text instead of "Not authorized"; remove the staged state afterwards.
