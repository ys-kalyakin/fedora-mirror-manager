# Design

## Context

`build_action_plan` (`crates/fmm-core/src/plan.rs`) already resolves the owning file and branches between in-place rewrite and F45+ shadow creation; the shadow-create branch even has the empty-`before_sha256` sentinel. But `PlanPreview` carries only `target_path`, and the dialog (`ui/src/apply.ts`, `previewRows`) renders `File: <target_path>` with no context — which on F45+ reads as a stale pre-F45 path (see proposal.md — Why). `RepoEntry` already carries the absolute effective path (`entry.path`), unused by the table UI.

## Goals / Non-Goals

**Goals:**
- The preview states, from backend-provided data: whether the target file will be created or rewritten, and the packaged source file when the content derives from one.
- The repositories table makes the absolute effective file path available per entry.
- No behavior change in what is written or where.

**Non-Goals:**
- Any write into `/usr/share/dnf5/repos.d`, or any change to plan/apply semantics (`ApplyPlan` untouched).
- Re-laying-out the whole dialog; only the File row(s) gain context.
- Wire compatibility with old builds: frontend and backend ship together; plans are never persisted.

## Decisions

1. **Compute context in `fmm-core`, not the UI.** `PlanPreview` gains two fields, filled by the same branch that already knows the answer:
   - `creates_file: bool` — true only in the shadow-create branch (target absent today).
   - `source_path: Option<String>` — the packaged file the content derives from: always in the shadow-create branch, in the in-place branch only when the packaged-sibling rebuild actually succeeded; `None` for plain local/legacy rewrites and the packaged-vanished fallback.
   *Alternatives:* UI infers semantics from path prefixes + existence checks — rejected, the webview cannot stat files and duplicating layout rules in the UI invites divergence; reusing the `before_sha256 == ""` sentinel in the preview — rejected, leaks plan internals into a user-facing struct.

2. **Three render cases in `previewRows`** (`ui/src/apply.ts`):
   - `creates_file` → `New file: <target_path>` plus a hint row `Will be created — overrides <source_path>` (hint omitted if `source_path` is null).
   - `!creates_file && source_path` → `File: <target_path>` plus hint `Rewritten in place — content rebuilt from <source_path>`.
   - otherwise → `File: <target_path>` exactly as today (legacy, third-party files).
   The dialog intro line (`id (file)`) is unchanged.

3. **Table tooltip without backend changes** (`ui/src/repos.ts`): `fileCell.title = entry.path` for every entry on both layouts. `RepoEntry.path` already exists on the wire.

4. **`api.ts` mirrors the new fields** as `creates_file: boolean; source_path: string | null` — plain fields, no `#[serde(default)]`, since both sides deploy as one Tauri app.

## Risks / Trade-offs

- [Preview copy grows by one hint row on F45+] → hint lines are short and the dialog already wraps; the clarification is the point of the change.
- [`source_path` semantics subtly differ per branch (derived-from vs merely-present)] → the field is set only when content actually derives from the packaged file; fallback rewrites yield `None`, so the UI never claims a source that was not used.
- [Tests drift from the four owner cases] → plan tests enumerate all four explicitly (legacy in-place, first switch, existing-shadow rebuild, third-party local).

## Migration Plan

None: no persisted state or external interface is affected; the new fields are additive within one shipped binary pair.

## Open Questions

(none)
