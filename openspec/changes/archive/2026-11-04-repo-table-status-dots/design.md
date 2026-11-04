# Design

## Context

Observed in the current code:

- `ui/src/repos.ts:105-111` renders the enabled state as a text pill (`badge badge--on|badge--off`, text "enabled"/"disabled") and the source as a `badge badge--source-*` pill.
- `ui/src/style.css` uses `table-layout: fixed` with percentage column shares (Enabled 7%, Source 9%, Repo/File 18% each, Action 18%) and applies `overflow-wrap: anywhere` to **every** cell (`.repo-table td`). "disabled" (8 chars, pill padding) needs ~78 px but 7% of the ~1050 px table minus cell padding is ~57 px, so the pill wraps and rows look misaligned. "HyperDedic" needs ~84 px vs ~78 px available in the Source column, and `overflow-wrap: anywhere` lets it break mid-word onto a second line.
- Dark-mode variants for all badges live in the `prefers-color-scheme: dark` block.

Constraints: Tauri/WebKitGTK frontend, no build-time CSS processing (plain `style.css`), existing project rule: div overlays not `<dialog>` (unrelated here but sets the "plain, hand-rolled markup" idiom of the UI).

## Goals / Non-Goals

**Goals:**

- Enabled/disabled rendered as a fixed-size green/red dot with tooltip + accessible name.
- Badge labels never break mid-word; Source column fits "HyperDedic" on one line.
- Column shares rebalanced so freed Enabled width goes to URL and no column overflows into a neighbor.

**Non-Goals:**

- No changes to Action column buttons themselves (labels, handlers, or their wrap-between-buttons behavior); the cell structure fix in decision 6 does not touch the buttons.
- No changes to inventory data, classification, backend, or dialogs.
- No general table redesign (no sticky headers, sorting, virtualization).

## Decisions

1. **Dot as an inline `<span>` with `role="img"` + `aria-label` + `title`.**
   `<span class="repo-table__dot repo-table__dot--on" role="img" aria-label="enabled" title="enabled">`. Fixed 0.75rem circle via `width/height/border-radius: 50%`, `display: inline-block`, `flex-shrink` irrelevant (no flex parent). Alternatives: (a) keep text pill with `white-space: nowrap` — rejected, user explicitly asked for the schematic dot; (b) render the dot via CSS pseudo-element on the `<td>` — rejected, worse for accessibility and testing; (c) visually-hidden text span next to the dot — rejected, more markup for the same screen-reader result as `role="img"` + `aria-label`.

2. **Dot colors defined per color scheme.** Light: green `#16a34a`, red `#dc2626`. Dark (existing dark block): brighter `#4ade80` / `#f87171` for contrast on dark panel background. Solid fill, no border, identical geometry in both states so nothing shifts.

3. **`white-space: nowrap` on all table badges.** One rule (`.repo-table .badge { white-space: nowrap; }`) covers source and origin pills. The global `overflow-wrap: anywhere` on `.repo-table td` stays — it is what makes long URLs/ids/file names wrap inside their own cells, and `nowrap` on the badge overrides it locally. Trade-off: with `table-layout: fixed`, a `nowrap` badge that does not fit paints over the next column, so the width rebalance (decision 4) is a correctness requirement, not cosmetic.

4. **New column shares:** Repository 16%, File 16%, State 7%, Source 11%, URL auto (~32%), Action 18%. Rationale: Source grows 9%→11% (badge ~82 px + cell padding 16 px ≈ 98 px < 11% ≈ 114 px); Repo/File give up 2% each; URL gains ~2%. The State column keeps 7% even though the dot needs only ~28 px, because the **header word** is the binding constraint: at 4% the header painted over the Source header in the running app (verified by screenshot). The header is renamed "Enabled" → "State" (in `ui/repos.ts`) to fit the compact column; the first draft (4% + "Enabled") was corrected after visual verification. Update the `nth-child` width rules and their comment.

5. **Remove `.badge--on`/`.badge--off`** (light and dark rules). Grep confirms `ui/src/repos.ts:106` is the only usage. The enabled column no longer uses the badge classes at all; keeping dead rules invites drift.

6. **Action cell: plain `<td>` with an inner flex `<div>`** (added after user feedback during verification: the Action column's row borders did not align with the other columns). `.repo-table__actions { display: flex }` was set on the `<td>` itself; `display: flex` overrides `display: table-cell`, so the cell stopped stretching to the row height and shrank to its content — its bottom border sat above the row's bottom. The td is now classless (a real table cell that stretches with the row and honors `vertical-align: top`) and the flex/flex-wrap/gap styling lives on an inner `div.repo-table__actions`.

## Risks / Trade-offs

- [Source badge overflows its fixed column under aggressive font scaling / tiny window] → Mitigation: 11% share leaves ~20 px headroom over the measured worst label; the badge is the widest fixed text in the table. Accept residual risk: at extreme zoom a badge may touch the next column, same failure mode as any fixed-layout table.
- [`role="img"` support in WebKitGTK accessibility bridge may be imperfect] → Mitigation: native `title` tooltip still conveys the state on hover; the dot colors are the primary channel per the user's request.
- [Green/red both used: red "disabled" may read as an error] → Intentional: the user explicitly requested green/red; the tooltip disambiguates.

## Migration Plan

None — pure frontend change, no data or API impact. Rollback = git revert of the two files.

## Open Questions

None.
