# Design

## Context

Today the whole backend assumes a single directory: `AppState.repo_dir = /etc/yum.repos.d`, read by `fmm_core::reposd::inventory`, targeted by `plan::build_action_plan`, snapshotted and written by the `fmm-apply` helper.

Fedora 45 moved packaged `.repo` files to `/usr/share/dnf5/repos.d` (Fedora change "Relocate RPM Repo Configs to /usr"). Facts verified on the target system (F45, dnf5 5.4.6.0):

- `dnf5.conf(5)`: `reposdir` default is `[/etc/yum.repos.d, /etc/distro.repos.d, /usr/share/dnf5/repos.d]`.
- Probes with throwaway `--setopt=reposdir=…` configs: when the same section id is defined in several reposdirs, the **first directory in the list owns the whole section** — later definitions are dropped and their keys are *not* merged (a first-dir file listing only `baseurl` hid the second-dir file's `name=`, which fell back to the id). With the default order, `/etc/yum.repos.d` therefore overrides `/usr/share/dnf5/repos.d` — matching the change's "modified files override /usr entirely" contract.
- Overrides (`/etc/dnf/repos.override.d`, key-level, "last option wins") are a separate mechanism on top; an empty value clears a key. `dnf config-manager` persists changes there as `99-config_manager.repo`.
- dnf5 loads a repo that has both `baseurl` and `metalink` without an error, so a partial override that only adds `baseurl` leaves the packaged `metalink` in place — dangerous for switching.

On a real upgraded F45 host: `/etc/yum.repos.d` contains only third-party files (e.g. COPR) and inert `.rpmsave` copies of the pre-upgrade (possibly app-switched) files; the app currently finds nothing switchable.

## Goals / Non-Goals

**Goals:**
- One code path per concern: detection, reading (merged inventory), planning (effective file), writing (shadow copy on F45+).
- Keep the privileged surface (`fmm-apply`, polkit policy, snapshot root) operating on `/etc/yum.repos.d` only — `/usr` is never written.
- Maximal reuse of the existing line-preserving rewriter, hash-verified apply, snapshot and restore machinery.

**Non-Goals:**
- Migrating pre-upgrade `.rpmsave` files back into effect (the user re-switches via the UI).
- Writing `/etc/dnf/repos.override.d` drop-ins (see Decision 2).
- Supporting ostree/Silverblue `/usr` writability (irrelevant: `/usr` is never written).
- Any change to the mirror catalog, URL mapping, classification rules, or the plan protocol version.

## Decisions

### Decision 1: Feature-based layout detection, not an OS-version lookup

New `fmm_core::reposd::detect_layout()`: F45+ iff `/usr/share/dnf5/repos.d` exists and contains ≥ 1 `*.repo` file; else legacy. `AppState` gains `layout` (and `packaged_dir`); every command re-runs detection or uses the stored one refreshed together with the inventory.

- Why not `/etc/os-release` `VERSION_ID`: derivative/rawhide edge cases, breakage if the layout is backported, and the feature itself is exactly what matters (where the files are).
- Why not per-call probing everywhere: one detection point keeps `inventory`, `plan` and the UI consistent; refresh re-runs it so an OS upgrade is picked up without restarting.

### Decision 2: Shadow copy into `/etc/yum.repos.d` (not override drop-ins, not in-place `/usr` writes)

On F45+ a switch writes the rewritten content as a **complete copy of the current packaged file** into `/etc/yum.repos.d` under the same file name. The first `reposdir` owns the section (verified), so the shadow takes effect including the removal of `metalink`/`mirrorlist` — the existing `rewrite_section` already produces exactly this content.

Alternatives rejected:
- **In-place edit of `/usr/share/dnf5/repos.d`**: package-owned, unconditionally replaced on package updates, contrary to the F45 model, and `/usr` may be read-only on some systems.
- **Key-level overrides in `/etc/dnf/repos.override.d`**: to work, it would need `metalink=`/`mirrorlist=` empty-value entries alongside `baseurl` (verified only indirectly via `--setopt`), must coexist alphabetically with `config-manager`'s `99-config_manager.repo` (ordering fights), and would force new target-dir, create-file, snapshot and restore semantics through the whole privileged path — all for zero UX gain. The shadow approach reuses `repo_dir`, `snapshot_root`, `fmm-apply` and restore as-is.

Consequence: shadow files can go stale when a package update changes the packaged file. Each switch **rebuilds** the shadow from the current packaged file (see Decision 3), so staleness self-heals on the next switch; inventory always shows the shadow, which *is* the effective config.

### Decision 3: Planning against the effective file, sourced from the packaged file

`build_action_plan` resolves the owning file of the target section:
- Owning file in `/etc/yum.repos.d` (legacy file, third-party file, or existing shadow) and it is the file to rewrite → current behavior: rewrite in place, `before_sha256` = hash of current content.
- F45+ and no local file for the packaged file's name → build the shadow: load the packaged file, apply `rewrite_section`, destination `/etc/yum.repos.d/<same basename>`, `before_sha256` = `""` (sentinel: "file must not exist at apply time").
- NoChange detection keeps using the effective entry's source classification, so re-picking the current mirror stays a no-op.

`PlanFile.before_sha256 = ""` is unambiguous (sha256 of any content is never empty) and needs no protocol version bump; old plans never contain it.

### Decision 4: Merged inventory keyed by section id, with origin

`inventory()` reads the locations of the detected layout: legacy → `/etc/yum.repos.d`; F45+ → both, `/etc/yum.repos.d` last-applied so its section ids win (matches dnf5: first reposdir wins). `RepoEntry` gains `origin: packaged | local` (`local` always on legacy). `Inventory` gains `layout` so the UI can label the view. Per-file errors keep their file attribution from whichever location the file is in. `.rpmsave` files stay invisible (extension filter already excludes them).

### Decision 5: `fmm-apply` learns to create files

The write step branches on the sentinel: `before_sha256 == ""` → the target must not exist (existing file ⇒ `stale_plan`), then create with mode 0644, root:root (mode/ownership of packaged repo files); otherwise the current hash-verified in-place rewrite. Restore, snapshot, prune, cache refresh: unchanged — they already operate on `repo_dir` = `/etc/yum.repos.d`, which on F45+ contains exactly the app-managed state.

## Risks / Trade-offs

- [Shadow stale after a fedora-repos package update] → every switch rebuilds from the current packaged file; a no-op re-pick reports NoChange without healing, accepted (the effective config is still what the shadow says).
- [dnf5 merge semantics change in a future release] → the probes verified the documented F45 compatibility contract ("modified files override /usr entirely") on the shipping dnf5 5.4.6; the inventory/plan model mirrors that contract, so drift would be an upstream behavior change, not an app bug.
- [Third-party file defines a section id that also exists packaged] → precedence rule gives `/etc` ownership, same as dnf5; entry is shown with `origin: local` and classified by its own id/file name.
- [User hand-edits other sections of a shadow file] → the next switch rebuilds those sections from the packaged file; the preview shows the full new content, and the pre-write snapshot always covers recovery.
- [Empty `before_sha256` sentinel collides with a user value] → impossible: it is produced only by the planner; sha256 output is 64 hex chars.

## Migration Plan

No data migration: detection activates the new behavior per host at runtime. Shadow files created in `/etc/yum.repos.d` are ordinary dnf config; rollback is reverting the app and (optionally) restoring a snapshot, which removes them via the existing purge path. Legacy hosts keep the old behavior bit-for-bit.

## Open Questions

None — the two behaviors the design depended on (section ownership across reposdirs, empty-value clearing) were verified empirically on the target dnf5; the remaining unknown (which of `baseurl`/`metalink` wins when both are set) is irrelevant because written sections never carry both.
