# Proposal

## Why

The in-app helper setup runs two separate `pkexec /usr/bin/install` invocations (helper binary, then polkit policy), and polkit asks for the admin password once per invocation — so the user types the same password twice for one setup action. The fix is to perform the whole install under a single polkit authorization.

## What Changes

- Merge the two privileged install steps (helper binary → `/usr/libexec/fmm-apply`, polkit policy → `/usr/share/polkit-1/actions/org.fmm.desktop.apply.policy`) into **one `pkexec` invocation**, so setup asks for the password exactly once.
- The install becomes atomic with respect to authorization: if the prompt is dismissed or any step fails, nothing is reported as installed (previously the helper could be installed while the policy step was denied).
- Update UI copy that advertises the double prompt: the busy dialog ("two authorization prompts may appear…") and the helper banner hint ("(two prompts)").
- Update the README section "Privileged helper installation", which also mentions the two prompts.
- Preserve existing behavior elsewhere: idempotent reinstall (helper step skipped when the resolved source already is the installed path), staged temp policy cleanup, `denied` error code on dismissed/failed authorization, and the `InstallReport` shape (list of installed paths).

## Capabilities

### New Capabilities

- `helper-setup`: In-app one-click installation of the privileged helper binary and its polkit policy — helper/policy presence reporting, the install flow and its authorization behavior, and the setup entry points (banner, error dialog). Today this behavior lives only in code (`install_helper`, `helper_status`, the setup UI) with no spec.

### Modified Capabilities

<!-- None: mirror-switch, repo-status and snapshot-rollback do not cover the helper install flow. -->

## Impact

- `src-tauri/src/commands.rs`: `install_helper_with`, the step builder (`install_steps`/`install_step`) and `run_pkexec` — the install becomes a single `pkexec /bin/sh -c '<install … && install …>'` invocation with shell-quoted paths; `run_pkexec` must accept non-path (`OsString`) arguments. Existing install tests reworked for the merged invocation.
- `ui/src/apply.ts`: busy-dialog text and helper-banner hint drop the "two prompts" wording.
- `README.md`: "Privileged helper installation" section wording.
- No new dependencies, no policy file change, no API shape change (`InstallReport`, `helper_status` unchanged).
- During apply (user-approved): a one-time workspace-wide `cargo fmt` to satisfy the `cargo fmt --check` gate in the verification task — formatting-only diffs in `crates/fmm-core/*` and `crates/fmm-apply/*`, no behavior change.
