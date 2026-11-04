# Tasks

## 1. Preview context fields (`fmm-core`)

- [x] 1.1 In `crates/fmm-core/src/plan.rs` add `creates_file: bool` and `source_path: Option<String>` to `PlanPreview` and fill them per branch: shadow-create → `creates_file=true`, `source_path=Some(packaged owner)`; in-place rewrite with a successful packaged-sibling rebuild → `creates_file=false`, `source_path=Some(packaged sibling)`; plain local/legacy rewrite and the packaged-vanished fallback → `creates_file=false`, `source_path=None`. Verify: `crates/fmm-core/tests/plans.rs` covers all four cases (legacy in-place, F45+ first switch, existing-shadow rebuild, third-party local) asserting both fields, and the serde round-trip test includes them.

## 2. UI rendering

- [x] 2.1 In `ui/src/api.ts` extend the `PlanPreview` type with `creates_file: boolean; source_path: string | null`. Verify: `npm run build` type-checks.
- [x] 2.2 In `ui/src/apply.ts` `previewRows()` render the three design cases: `creates_file` → `New file: <target_path>` with hint `Will be created — overrides <source_path>` (hint omitted when `source_path` is null); `!creates_file && source_path` → `File: <target_path>` with hint `Rewritten in place — content rebuilt from <source_path>`; otherwise the current plain `File: <target_path>` row. Verify: with stub preview objects in the running UI the dialog shows the expected rows for F45+ first switch, shadow rewrite, and legacy switch.
- [x] 2.3 In `ui/src/repos.ts` set `fileCell.title = entry.path` so every File cell exposes the absolute effective path. Verify: hovering the File cell of a packaged entry in the stub check shows `/usr/share/dnf5/repos.d/<name>.repo`.

## 3. Integration verification

- [x] 3.1 Full workspace check: `cargo test --workspace` and `cargo clippy --workspace` (toolchain bin path per `rust-toolchain.toml`) pass, including the updated plan tests.
- [ ] 3.2 On this F45 host, run the app (`npm run tauri dev` from the project root) and verify read-only: the preview for `fedora` shows the new-file row naming `/etc/yum.repos.d/fedora.repo` with the override hint naming `/usr/share/dnf5/repos.d/fedora.repo`; the repositories table tooltip for `fedora` shows `/usr/share/dnf5/repos.d/fedora.repo`; cancel the dialog so nothing is written.

## Workflow follow-up

- Archive the change with `/opsx:archive` after apply (syncs the delta specs into `openspec/specs/`).
