# Proposal

## Why

The repositories table still renders unevenly despite the earlier fixed-layout fix: the "enabled"/"disabled" text pill overflows its narrow column and shifts the row layout, and the source badge (e.g. "HyperDedic") breaks mid-word onto a second line. The user finds the table visually broken and has requested a schematic enabled/disabled indicator (green/red dot) instead of the text pill.

## What Changes

- Replace the "enabled"/"disabled" text badge in the repositories table with a fixed-size schematic indicator: a green dot for enabled, a red dot for disabled (hover tooltip and accessible label keep the meaning readable).
- Prevent source badge labels from breaking mid-word: the source badge never wraps inside its cell ("HyperDedic" stays on one line).
- Rebalance the table column widths to match the new content: the enabled column shrinks to a dot-sized share, the source column gets enough width for its longest label ("HyperDedic"), and the freed space goes to the URL column.
- Keep mid-word breaking where it is wanted (URLs, repository ids, file names) — the change only stops it for badge labels.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `repo-status`: adds a presentation requirement for the inventory table — the enabled state is shown as a fixed-size color dot (green enabled / red disabled) with a text alternative, badge labels (source, origin) never break mid-word, and the column layout stays stable so no cell content shifts or overlaps neighboring columns.

## Impact

- `ui/src/repos.ts` — the row renderer: enabled badge becomes a dot element with tooltip/aria label; source/origin badges unchanged structurally.
- `ui/src/style.css` — badge styles (`.badge--on`/`.badge--off` replaced by dot styles), new no-wrap rule for badges, adjusted column-width shares; dark-mode variants updated.
- No backend, Rust, or API changes; no data or CLI behavior changes.
- Visual-only change: verification is manual inspection of the running app (light and dark mode).
