# Proposal

## Why

The repositories table lists entries in backend file order, so finding a particular repository (or grouping enabled ones, or spotting all metalink sources) means scanning the whole list by eye. Letting the user sort by any data column makes the inventory readable at a glance.

## What Changes

- The repositories table (`ui/src/repos.ts`) gets clickable column headers: Repository, File, State, Source, and URL can each sort the rows; the first click sorts ascending, a second click reverses to descending.
- The Action column is not sortable (it holds buttons, not data) and its header is not interactive.
- Sorting is client-side in the UI; the backend inventory API and the `fmm-core` crates are unchanged.
- The chosen sort (column + direction) persists across Refresh and after apply-triggered reloads within the session; with no user sort selected, rows keep the current backend order.
- Column headers show the active sort direction and expose it to assistive technology (`aria-sort`).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `repo-status`: adds a requirement that the inventory table's data columns (Repository, File, State, Source, URL) can sort the rows ascending/descending by clicking the header, that the chosen sort survives a refresh, and that the unsorted default order stays the backend order.

## Impact

- `ui/src/repos.ts` — header rendering, row ordering, and per-panel sort state.
- `ui/src/style.css` — header affordance (cursor, sort indicator) for sortable headers.
- No backend, protocol, helper, or packaging changes; no new dependencies.
