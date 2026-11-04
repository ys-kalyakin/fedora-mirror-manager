# Design

## Context

The switching machinery already supports everything a bulk switch needs except the batch itself: `ApplyPlan.files` is a `Vec<PlanFile>`, and the helper (`fmm-apply`) verifies **every** file hash before writing **any** file, then writes all of them — so a multi-file plan is already atomic with respect to staleness (one stale file aborts the whole batch before the first write) and is covered by one snapshot and one polkit prompt. Per-repository planning (`build_action_plan`) plans one section at a time and always produces a full-file rewrite, which is exactly why it cannot simply be called in a loop: two sections sharing a `.repo` file would yield two conflicting full-file rewrites whose sequential application fails on the second file's hash. The bulk feature is therefore a new planning function that groups candidates per file and merges their section rewrites into one content per file, reusing the existing rewrite primitives.

## Goals / Non-Goals

**Goals:**
- One unprivileged planning step per mirror producing a combined preview plus one multi-file `ApplyPlan`.
- Correct merge semantics when several switchable sections share a `.repo` file (local shadow rewrite or one overriding copy per packaged file).
- Explicit, user-visible skip reasons for every repository not included in the batch.
- Zero changes to the helper, the plan protocol, and the existing per-repository flow.

**Non-Goals:**
- A "restore to metalink"/"switch all back" action.
- Cisco OpenH264 bulk modes (disable/codecs stay per-row).
- Persisting a chosen mirror as a default or auto-switching on refresh.
- Any change to `PLAN_VERSION`, polkit policy, or `fmm-apply`.

## Decisions

### D1: New planner in a new `fmm-core` module (`bulk.rs`), not a loop over `build_action_plan`
`build_action_plan` is per-section and per-full-file; looping it cannot merge same-file rewrites. `build_bulk_plan(&RepoLocations, Mirror) -> Result<BulkOutcome>` runs one `inventory()`, selects candidates, groups them by target path, and builds one `RepoFile` rewrite per target by applying `rewrite_section(repo_id, Some(baseurl), None)` for each included section on the same parsed file. It reuses `catalog::baseurl` (so the URL mapping has a single source of truth), the packaged-sibling rebuild rule, and the sha256/create-sentinel conventions of `plan.rs`. Alternative considered — teaching `build_action_plan` to accept multiple repos: would complicate a stable, well-tested API for no gain; a sibling module keeps both flows readable.

### D2: Candidate rule — switchable AND carried AND not already on the mirror
A candidate must (a) have `Switchability::Switchable`, (b) have `catalog::baseurl(mirror, entry.kind)` yield `Some` (else skip: "HyperDedic does not mirror RPM Fusion" / generic "not carried"), and (c) have `entry.source != mirror` (else skip: "already on the mirror"). Condition (c) mirrors the single-switch `NoChange` semantics and prevents formatting-only rewrites of sections that already carry the correct `baseurl`. Consequence (accepted, consistent with the existing rebuild rule): when a local shadow is rewritten because a *sibling* section needs a change, the shadow is rebuilt from the current packaged file, so unrelated packaged drift in that file is absorbed — the same thing a single switch on that file does today. `Mirror` gets a `source()` mapping (Yandex→`yandex`, Hyperdedic→`hyperdedic`) for the comparison.

### D3: Grouping by target path with the existing ownership rules
For each candidate: if the effective owner (`entry.path`) is under `local_dir`, the target is that file itself (rewrite in place; content rebuilt from the packaged sibling when one exists, falling back to the file's own content if the sibling is unreadable — same fallback as the single switch). If the owner is packaged, the target is `local_dir/<entry.file>` with the empty-`before_sha256` create sentinel; if that destination already exists but does not define the section (the pathological case the single switch rejects), the entry is **skipped with a reason** instead of failing the batch. Files end up in the plan only through their candidate sections, so a file whose sections are all excluded never produces a write.

### D4: Per-file failures are skips, not batch errors
Only an inventory-level failure (layout detection, unreadable repo directory) fails the whole preview. A packaged file that cannot be read during grouping skips its sections with a reason ("cannot read …") instead of aborting the batch — the bulk action's contract is "switch everything it can", and every skip is shown in the preview dialog.

### D5: New serde types alongside `PlanPreview` (in `bulk.rs`)
```rust
pub enum BulkOutcome { NoChange { reason: String, skipped: Vec<BulkSkip> },
                      Plan { preview: BulkPreview, plan: ApplyPlan } }
pub struct BulkPreview  { mirror: Mirror, files: Vec<BulkFilePreview>, skipped: Vec<BulkSkip> }
pub struct BulkFilePreview { file, target_path, creates_file, source_path: Option<String>,
                             sections: Vec<BulkSectionPreview> }   // { repo_id, new_baseurl, removed_lines }
pub struct BulkSkip { repo_id: String, reason: String }
```
`NoChange` carries the skip list so the "nothing to switch" dialog can say why (e.g. "all 12 already use Yandex; 4 RPM Fusion repositories are not carried"). The snapshot tag is `switch-all-<mirror>` (e.g. `switch-all-yandex`), fitting the existing `<timestamp>-<tag>` snapshot id format. `ApplyPlan` itself is unchanged (`refresh_cache` stays `false` at plan time; `apply_plan` sets it from the dialog checkbox as today).

### D6: One new Tauri command; apply path reused as-is
`preview_bulk_switch(mirror: Mirror) -> Result<BulkOutcome, CmdError>` (thin, like `preview_action`). The UI applies through the existing `apply_plan(plan, refresh_cache)` — no new privileged path, no new error codes.

### D7: UI — toolbar above the table, one new confirm dialog variant
- `repos.ts`: the panel header hosts the bulk buttons **All → Yandex** and **All → HyperDedic** (from `Mirror`-driven data, tooltips stating the skip semantics) next to the Refresh button — placement chosen by the user during apply; clicks go through the same guards as row buttons (`anyDialogOpen()`), then preview → confirm → `applyPlan` → `showReport` → `notifyApplied`. A bulk `NoChange` reuses `showNoChange` with the summary reason.
- `apply.ts`: `confirmBulkPlan(title, preview, defaultRefresh)` — one dialog: intro line ("N repositories in M files will be switched to X"), one group per file (target path with created/rewritten marking, packaged-source hint, each section with its new `baseurl` and a collapsible removed-lines list), a skipped-repositories list with reasons, and the same cache-refresh checkbox. The dialog body scrolls (`max-height` + overflow) since a full batch can list many files; the existing single-preview dialog is untouched.

## Risks / Trade-offs

- [A whole-file hash mismatch from any unrelated edit aborts the entire batch] → This is the desired atomicity (spec scenario); the error names the file, and Refresh + retry rebuilds the plan.
- [Bulk dialogs get long (many files × sections)] → The dialog body scrolls; per-file groups are compact; removed lines stay in collapsible `<details>`.
- [Disabled repositories get their URLs rewritten] → Recorded assumption in the proposal; only URL lines change, the pre-write snapshot makes it one-click reversible via restore.
- [Rebuilding shadows absorbs unrelated packaged drift when a sibling section is included] → Same semantics as today's single switch on the same file; documented in D2 rather than special-cased.
- [Two mirrors offered but only two exist] → `Mirror::ALL` drives the buttons, so adding a mirror to the catalog automatically adds a bulk button.

## Migration Plan

No migration: no protocol, policy, helper, or data-format changes (the helper does not need reinstalling). Rollback of an applied batch is the existing snapshot restore. Feature rollback is a plain code revert.

## Open Questions

None.
