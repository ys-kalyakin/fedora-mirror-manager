# Tasks

## 1. Sortable headers in the repositories table

- [x] 1.1 In `ui/src/repos.ts`, add a sort-state variable in `initRepos`'s closure (`column` + direction, `null` = unsorted) and a comparator map over `inv.entries` copies implementing the five column keys per design.md (case-insensitive text, `enabled` false<true, source by label, `null` URL last in both directions); verify with `npm --prefix ui run build` that `tsc` passes.
- [x] 1.2 Make the header row interactive: wrap each of the five sortable header labels in a `<button>`, wire clicks to set/toggle the sort state (first click ascending, second descending; Action stays a plain `th`) and re-render the table body; verify clicking headers in the running app reorders rows (spec scenarios: Repository ascending, second click reverses, State groups, Source orders, URL nulls last, equal keys stable, Action inert).
- [x] 1.3 Reflect the active sort: set `aria-sort="ascending"/"descending"` on the sorted `th` only, add the direction class, and confirm keyboard operation (Tab to a header button, Enter sorts); verify in the running app.
- [x] 1.4 Keep the sort across reloads: confirm the state lives outside `render()`/`load()` so Refresh and `fmm:applied` re-renders reuse it, and that unsorted rendering still uses backend order; verify by sorting, clicking Refresh, and checking the order holds.

## 2. Header styling

- [x] 2.1 In `ui/src/style.css`, style sortable header buttons as bare interactive headers (inherit table header look, pointer cursor, focus outline) and add the asc/desc indicator (e.g. `::after` glyph) without changing the fixed column widths (`th:nth-child(n)`); verify with `npm --prefix ui run build` and by eyeballing column widths with and without an active sort (spec scenario: `Table columns keep stable widths` unchanged, headers expose direction visually).

## 3. Integration check

- [x] 3.1 Run the full app (`npm run tauri dev` or the packaged build) and walk the spec scenarios end to end: default file order on first load, each column sorts both ways, Action inert, sort survives Refresh and an applied switch, layout hint/errors/empty states unaffected; verify `npm --prefix ui run build` and, if Rust sources were touched (they should not be), `cargo test --workspace` still pass.
