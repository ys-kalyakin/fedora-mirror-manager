# Proposal

## Why

The one-click helper setup fails when the application runs from the released AppImage: the user approves the single authorization prompt, then the setup reports a failure ("Not authorized") and nothing is installed. The root cause was diagnosed on a real Fedora 45 host (journal + mount inspection): the resolved helper source lives on the AppImage's FUSE mount, which is mounted without `allow_other` and is therefore readable only by the mounting user's fsuid — the pkexec root process cannot read the helper binary, so the privileged `install` exits non-zero and the whole `&&` chain (helper and policy) installs nothing.

## What Changes

- Stage the helper binary to a root-readable temporary location before the single privileged invocation, so the install script never asks root to read from the app-bundle mount (the polkit policy is already staged this way).
- Keep the one-prompt invariant and all existing destinations, modes, and the "already installed → refresh policy" behavior.
- Report failures honestly: a dismissed/cancelled authorization keeps the `denied` code, but a privileged step that ran and failed returns a distinct `install_failed` code carrying the real stderr, and the UI shows that message instead of the generic "not authorized" text.
- No changes to bundling (the sidecar is already in `usr/bin/` inside the image), the polkit policy, or the helper-run path (`pkexec /usr/libexec/fmm-apply` is unaffected).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `helper-setup`: two requirement changes — (1) the setup must source both installed files from locations the privileged process can actually read (staged copies), so installation works when the app bundle lives on an owner-only FUSE mount (AppImage) as well as dev/RPM layouts; (2) a failed privileged step must surface the underlying error instead of reporting the authorization as dismissed.

## Impact

- `src-tauri/src/commands.rs`: `install_with`/`install_script`/`write_policy_temp` — add helper staging (one temp directory per setup run holding both files), and map pkexec exit codes (`127` → `denied`, other non-zero → `install_failed` with stderr).
- `ui/src/apply.ts`: `errorMessage` — show the backend message for `install_failed`.
- Tests: unit tests with the stub pkexec cover staging (the script references staged paths, not the original source), cleanup, and the exit-code mapping.
- Manual verification: run the released AppImage on a Fedora host with default `/etc/fuse.conf` (no `user_allow_other`) and install the helper with one prompt.
