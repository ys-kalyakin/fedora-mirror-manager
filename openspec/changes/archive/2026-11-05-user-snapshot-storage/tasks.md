# Tasks

## 1. Snapshot root and plan protocol (crates/fmm-core)

- [x] 1.1 Replace `DEFAULT_SNAPSHOT_ROOT` in `reposd.rs` with `user_snapshot_root()`: `$XDG_DATA_HOME/fmm/snapshots`, falling back to `$HOME/.local/share/fmm/snapshots` via `std::env::home_dir` (no new dependency); unit-test both resolutions plus the `FMM_SNAPSHOT_ROOT`-free default shape — `cargo test -p fmm-core` passes.
- [x] 1.2 Add `snapshot_root: String` to `ApplyPlan` (`#[serde(default)]`, empty = unset) and bump `PLAN_VERSION` to 2; update plan-building call sites and fixtures — `cargo test -p fmm-core` passes.
- [x] 1.3 Implement `delete_snapshot(snapshot_root, id)` in `snapshot.rs`: reuse `validated_snapshot_dir` (no separators, no `..`, must exist) and remove the directory with the `starts_with(snapshot_root)` guard; tests: create two, delete one, list shows one; invalid and missing ids error (`NotFound`) without touching anything else — `cargo test -p fmm-core` passes.
- [x] 1.4 Thread `snapshot_root` through `build_restore_plan` (embeds it in the plan next to `snapshot`) and update `tests/snapshots.rs` restore-plan assertions — `cargo test -p fmm-core` passes.
- [x] 1.5 Update the README's snapshot description (user data directory `~/.local/share/fmm/snapshots`, user-owned, per-snapshot delete, one-click legacy migration); verify `grep -rn "repos.d.bak" README.md` finds nothing and `cargo doc -p fmm-core` (or `cargo build`) shows no stale `DEFAULT_SNAPSHOT_ROOT` references.

## 2. Privileged helper (crates/fmm-apply)

- [x] 2.1 Use the plan-provided snapshot root: `run` rejects an empty `plan.snapshot_root` with `bad_plan`; the plan-version check now expects 2 (old plans/old helpers fail loudly on both sides); `Args::parse` keeps `--snapshot-root` as a dev/test override defaulting to `fmm_core::user_snapshot_root()`; update the args-default test — `cargo test -p fmm-apply` passes.
- [x] 2.2 Hand over ownership after `create_snapshot`: recursively chown the new snapshot to `PKEXEC_UID` (unix), falling back to the snapshot-root owner when unset; a chown failure aborts the privileged write with a `snapshot_failed` report (spec: snapshot failure aborts the write); integration test with `PKEXEC_UID` set to the current uid on a temp snapshot root — `cargo test -p fmm-apply` passes.
- [x] 2.3 Implement the `migrate-snapshots` mode (`fmm-apply migrate-snapshots --snapshot-root <dir> [--legacy-root /etc/yum.repos.d.bak]`): move every `20*` directory (`fs::rename`, falling back to `copy_tree` + remove on `EXDEV`), chown each to `PKEXEC_UID`/root-owner fallback, remove the legacy root when left empty, print `{ok, moved, failed}`; integration tests: two snapshots + a stray file move and leave the legacy dir in place, a fully-emptied legacy dir is removed — `cargo test -p fmm-apply` passes.

## 3. Command layer (src-tauri)

- [x] 3.1 Resolve `AppState::snapshot_root` from `FMM_SNAPSHOT_ROOT`, else `user_snapshot_root()`; ensure the root exists (user-owned `create_dir_all`) at app startup and before every `run_helper` call — verify with `cargo test` (workspace) and `cargo build -p fmm`.
- [x] 3.2 Add commands `delete_snapshot(snapshot_id)` (unprivileged, maps `NotFound`/io errors to `CmdError`), `legacy_snapshots()` (list from the legacy root, env-overridable for tests, empty when missing or unreadable) and `migrate_legacy_snapshots()` (one pkexec `fmm-apply migrate-snapshots` run, same one-prompt pattern as `install_helper`); register all three in `lib.rs`; tests drive delete/migrate against a temp root with a stub pkexec — `cargo test` passes.
- [x] 3.3 Pass `state.snapshot_root` into every built plan (`apply_plan` fills the field on plans arriving from the UI, `apply_restore` already rebuilds its plan) and reject plans without a root with `bad_plan`; test both command paths with the stub pkexec — `cargo test` passes.

## 4. UI (ui/src)

- [x] 4.1 Add `api.deleteSnapshot(id)`, `api.legacySnapshots()` and `api.migrateLegacySnapshots()` to `api.ts` — verify with `npm --prefix ui run build` (type check).
- [x] 4.2 In `snapshots.ts`: per-row Delete button opening a confirmation overlay (div overlay per project convention, Cancel/Delete), wired through the `anyDialogOpen` guard → `api.deleteSnapshot` → list reload, errors to `showErrorDialog`; add the destructive-button and overlay styles to `style.css` — verify with `npm --prefix ui run build` and `npm run tauri dev` (window visible via `niri msg windows`).
- [x] 4.3 In `snapshots.ts`: migration notice above the list (pattern of `helperBanner` in `apply.ts`) shown when `legacySnapshots()` returns entries, one-click → busy overlay → `api.migrateLegacySnapshots()` → reload and banner gone; dismissed authorization keeps the notice — verify via the built UI and a seeded legacy directory.

## 5. End-to-end verification

- [x] 5.1 `cargo fmt --check`, `cargo clippy`, full `cargo test`, and `npm --prefix ui run build` are clean.
- [x] 5.2 Manual GUI check on the dev machine (`npm run tauri dev` from the project root): a switch creates its snapshot under `~/.local/share/fmm/snapshots` (nothing appears under `/etc/yum.repos.d.bak`), the snapshots are listed, deleting one with confirmation removes only that directory, restoring from another snapshot still works with one prompt; with a seeded `/etc/yum.repos.d.bak` the migration notice appears once, one password prompt moves the snapshots (now user-owned), the legacy directory is gone and the notice no longer shows.

## 6. Helper upgrade banner (stale detection) — added mid-apply per user request

- [x] 6.1 Add a `version` mode to `fmm-apply` (first argv word `version` → prints the bare `PLAN_VERSION`, no stdin, no polkit) and an integration test asserting the printed version — `cargo test -p fmm-apply` passes.
- [x] 6.2 In `commands.rs`: `helper_status` probes the installed helper (`helper_plan_version`, executed directly without polkit) and `HelperStatus` gains `plan_version: Option<u32>` + `outdated` (`installed && plan_version != Some(PLAN_VERSION)`, unknown counts as outdated); stub-helper tests cover current/older/unknown-version/missing — `cargo test -p fmm-app` passes.
- [x] 6.3 Extend `HelperStatus` in `api.ts` and branch `helperBanner()` in `apply.ts`: installed-but-outdated shows the banner with an "Update helper" button reusing `runSetupFlow` and single-authorization hint wording — verify with `npm --prefix ui run build` and a real-window screenshot against the installed v1 helper.
- [x] 6.4 Update the README helper-installation intro: the banner also appears when the installed helper is outdated — verify the wording mentions both cases.

## Workflow follow-up

- After the user's review, sync the delta into the main `snapshot-rollback` spec and archive the change (`/opsx:sync` then `/opsx:archive`).
