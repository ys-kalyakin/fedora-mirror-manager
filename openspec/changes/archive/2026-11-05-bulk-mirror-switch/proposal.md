# Proposal

## Why

Switching the whole system to one mirror today means clicking the same per-row mirror button on every switchable repository, one polkit authorization and one confirmation dialog at a time. A one-click bulk action per mirror removes that repetitive flow and matches how the app already treats other privileged work (one preview, one authorization, one report).

## What Changes

- Add bulk-switch buttons to the repositories panel header, next to Refresh — one button per mirror variant (**Yandex**, **HyperDedic**), the same fixed mirror set the per-row buttons offer.
- One button click plans a switch to that mirror for **every repository that can be switched to it**: all switchable (non-Cisco) repositories whose family the mirror carries and that are not already pointing at it. Repositories the mirror does not carry (RPM Fusion on HyperDedic), repositories already on the mirror, and non-switchable repositories are skipped, not touched.
- The whole batch is planned unprivileged as **one multi-file plan**: sections that live in the same `.repo` file are merged into a single rewrite of that file (multiple sections of one packaged file produce one overriding copy, not one per section).
- The combined preview is shown in a single confirmation dialog (files to create/rewrite, sections and new baseurls, skipped repositories with reasons, cache-refresh option); applying asks for **one** polkit authorization and creates **one** pre-write snapshot for the whole batch.
- No change to the per-row switch flow, the URL mapping, the section-rewrite rules, the F45+ override rules, or the plan protocol (`ApplyPlan` already carries multiple files; the helper already verifies every file hash before the first write and aborts without writing on a mismatch).

Assumptions (recorded, not user-confirmed):
- Disabled repositories are included in the bulk switch: switching only rewrites URL lines and never touches `enabled`, and the per-row buttons already allow switching a disabled repository.
- Repositories with a foreign (`other`) or missing (`none`) URL are switched as long as their family is classified switchable and carried by the mirror.
- Cisco OpenH264 repositories are not part of a mirror bulk switch (they have their own disable/codecs modes, not a mirror variant).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `mirror-switch`: adds bulk switching — the per-mirror bulk actions above the table, the "switch everything it can" selection rule with explicit skips, merged per-file planning for shared `.repo` files, the combined preview dialog, and the single-authorization/single-snapshot application of the batch. All existing per-repository requirements are unchanged.

## Impact

- `crates/fmm-core/src/plan.rs` (or a new `bulk.rs`): new bulk plan builder — candidate selection (switchable + carried + not already on the mirror), grouping by target file, merged section rewrites on one parsed file per target, per-file `before_sha256`/create-sentinel, `BulkPreview` with per-file details and skip reasons, snapshot tag `switch-all-<mirror>`.
- `src-tauri/src/commands.rs`: new `preview_bulk_switch(mirror)` command returning the bulk outcome; apply reuses the existing `apply_plan` passthrough unchanged.
- `ui/src/api.ts`: `previewBulkSwitch` binding and the bulk preview types.
- `ui/src/repos.ts`: bulk buttons in the panel header next to Refresh, wired through the confirm→apply→report flow.
- `ui/src/apply.ts`: combined confirmation dialog variant (batch summary, per-file groups, skipped list) alongside the existing single preview dialog.
- `crates/fmm-apply`: **no changes** — multi-file verify-then-write is already implemented.
- Tests: unit tests for the bulk builder (merged same-file rewrites, skip rules, F45+ single overriding copy per file, stale-hash abort), command-level tests, manual GUI verification.
