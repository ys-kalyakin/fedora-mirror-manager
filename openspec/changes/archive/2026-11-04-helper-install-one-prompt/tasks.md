# Tasks

## 1. Backend: single-authorization install (src-tauri/src/commands.rs)

- [x] 1.1 Add a `shell_quote` helper (single-quote wrapping with `'\''` escaping) with unit tests covering plain paths, paths with spaces, and paths with embedded single quotes; verify with `cargo test -p fmm --lib` (or the workspace root `cargo test`) — quoting tests pass.
- [x] 1.2 Replace `install_steps`/`install_step` with a script builder returning the shell script plus the logical destination list (helper destination only when the source is not already `/usr/libexec/fmm-apply`, policy destination always); rework `install_steps_cover_binary_and_policy_and_skip_reinstall` into script-content tests asserting both `/usr/bin/install` commands, the `&&` join, the destinations, and the reinstall-skip case — `cargo test` passes.
- [x] 1.3 Generalize `run_pkexec` to take non-path (`OsString`) arguments and update the existing call sites and stub-pkexec tests; `run_helper_with` behavior unchanged — `cargo test` passes.
- [x] 1.4 Rewire `install_helper_with` to run exactly one `pkexec /bin/sh -c '<install … && install …>'` invocation, keep temp-policy cleanup, and fill `InstallReport` from the builder's destination list; update `install_flow_runs_and_cleans_up_with_stub_pkexec` to drive the single invocation and assert one pkexec call, cleanup, and the report paths — `cargo test` passes.

## 2. UI copy: single prompt wording (ui/src/apply.ts)

- [x] 2.1 Change the setup busy dialog text from "two authorization prompts may appear…" to a single-prompt wording ("one authorization prompt") — verify in the built output or by inspecting `ui/src/apply.ts`; no other copy touched.
- [x] 2.2 Change the helper-banner hint from "(two prompts)" to wording saying the install asks for authorization once — `grep -rn "two prompt" ui/src` returns nothing.

## 3. Documentation (README.md)

- [x] 3.1 Update the "Privileged helper installation" section: "two polkit authorization prompts" becomes single-prompt wording (and keep the idempotent-reinstall note accurate) — `grep -n "two" README.md` no longer matches the setup wording; the section still describes the one-click in-app flow.

## 4. End-to-end verification

- [x] 4.1 `cargo fmt --check`, `cargo clippy`, and full `cargo test` are clean.
- [x] 4.2 Manual GUI check: uninstall the helper (remove `/usr/libexec/fmm-apply` and the policy, `sudo` needed once), run `npm run tauri dev` from the project root, click **Install helper** in the banner, and confirm exactly one password prompt appears and the completion dialog lists both installed paths; a second click re-succeeds with one prompt.
