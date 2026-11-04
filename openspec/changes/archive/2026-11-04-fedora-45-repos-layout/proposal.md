# Proposal

## Why

Fedora 45 (the "Relocate RPM Repo Configs to /usr" change) moved all packaged `.repo` files out of `/etc/yum.repos.d` into `/usr/share/dnf5/repos.d`; on such systems the application now finds no switchable repositories and can no longer switch anything (verified on a real F45 system: `/etc/yum.repos.d` holds only third-party files and inert `.rpmsave` leftovers of pre-upgrade switches). The application must support both the legacy (≤ F44) and the F45+ layout, detecting the layout automatically.

## What Changes

- The application automatically detects which repository layout the host uses: legacy (packaged repos in `/etc/yum.repos.d`) vs F45+ (packaged repos in `/usr/share/dnf5/repos.d`, `/etc/yum.repos.d` kept for third-party and locally written files). Detection is feature-based (presence of `.repo` files in the F45+ directory), not an OS-release version lookup.
- On the F45+ layout, the repository inventory merges both sources: packaged repos from `/usr/share/dnf5/repos.d` and user repos from `/etc/yum.repos.d`. When a repository section id is defined in both, the `/etc/yum.repos.d` definition is the effective one (dnf5 semantics: the first `reposdir` in its list wins the section; dnf5 5.4.x default list is `[/etc/yum.repos.d, /etc/distro.repos.d, /usr/share/dnf5/repos.d]`).
- On the F45+ layout, a mirror switch never writes to `/usr/share/dnf5/repos.d` (package-owned, unconditionally replaced on package updates). Instead the switch writes a complete shadow copy of the packaged `.repo` file — with only the target section's URL lines rewritten — into `/etc/yum.repos.d` under the same file name, where it overrides the packaged file entirely (verified dnf5 behavior).
- The user sees which layout is active, and on the F45+ layout sees which location each entry comes from.
- Snapshots and restore keep operating on `/etc/yum.repos.d` in both layouts, which on F45+ covers exactly the state the application can modify (third-party files plus its own shadow copies); packaged files under `/usr` are never snapshotted, restored, or modified.
- The privileged helper gains the ability to create a file that does not yet exist (the first switch on F45+ creates a shadow), while keeping the existing stale-plan hash check for existing files.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `repo-status`: inventory must load from the detected layout — both sources merged on F45+ with `/etc/yum.repos.d` precedence per section id; layout and entry origin surfaced in the UI; missing-directory error states redefined per layout.
- `mirror-switch`: on the F45+ layout the switch writes a shadow copy into `/etc/yum.repos.d` derived from the current packaged file instead of rewriting a file in place; preview must show the target path; all URL mapping, section-rewrite and preview rules are unchanged.
- `snapshot-rollback`: snapshot/restore scope redefined as "the app-managed state in `/etc/yum.repos.d`" for both layouts; on F45+ restoring to a snapshot taken before any switches also removes shadow copies created after it (the existing extra-file handling already covers this — the spec text must state it explicitly).

## Impact

- `crates/fmm-core`: new layout-detection module; `reposd.rs` inventory gains multi-source reading with section-id precedence; `plan.rs` builds switch plans against the effective/owning file and may target a not-yet-existing file (`PlanFile.before_sha256` gains a "must not exist" sentinel); classification (`classify.rs`) unchanged — file names and section ids are the same in the F45+ directory.
- `crates/fmm-apply`: create-new-file support in the write step; defaults and `--repo-dir` handling unchanged (it still writes only into `/etc/yum.repos.d`).
- `src-tauri` (`commands.rs`, `lib.rs`): `AppState` carries both directories plus the detected layout; no new commands, existing command signatures extended with layout/origin fields in returned structs.
- `ui/src` (`repos.ts` and friends): layout indicator; empty/error states reworded per layout; strings no longer hardcode only `/etc/yum.repos.d`.
- Tests: `fmm-core` inventory/plan tests gain F45-layout fixtures (temp `usr`-style dir + `etc` dir); `fmm-apply` tests gain the create-new-file case.
- Out of scope: migrating pre-upgrade `.rpmsave` files back into effect; editing `/etc/dnf/repos.override.d` drop-ins; Silverblue/ostree `/usr` writability concerns (never written by this change).
