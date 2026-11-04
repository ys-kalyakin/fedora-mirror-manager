# Tasks

## 1. Enabled-state dot

- [x] 1.1 In `ui/src/repos.ts` (enabled cell, row renderer): replace the `badge badge--on|badge--off` text pill with `<span class="repo-table__dot repo-table__dot--on|repo-table__dot--off" role="img" title="enabled|disabled" aria-label="enabled|disabled">` (no text content). Verify: `grep -n "badge--on\|badge--off" ui/src/repos.ts` returns nothing and the dot element attributes appear in the built table DOM.
- [x] 1.2 In `ui/src/style.css`: add `.repo-table__dot` geometry (fixed 0.75rem circle, `border-radius: 50%`, `display: inline-block`) with light-mode colors (`--on: #16a34a`, `--off: #dc2626`) and remove the light-mode `.badge--on`/`.badge--off` rules. Verify: `grep -n "badge--on\|badge--off" ui/src/style.css` only matches the dark-mode block until task 1.3 lands; dots render as fixed-size circles in the running app.

## 2. No-wrap badges and column widths

- [x] 2.1 In `ui/src/style.css`: add `.repo-table .badge { white-space: nowrap; }`. Verify: in the running app a `HyperDedic` source badge and a `packaged` origin badge (F45+ layout) each render on a single line, unbroken.
- [x] 2.2 In `ui/src/style.css`: update the `nth-child` column-share rules and their comment to Repository 16%, File 16%, State 7%, Source 11%, URL auto, Action 18%, and rename the third column header to "State" in `ui/src/repos.ts`. (The first draft gave the dot column 4% with the "Enabled" header; the running app showed the header word painting over Source — 7% + the shorter word is the smallest fit that cannot overlap.) Verify: the State column shows the dot with room to spare and no header, badge, or cell content paints over a neighboring column.
- [x] 2.3 In `ui/src/style.css` dark-mode block: replace the `.badge--on`/`.badge--off` rules with dark dot colors (`--on: #4ade80`, `--off: #f87171`). Verify: `grep -n "badge--on\|badge--off" ui/src/` returns nothing project-wide.

## 3. Integration verification

- [x] 3.1 Type-check and build the UI: `npm run build` in `ui/` exits 0.
- [x] 3.2 Launch the app (`npm run tauri dev` from the project root) and inspect the repositories table in light and dark mode: enabled/disabled rows show green/red dots of identical size, "HyperDedic" and origin badges stay on one line, long URLs wrap inside the URL column, and no row layout shifts between entries. Confirm with a screenshot review. (Verified via niri window screenshots in both color schemes; all badges present in the host's live data — `metalink`, `other`, `packaged`, `local` — render on one line; no HyperDedic-sourced repo exists on the host right now, so that badge was not exercised live — covered by the same class-wide nowrap rule plus 14 px width headroom.)

## 4. Follow-up: Action cell row alignment (user feedback after 3.2)

- [x] 4.1 In `ui/src/repos.ts`: make the Action cell a plain `<td>` (classless) and move `.repo-table__actions` (flex, flex-wrap, gap) onto an inner `<div>` wrapper, so the cell stretches to the row height and its bottom border aligns with the other columns' borders. Update the delta spec with an "Action cell spans the full row height" scenario and design.md with decision 6. Verify: `npm run build` exits 0 and a window screenshot shows the Action column's row borders aligned with the Repository/File/State/Source/URL columns' borders.
