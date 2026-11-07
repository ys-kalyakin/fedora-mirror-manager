# Design

## Context

The repositories panel (`ui/src/repos.ts`, `initRepos`) renders the only table in the app: header row + one `row()` per `RepoEntry`, rebuilt from scratch on every `load()` (initial, Refresh button, `fmm:applied` events). Entries arrive from the backend in file/precedence order and are rendered in that order today. Column widths and header styles live in `ui/src/style.css` under `.repo-table`. No UI framework — plain DOM built with the `el()` helper in `ui/src/dom.ts`.

## Goals / Non-Goals

**Goals:**

- Click-to-sort on the five data columns; visual + `aria-sort` indication of the active sort.
- Sort state survives re-renders within the session.
- Keep the existing default (backend) order when the user has not sorted.

**Non-Goals:**

- Server-side or backend sorting; sorting of any other view (snapshots is a plain list); multi-column sort; persisted sort across app restarts; column reordering or hiding.

## Decisions

- **Client-side sort in `repos.ts`, no new module or dependency.** The inventory is small (local `.repo` files), and sorting is a pure view concern over already-fetched data. Alternative — sorting in `fmm-core` or the Tauri command — rejected: it would add an API round-trip per click and change backend surface for no benefit.

- **Sort state lives in a module-level variable inside `initRepos`'s closure** (e.g., `let sort: { column: SortColumn; dir: "asc" | "desc" } | null = null`), applied at render time in `render()`. Because every `load()` goes through `render()`, the choice survives refreshes for free. Resetting on reload would fight the user's expectation; not persisting across app restarts is acceptable for a view preference. Alternative — DOM-only resorting of existing `tr` nodes — rejected: re-render is already the established pattern and keeps one code path.

- **Click cycles two states, not three.** First click ascending, second descending; no third click back to "unsorted". A three-state cycle is harder to discover and the default order has little value once the user has sorted. Header text/indicator makes the active state obvious.

- **Sort keys per column**, defined in one place (a comparator map):
  - Repository → `entry.id.toLowerCase()`
  - File → `entry.file.toLowerCase()`
  - State → `entry.enabled` (false < true: disabled first ascending)
  - Source → `SOURCE_LABELS[entry.source]` compared case-insensitively (sorts by the label the user sees)
  - URL → `entry.url`; `null` compares greater than any URL so missing URLs land last in both directions (return a sentinel from the comparator rather than comparing `null` as a string)
- **Stability:** copy before sorting — `[...inv.entries].sort(cmp)` — since `ui/tsconfig.json` targets ES2021 (no `Array.prototype.toSorted`, and mutating the caller's `inv.entries` is a side effect to avoid). `Array.prototype.sort` is stable per spec, so equal keys keep their current order.
- **Headers:** each sortable `th` gets a real `<button>` inside (keyboard operable, matches existing `.btn` patterns without inheriting button chrome — styled bare). The `th` carries `aria-sort="ascending" | "descending"` only while active, plus a visual indicator via CSS `::after` (▲/▼) driven by a `repo-table__th--sort-asc/desc` class on the `th`. The Action `th` stays a plain header.

## Risks / Trade-offs

- [Sort indicator may shift header text/width] → Reserve space with the `::after` pseudo-element on all sortable headers (neutral glyph when unsorted, or hidden-but-present), so the existing fixed column widths (`.repo-table th:nth-child(n)`) are not disturbed; verify the `Table columns keep stable widths` scenarios still hold.
- [Sort comparator must not mutate `inv.entries`] → Sort a shallow copy (`[...inv.entries]`); the original inventory stays in backend order for the "default order" state.
- [User sorts while a load is in flight] → Sort state is read at render time; a click during "Loading repositories…" is impossible (table is replaced by the loading note), so no race.

## Migration Plan

UI-only change; no data, protocol, or packaging migration. Rollback = revert the commit.

## Open Questions

None.
