# Design

## Context

Today every snapshot is written by the root helper (`fmm-apply`) into `DEFAULT_SNAPSHOT_ROOT = /etc/yum.repos.d.bak`. The helper's plan protocol carries no snapshot-root information: `apply_switch`/`apply_restore` use the helper's own default (overridable only by the never-passed `--snapshot-root` flag), while restore reads the source from the plan's absolute `snapshot` path. Listing and restore preview already run unprivileged in the app process (`list_snapshots`, `preview_restore` in `src-tauri/src/commands.rs`) and merely read the root-owned directory through permissive modes. The app state holds the snapshot root in `AppState::default()`. See proposal.md — Why for the motivation.

## Goals / Non-Goals

**Goals:**
- Snapshots stored under the invoking user's data directory; nothing written under system directories.
- List / restore-preview / delete work without polkit; only writes (switch, restore) keep requiring authorization.
- Existing snapshots survive the move via an explicit, one-click migration.
- Protocol mismatch between app and helper versions fails loudly.

**Non-Goals:**
- Changing retention behavior (still 10, still pruned on create).
- Per-snapshot metadata files, rename, or tag editing.
- Encrypting or integrity-protecting snapshots (see Risks).
- Changing restore semantics (purge option, extra-files reporting).

## Decisions

**1. Snapshot root: `$XDG_DATA_HOME/fmm/snapshots` (default `~/.local/share/fmm/snapshots`).**
Resolved in the app from `XDG_DATA_HOME` with a `$HOME` fallback (`std::env::home_dir` — un-deprecated and correct on the pinned stable toolchain; no new dependency). The `fmm` directory matches the existing naming family (`fmm-core`, `fmm-apply`, `FMM_APPLY_HELPER`).
- Alternative `app_data_dir()` (`$XDG_DATA_HOME/org.fmm.desktop/snapshots`): reverse-DNS noise in the file manager for zero benefit; rejected.
- Dev/test override `FMM_SNAPSHOT_ROOT`, consistent with `FMM_APPLY_HELPER`.
- `reposd.rs::DEFAULT_SNAPSHOT_ROOT` is replaced by `user_snapshot_root()` in fmm-core; the helper keeps `--snapshot-root` purely as a dev/test escape hatch.

**2. The app owns the root; the plan carries it.**
`ApplyPlan` gains `snapshot_root: String`; the helper uses it for the pre-write snapshot and rejects an empty value with `bad_plan`. The helper (running as root) cannot reliably resolve the invoking user's XDG directories, and the plan is already the app→helper channel of record. The restore source stays the plan's absolute `snapshot` path inside that root.
- `PLAN_VERSION` bumps 1 → 2: serde on the old helper would silently ignore the new field and fall back to the legacy path — a version bump turns that into a loud `bad_plan` on both sides instead.

**3. Ownership handover in the helper.**
The app ensures the snapshot root exists (user-owned) at startup and before every helper call, so the helper's `create_dir_all` never creates root-owned parents. After creating a snapshot the helper recursively chowns it to `PKEXEC_UID` (set by pkexec, documented); when unset (manual helper runs), it falls back to the owner of the snapshot root. This is what makes list/preview/delete unprivileged — without it the snapshots would be root-owned files inside the user's home.
- Alternative (app-side snapshot copy as the user) rejected: reading `/etc/yum.repos.d` requires root in the first place.

**4. Delete is an app-side operation: `delete_snapshot(snapshot_root, id)` in fmm-core.**
Validates the id with the existing `validated_snapshot_dir` rules (no separators, no `..`, must exist) and removes the directory with `fs::remove_dir_all` guarded by a `starts_with(root)` check (same guard as the prune path). New Tauri command `delete_snapshot`, no polkit. The UI adds a per-row delete control with a confirmation overlay (div overlay, not `<dialog>`, per project convention) and refreshes the list.

**5. Migration as a helper mode, one prompt.**
`fmm-apply migrate-snapshots --snapshot-root <user root> [--legacy-root /etc/yum.repos.d.bak]`: moves every `20*` directory into the user root (`fs::rename`, falling back to `copy_tree` + remove on `EXDEV` — XDG_DATA_HOME may live on another filesystem), chowns each to `PKEXEC_UID`, removes the legacy root when left empty, and prints `{ok, moved, failed}`. It takes no stdin plan — it is not a plan action. The app checks the legacy root (`legacy_snapshots()`, empty when unreadable or missing) only to decide whether the one-click notice is shown in the snapshots panel (reusing the `helperBanner` pattern); the actual work is always delegated to the helper, which is root and sees the truth.
- Alternatives: migrate lazily inside the next privileged write (snapshots invisible until then — bad UX); app-side copy + a pkexec `rm` one-shot (two prompts' worth of logic, still perm-fragile).

**6. Security posture of a user-writable snapshot root.**
A local process running as the user can now edit snapshot content before a restore copies it into `/etc/yum.repos.d`. This adds no real privilege escalation: the same polkit action already makes the helper write fully user-supplied file content (`plan.files` carry complete new content), so anyone able to tamper with the snapshot dir can equally send a crafted switch plan. Accepted trade-off; the snapshots remain tamper-evident only to the extent the user's own account is trusted.

**7. Stale-helper detection: `fmm-apply version` probe.**
The installed helper from before this change speaks plan protocol 1 and rejects every v2 plan with `bad_plan` — and the setup banner only showed for a *missing* helper, leaving no in-app recovery. The helper gains a `version` mode that prints the bare `PLAN_VERSION` and exits (no stdin, no polkit — the application execs the installed binary directly, which grants nothing). `helper_status` probes it and reports `plan_version` plus `outdated` (`installed && version != PLAN_VERSION`; a helper too old to know the flag, or any exec failure, counts as outdated). The banner then offers the same one-click setup as an update. Alternatives: embedding a version in the report (only visible after a privileged run — too late); keying off `bad_plan` errors (scatter-gun, no proactive offer).
- The probe cannot hang the panel: the version mode prints and exits without reading stdin, and an unknown flag makes an old helper exit immediately.

## Risks / Trade-offs

- [User-writable snapshots can be tampered with before a restore] → Accepted per Decision 6; the restore action itself still requires authorization.
- [Root-owned remnants inside the user root (e.g. helper run manually without `PKEXEC_UID` and a root-owned snapshot root)] → ownership fallback to the snapshot-root owner; the UI delete surfaces a clear `io` error instead of failing silently.
- [`XDG_DATA_HOME` on a different filesystem than `/etc`] → migration falls back to copy+remove on `EXDEV`.
- [Old helper + new app (or vice versa)] → `PLAN_VERSION` 2 rejected as `bad_plan` on both sides; the existing one-click install upgrades the helper.
- [User deletes `~/.local/share/fmm` out from under the app] → the app recreates the root; restore of a missing snapshot reports `not_found`. Data loss is inherent to user-owned data.
- [Legacy root still listed in docs/habits] → README and spec paths updated in this change; the notice disappears once `/etc/yum.repos.d.bak` is gone.

## Migration Plan

1. Ship app and helper together; the user re-runs the in-app one-click install (existing pattern) so `/usr/libexec/fmm-apply` matches `PLAN_VERSION` 2.
2. First launch of the new version: the snapshots panel shows the migration notice when `/etc/yum.repos.d.bak` still contains snapshots; one click + one polkit prompt completes the move, hands ownership to the user, and removes the legacy directory when empty.
3. Rollback: until migration is accepted, nothing has changed on disk — the legacy snapshots stay put; downgrading the app (with the old helper) keeps working against `PLAN_VERSION` 1 plans.

## Open Questions

*(none)*
