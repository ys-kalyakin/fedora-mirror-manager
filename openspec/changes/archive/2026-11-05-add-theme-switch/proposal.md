# Proposal

## Why

The app currently has no way to choose its color theme: light or dark is dictated entirely by the GNOME system color scheme. The user wants a one-click in-app control to switch between light and dark, so the UI can be flipped without leaving the app or changing system settings.

## What Changes

- Add a theme toggle button to the app header that switches the UI between light and dark in one click.
- Introduce explicit CSS theme tokens (design tokens) so light and dark palettes are selectable, replacing the current hard-coded colors plus a single `@media (prefers-color-scheme: dark)` override block.
- Until the user makes an explicit choice, the app keeps following the system color scheme (current behavior preserved), including live re-theming when the system scheme changes.
- An explicit choice is persisted (browser `localStorage`) and survives app restarts, overriding the system scheme while it exists. (Switching back to system-following is out of scope for the two-state button.)

## Capabilities

### New Capabilities

- `theme-switch`: The in-app light/dark theme control — defaulting to following the system scheme, switching on one click, persisting an explicit choice, and keeping the whole UI (panels, table, badges, dialogs, status bar) correctly themed in both modes.

### Modified Capabilities

<!-- None: all existing capabilities (helper-setup, mirror-switch, repo-status, snapshot-rollback) describe backend behavior and state no requirements about UI appearance. -->

## Impact

- `ui/src/style.css` — colors refactored into CSS custom properties; the `prefers-color-scheme` media block is replaced by token overrides on a theme attribute.
- `ui/src/shell.ts` — header gains the toggle button; new `ui/src/theme.ts` (or equivalent) owns theme state, system-scheme tracking, and persistence.
- No backend (`src-tauri`, `crates`) changes; no new dependencies; no Tauri capability/permission changes — theming is pure WebView-side state in `localStorage`.
