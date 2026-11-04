# Design

## Context

Today the UI theme is entirely system-driven: `ui/src/style.css` hard-codes light colors in base rules and redefines them inside one `@media (prefers-color-scheme: dark)` block (~130 lines), with `color-scheme: light dark` on `:root`. WebKitGTK re-themes the running window live when the GNOME `color-scheme` gsettings key flips. The header (`ui/src/shell.ts`, `.app-header`) currently holds only the title; panels use `.panel__header` / `.panel__actions` flex rows for buttons. There is no CSS custom-property system yet (only `--fedora-blue`).

## Goals / Non-Goals

**Goals:**
- Make light and dark both explicit, selectable palettes without duplicating selectors.
- Preserve the current system-following default and the live re-theme on gsettings flips for users who never touch the toggle.
- Keep the change UI-only: no backend, no new dependencies, no Tauri capability changes.

**Non-Goals:**
- No three-state control (light / dark / system) and no settings dialog for theme. The button is two-state as requested; once the user picks a side, system-following stops until the persisted choice is manually cleared (`localStorage`). Adding a "system" option can be a follow-up change if wanted.
- No per-component theme overrides or user-defined accent colors.

## Decisions

1. **Palette selection in CSS via `light-dark()` tokens and the used `color-scheme`.**
   Every hard-coded color in `style.css` becomes one CSS custom property holding a `light-dark()` pair. `:root` keeps `color-scheme: light dark`, so while no explicit choice exists the used scheme — and with it every token, the native form controls, and the canvas — follows the system color scheme and re-themes live. An explicit choice sets `data-theme="light"|"dark"` on the root, which pins `color-scheme` to that side; the stored choice then wins over the system. The `prefers-color-scheme` media block is deleted.
   *Alternative considered:* duplicating the dark token values under both `:root[data-theme="dark"]` and a media query guarded by `:not([data-theme="light"])` — rejected: two copies of the palette to keep in sync.
   **Why not the originally planned `[data-theme]` token overrides driven by a JS `matchMedia` listener:** implemented first, and the running app did not re-theme on a gsettings flip — this WebKitGTK (2.54.2) does not deliver MediaQueryList `change` events when the system scheme changes, while its CSS-level scheme tracking demonstrably re-themes live (the pre-change media-query build followed gsettings flips in place). Moving the palette decision into the CSS engine's used-scheme machinery removes the dependency on event delivery entirely.

2. **Theme state in a new `ui/src/theme.ts` module, attribute on `document.documentElement`.**
   It owns: (a) reading the stored choice from `localStorage` under one key, `fmm.theme` (`"light"` | `"dark"`); (b) `initTheme()` pinning the attribute only when a valid choice exists; (c) `toggleTheme()` flipping the effective theme, pinning the attribute, and persisting; (d) `currentTheme()` computing the effective theme fresh on every call — stored choice, else the live `matchMedia("(prefers-color-scheme: dark)").matches`; (e) `onSystemThemeChange()` for the header indicator to re-sync (guarded to fire only while no choice is stored). The header button also re-syncs its icon on hover/focus, so a missed `change` event cannot leave the icon lying: worst case it self-corrects the moment the user reaches for it.
   *Alternative considered:* a Tauri command + config file — rejected: a UI-only preference does not justify backend surface; `localStorage` survives WebView restarts in this app.

3. **Toggle button lives in the app header, right-aligned.**
   `.app-header` becomes a flex row: title left, actions right. The button reuses the existing `.btn` styling (now token-driven). Placement follows the pattern of `.panel__actions` rows; the header is always visible, unlike panel headers.

4. **Theme applied before first render.**
   `main.ts` calls the theme init before building the shell, so a stored dark choice does not flash light for a frame on startup (deterministic since `main.ts` is the single Vite entry).

## Risks / Trade-offs

- **Token refactor touches nearly every rule in a 611-line stylesheet** → missed color = wrong patch in one theme. Mitigation: after the refactor, grep the stylesheet for remaining raw hex values (only token definitions may contain them); manual visual pass over every panel/table/badges/dialogs in both themes as an explicit task.
- **`light-dark()` / `color-scheme` support in WebKitGTK** → verified available (webkit2gtk 2.54.2); the live system-following test is an explicit task. If a future WebKitGTK regression pins the used scheme, the fallback is the dual dark-token block (attribute + media query) noted in Decision 1.
- **WebKitGTK does not deliver MediaQueryList `change` events** (observed during implementation) → no theming path depends on them: the palette follows the used scheme in CSS, and `currentTheme()` re-reads live state. Residual exposure is only a stale header icon between system flips, self-corrected on hover/focus/click.
- **Persistence is per-WebView-profile `localStorage`** → a cleared profile resets to system-following. Accepted: that is the desired fallback behavior anyway.

## Migration Plan

None needed — new feature, no persisted data format to migrate, and the `localStorage` key is introduced read-tolerantly (missing/invalid value ⇒ system-following). Rollback is a plain revert; a leftover key is ignored by the reverted build.

## Open Questions

None.
