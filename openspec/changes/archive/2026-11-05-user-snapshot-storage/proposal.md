# Proposal

## Why

Snapshots currently live in `/etc/yum.repos.d.bak` — a root-owned system directory. The user wants snapshots kept out of system directories, stored under their own home so system directories stay clean, and wants the ability to delete a single snapshot they no longer need instead of waiting for the retention prune.

## What Changes

- **BREAKING** Snapshot storage moves from `/etc/yum.repos.d.bak` to the user's data directory: `$XDG_DATA_HOME/fmm/snapshots` (default `~/.local/share/fmm/snapshots`). The plan protocol gains a `snapshot_root` field and `PLAN_VERSION` is bumped 1 → 2, so an old helper with a new app (or vice versa) fails loudly instead of silently writing to the legacy path.
- Snapshots become user-owned: the helper hands over ownership of every snapshot it creates (chown to the invoking user via `PKEXEC_UID`), so listing, restore preview and deletion need no polkit — only actual writes (switch, restore) still require authorization.
- New capability in the snapshots panel: delete one snapshot after an explicit confirmation; deletion is unprivileged and refreshes the list.
- One-click migration: when snapshots still exist in the legacy `/etc/yum.repos.d.bak`, the UI offers to move them into the user snapshot root with a single polkit prompt; the legacy directory is removed when empty.
- Update banner for a stale helper (added mid-apply per user request): an installed helper that reports an older plan-protocol version than the application now surfaces the same one-click setup as an update — previously an installed-but-outdated helper showed no banner at all, leaving the version mismatch (`bad_plan`) without a recovery path.

## Capabilities

### New Capabilities

*(none)*

### Modified Capabilities

- `snapshot-rollback`: snapshot location and ownership change (user data directory instead of `/etc/yum.repos.d.bak`); new requirements for deleting a single snapshot and for one-click migration of legacy snapshots.

## Impact

- `crates/fmm-core`: `snapshot.rs` (new root constant, `delete_snapshot`), `plan.rs` (`snapshot_root` field, `PLAN_VERSION` 2), `reposd.rs` (`DEFAULT_SNAPSHOT_ROOT` replaced by the user-dir default).
- `crates/fmm-apply`: uses the plan-provided snapshot root, chowns created snapshots to the invoking user, new `migrate-snapshots` mode, plan-version check.
- `src-tauri`: `AppState` resolves the snapshot root from `XDG_DATA_HOME` (env override for dev/tests), ensures it exists as a user-owned directory, new `delete_snapshot` and `migrate_legacy_snapshots` commands.
- `ui/src/snapshots.ts`: delete button + confirmation per snapshot row, legacy-snapshot migration banner (same one-click pattern as the helper setup banner).
- Tests: `crates/fmm-core/tests/snapshots.rs`, helper and command tests extended; README path references updated.
