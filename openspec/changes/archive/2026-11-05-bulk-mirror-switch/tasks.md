# Tasks

## 1. Core bulk planner (crates/fmm-core)

- [x] 1.1 Add `Mirror::source()` (Yandex→`yandex`, Hyperdedic→`hyperdedic`) with a unit test in `catalog.rs`; create `src/bulk.rs`, export it from `lib.rs`, and verify with `cargo test -p fmm-core`.
- [x] 1.2 Define the serde types (`BulkOutcome` with `NoChange { reason, skipped }` / `Plan { preview, plan }`, `BulkPreview`, `BulkFilePreview`, `BulkSectionPreview`, `BulkSkip`) per design D5 and add JSON round-trip tests (kebab-case outcome tag, `creates_file`, `source_path`) — `cargo test -p fmm-core` passes.
- [x] 1.3 Implement candidate selection in `build_bulk_plan(&RepoLocations, Mirror)`: include switchable entries with a `catalog::baseurl` for the mirror that are not already on it; skip with reasons "already on the mirror", "not carried" (RPM Fusion wording for HyperDedic), Cisco and not-switchable are never candidates — cover each rule in new tests in `crates/fmm-core/tests/plans.rs` (fixture repo dirs, both layouts).
- [x] 1.4 Implement grouping and the merged rewrite: entries sharing an effective local file produce exactly one `PlanFile` whose content carries every included section's new `baseurl` and leaves all other sections byte-identical; test a two-section file plus an untouched sibling file — `cargo test -p fmm-core` passes.
- [x] 1.5 Implement the F45+ paths: packaged file with several included sections yields one created overriding copy (empty `before_sha256` sentinel, `source_path` set, packaged file untouched); an existing local shadow is rewritten in place rebuilt from the packaged sibling; a conflicting destination that does not define the section is skipped with a reason instead of failing the batch — tests for all three in `tests/plans.rs`.
- [x] 1.6 Implement the outcome edges: no candidates → `NoChange` with a summary reason plus the skip list; unreadable packaged source during grouping → its sections are skipped with a reason while the rest of the batch is planned; plan snapshot tag is `switch-all-<mirror>` and `refresh_cache` stays `false` — tests in `tests/plans.rs` and `cargo test -p fmm-core` green.

## 2. Helper contract regression (crates/fmm-apply)

- [x] 2.1 Add an integration test in `crates/fmm-apply/tests/apply.rs` exercising the existing multi-file switch path the bulk flow relies on: a plan with several files writes all of them and reports each; a plan where one file's `before_sha256` no longer matches writes nothing and reports `stale_plan` — `cargo test -p fmm-apply` passes.

## 3. Command layer (src-tauri)

- [x] 3.1 Add `preview_bulk_switch(mirror: Mirror) -> Result<BulkOutcome, CmdError>` to `commands.rs` (same shape as `preview_action`) and register it in the invoke handler in `lib.rs`; verify with `cargo test` (workspace) and `cargo build -p fmm`.

## 4. UI (ui/src)

- [x] 4.1 Add the bulk types (`BulkOutcome`, `BulkPreview`, `BulkFilePreview`, `BulkSectionPreview`, `BulkSkip`) and `api.previewBulkSwitch(mirror)` to `api.ts`; verify with `npm --prefix ui run build` (type check).
- [x] 4.2 Add `confirmBulkPlan(title, preview, defaultRefresh)` in `apply.ts`: intro summary (N repositories in M files), one group per file with created/rewritten marking and packaged-source hint, per-section new `baseurl` with collapsible removed lines, skipped list with reasons, cache-refresh checkbox, Cancel/Apply; scrollable dialog body — verify via the built UI and a scripted preview render.
- [x] 4.3 Add the bulk buttons to the repositories panel header next to Refresh in `repos.ts` (placement chosen by the user during apply; buttons from `Mirror`-driven data, tooltips stating the skip semantics), wired through the `anyDialogOpen` guard → `previewBulkSwitch` → `confirmBulkPlan` → `api.applyPlan` → `showReport` + `notifyApplied`, with `NoChange` routed to `showNoChange`; add header-actions and dialog-scroll styles to `style.css` — verify with `npm --prefix ui run build` and `npm run tauri dev` launching (window visible via `niri msg windows`).

## 5. End-to-end verification

- [x] 5.1 `cargo fmt --check`, `cargo clippy`, and full `cargo test` are clean.
- [x] 5.2 Manual GUI check on the dev machine (`npm run tauri dev` from the project root): "All → Yandex" plans the batch, shows the combined preview with skipped repos and reasons, asks exactly one password prompt, writes all files, saves one snapshot, and the table shows Yandex sources after refresh; "All → HyperDedic" lists RPM Fusion as skipped; running a mirror's button again yields the "nothing to switch" dialog without a prompt; per-row buttons still work as before.
