# Tasks

## 1. Layout detection and merged inventory (`fmm-core`)

- [x] 1.1Add `detect_layout()` to `crates/fmm-core/src/reposd.rs` (F45+ iff `/usr/share/dnf5/repos.d` contains ≥ 1 `*.repo` file, else legacy) plus a `Layout` type carrying the packaged/local directory paths; export via `lib.rs`. Verify: unit test with temp dirs (empty dir → legacy, dir with `x.repo` → F45+, missing dir → legacy).
- [x] 1.2Extend `inventory()` to read the locations of the detected layout: merge both dirs on F45+ with `/etc/yum.repos.d` section ids overriding packaged ones, add `origin: packaged|local` to `RepoEntry` and `layout` to `Inventory`. Verify: `crates/fmm-core/tests/inventory.rs` gains F45+ fixtures (packaged dir + etc dir, an overridden section, a third-party file) asserting merged entries, precedence, origins and per-file errors; legacy fixtures still pass unchanged.
- [x] 1.3Rework the missing/empty directory errors so they name the locations of the current layout (F45+ distinguishes "neither location has repositories" from a genuinely missing directory). Verify: unit tests for both layouts' empty and missing cases produce the documented error/empty states.

## 2. Planning against the effective file (`fmm-core`)

- [x] 2.1Extend `build_action_plan` to resolve the owning file of the target section: rewrite in place when a local file owns it; otherwise build the shadow plan (load packaged file, `rewrite_section`, destination `/etc/yum.repos.d/<basename>`, `before_sha256 = ""`). Keep NoChange detection on the effective entry's source. Verify: `crates/fmm-core/tests/plans.rs` gains cases — first switch on F45+ produces a shadow plan with empty sentinel and packaged path as source; second switch targets the existing shadow in place; switch of a third-party-owned section targets that file; NoChange re-pick on F45+.
- [x] 2.2Add `target_path` (full destination path) to `PlanPreview`, populated for both layouts. Verify: plan tests assert the preview path per layout; serialization round-trip covers the new field.

## 3. Helper create-file support (`fmm-apply`)

- [x] 3.1In `apply_switch`, branch on the sentinel: `before_sha256 == ""` requires the target file to be absent (existing file ⇒ `stale_plan`) and creates it with mode 0644, root:root; otherwise keep the hash-verified in-place rewrite. Verify: `crates/fmm-apply/tests/apply.rs` covers create-new (file appears with expected mode/owner), create-conflict (file exists ⇒ `stale_plan`), and the existing rewrite path unchanged.

## 4. Tauri commands and state (`src-tauri`)

- [x] 4.1Extend `AppState` with the layout/packaged dir, keep every command's signatures stable, and make `list_repos`/`preview_action`/`apply_restore` use the detected layout (inventory carries `layout`; preview carries `target_path`). Verify: `cargo build -p <app>` compiles; existing command tests pass; `helper_status`-style tests unaffected.
- [x] 4.2Update the two hardcoded UI-facing error strings at their source and any `CmdError` message that mentions only `/etc/yum.repos.d`. Verify: grep finds no backend user-facing string claiming the legacy path unconditionally.

## 5. UI: layout indication and origin

- [x] 5.1Show the detected layout in the repositories panel (e.g. a badge/hint "F45+ (dnf5 repos.d)" vs "classic /etc/yum.repos.d") and an `origin` marker (packaged/local) on entries when the inventory reports F45+. Verify: `ui/` builds (`npm run build`) and the rendered panel shows the badge with stub data; legacy view hides the origin marker.
- [x] 5.2Reword the empty/error states in `ui/src/repos.ts` per layout (use the layout/locations from the inventory instead of the hardcoded `/etc/yum.repos.d` strings). Verify: strings render from layout-aware data in the same stub check.

## 6. Integration verification

- [x] 6.1Full workspace check: `cargo test --workspace` and `cargo clippy --workspace` (toolchain via `rust-toolchain.toml` bin path) pass, including the new tests.
- [ ] 6.2 On this F45 host, build the app and verify end-to-end behavior read-only-safe: inventory shows packaged + third-party entries with layout badge; a previewed switch names the `/etc/yum.repos.d` shadow path. Verify: manual run of the built app (`cargo tauri dev` or the built binary), observed inventory and preview.

## Workflow follow-up

- Archive the change with `/opsx:archive` after apply (syncs the delta specs into `openspec/specs/`).
- Optional follow-up change (not this one): offer to import pre-upgrade `.rpmsave` switch states.
