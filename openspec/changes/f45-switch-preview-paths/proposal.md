# Proposal

## Why

On a Fedora 45 host the mirror-switch confirmation dialog shows `File: /etc/yum.repos.d/fedora.repo` — the location `.repo` files had before F45. On F45 the packaged file lives at `/usr/share/dnf5/repos.d/fedora.repo` (verified on this host), and the `/etc` path does not exist yet: it is the overriding copy the switch will create. The preview names the correct write target but gives no "will be created" or source-file context, so it reads as a stale Fedora 44 path and the user cannot tell what will actually happen to their repositories.

## What Changes

- The switch preview gains backend-provided context fields: whether the target file will be created (does not exist yet) and the packaged `.repo` file the new content is derived from.
- On the F45+ layout, a preview for a packaged-owned repository names both paths: the packaged source file (read-only origin) and the overriding file in `/etc/yum.repos.d`, explicitly marked as created or rewritten in place.
- Legacy-layout previews keep the current single-path form.
- The repositories table makes the full path of the effective file visible per entry (tooltip on the File cell), so a `packaged` entry can be traced to `/usr/share/dnf5/repos.d/...`.
- The write target on F45+ is unchanged: `/usr/share/dnf5/repos.d` is still never written; only the clarity of what is shown changes.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `mirror-switch`: the "Changes are previewed before authorization" requirement is extended — on the F45+ layout the preview SHALL name the packaged source file and SHALL mark the target file as created or rewritten.
- `repo-status`: the "Entry origin is shown on the F45+ layout" requirement is extended — the full path of the effective file SHALL be visible per entry, not only the base name and origin badge.

## Impact

- `crates/fmm-core` (`plan.rs`): `PlanPreview` gains `creates_file: bool` and `source_path: Option<String>`; plan construction fills both for every layout/owner case.
- `src-tauri`: pass-through serde only; no command signature changes.
- `ui/src` (`api.ts`, `apply.ts`, `repos.ts`): preview rows render the new fields per case; table tooltip from the existing `entry.path`.
- Tests: `fmm-core` plan tests assert the new fields across legacy/F45+ and create/rewrite cases; UI verified with stub data.

## Assumptions

- The displayed `/etc/yum.repos.d/<name>.repo` path is the intended override target per the archived `fedora-45-repos-layout` change; the problem is presentation, not the write target. Writing to `/usr/share/dnf5/repos.d` remains forbidden.
- UI text stays in English, matching the rest of the UI.
