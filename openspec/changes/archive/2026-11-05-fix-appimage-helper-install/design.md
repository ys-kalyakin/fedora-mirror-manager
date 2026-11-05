# Design

## Context

The setup resolves the helper source (env override → executable's directory → installed path) and runs ONE `pkexec /bin/sh -c` chain of `/usr/bin/install` commands: helper first, then the staged policy. From the released AppImage the resolved source is `<mount>/usr/bin/fmm-apply`, where `<mount>` is `/tmp/.mount_<random>` — a FUSE (squashfuse) mount without `allow_other`. The kernel refuses every access from a process whose fsuid differs from the mount owner (uid 1000) — including root, regardless of capabilities, and without any SELinux AVC. Diagnosis evidence on Fedora 45: journal shows the pkexec root session opening (password accepted) and the exact failing command; `/etc/fuse.conf` has no `user_allow_other`; `mount` reports `user_id=1000,group_id=1000` without `allow_other`; the audit log has no AVC denials.

The polkit policy never had this problem: the setup already stages it into `std::env::temp_dir()` before the invocation. The helper run path (`pkexec /usr/libexec/fmm-apply`) is unaffected — root reads the installed, root-owned helper.

## Goals / Non-Goals

Goals:
- The one-click setup installs both files from the AppImage with one prompt.
- Failures after a successful authorization are reported with their real cause.

Non-Goals:
- No change to bundling, the polkit policy, or `fmm-apply` itself.
- No support for sandboxed/immutable `/usr` (rpm-ostree); out of scope as before.
- No rework of the generic `denied` handling for other commands (`apply_plan`, migration).

## Decisions

### Stage the helper into the existing per-run staging, unconditionally

Copy the resolved helper to a fresh temporary directory (created per setup run) that also holds the staged policy, then build the install script from those two paths. Unconditional staging (except when the source already is the installed path) keeps one code path for AppImage, RPM, and dev layouts, and makes the invariant unit-testable: the privileged script never references the resolved source path.

- Why not `allow_other` / `user_allow_other` in `/etc/fuse.conf`: requires a system-wide config change outside the app; mount fails for users without it. Rejected.
- Why not detect "root-readable" and skip staging: the app cannot reliably probe what root will be able to read (the FUSE check is fsuid-based). Conditional paths would leave the AppImage case untested. Rejected.
- Why not pipe the binary through stdin to a root `sh -c 'cat > …'`: muddles the single-script install, needs manual mode/owner handling, and diverges from the already-proven `install` chain. Rejected.
- Why not add an `install` mode to `fmm-apply`: adds protocol surface for no gain — the helper would face the same unreadable-source problem. Rejected.

Staging layout: one directory `fmm-install-<pid>-<n>` in `std::env::temp_dir()` (unique-name loop, same pattern as `write_policy_temp` today) containing `fmm-apply` (mode 0644) and `org.fmm.desktop.apply.policy`. Cleanup is `remove_dir_all` on every exit path, replacing the single-file policy cleanup.

Security: the staged copies are user-writable, and root installs from them. No new exposure — the source is user-controlled in every case today (the bundle itself is user-provided), and installing requires an admin authorization; the destinations remain root-owned with the same modes.

### Map pkexec exit codes to distinct errors

`run_pkexec` currently maps every non-zero exit to `denied`. Keep `denied` only for "authorization not obtained" (pkexec exits 127) and spawn failures; a privileged step that ran and failed returns `install_failed` with the captured stderr as the message. `install` exits 1 on failure, so 127 is unambiguous in practice.

### Surface `install_failed` messages in the UI

`errorMessage()` in `ui/src/apply.ts` hardcodes the "not authorized" text for `denied`; add the `install_failed` case showing `err.message`.

## Risks / Trade-offs

- [Temp directory unusable (noexec/full)] → Staging only reads/writes plain files there, nothing is executed from temp; a staging failure surfaces as a regular `io` error before any authorization, like the current policy staging.
- [pkexec exit-code semantics drift across polkit versions] → 127 is the documented "not authorized" code; if a future polkit changes it, the failure mode degrades to the current behavior (`denied`), never to a wrong success.
- [Concurrent setups] → Unique per-run directory name (`<pid>-<n>` loop) prevents collisions, same as the current policy file naming.

## Migration Plan

No data or configuration migration. Ships in the next release; the AppImage artifact is the only affected distribution channel. Rollback is a plain revert.

## Open Questions

None.
