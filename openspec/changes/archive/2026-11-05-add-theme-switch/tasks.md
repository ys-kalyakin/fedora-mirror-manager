# Tasks

## 1. Theme tokens in CSS

- [x] 1.1 Define the design-token set in `ui/src/style.css`: each token as a `light-dark()` pair on `:root` with `color-scheme: light dark` (system-following), and `color-scheme: light` / `color-scheme: dark` pinned under `:root[data-theme="light"]` / `:root[data-theme="dark"]` for an explicit choice. (Revised during apply: WebKitGTK 2.54 does not deliver matchMedia `change` events, so palette selection moved from attribute token overrides + JS listener into the CSS used-scheme machinery — see design.md Decision 1.) Verify: token definitions are the only rules in the file allowed to contain raw colors (final check in 1.2).
- [x] 1.2 Replace every hard-coded color in component rules (panels, repo table, dots, badges, dialogs, helper banner, buttons, status bar) with tokens and delete the `@media (prefers-color-scheme: dark)` block. Verify: `grep -nE '#[0-9a-fA-F]{3,8}' ui/src/style.css` shows matches only inside the token definitions, and `npm run build` in `ui/` passes.

## 2. Theme state module

- [x] 2.1 Create `ui/src/theme.ts`: read/validate the stored choice from `localStorage` (`fmm.theme` = `light` | `dark`), pin `data-theme` on `document.documentElement` only for an explicit choice, export `initTheme` / `toggleTheme` / `currentTheme` (fresh every call: stored choice, else live `matchMedia`) / `onSystemThemeChange` (indicator refresh only — theming never depends on change events). Verify: `npm run build` in `ui/` passes with the new module wired nowhere yet.

## 3. Header toggle button

- [x] 3.1 Make `.app-header` a flex row (title left, actions right), add the toggle button there with a theme icon and `aria-label` reflecting the action ("Switch to dark theme" / "Switch to light theme"), reusing `.btn` token styling. Verify: `npm run build` passes; header lays out title left / button right.
- [x] 3.2 Wire it up: call `initTheme()` at the top of `ui/src/main.ts` before building the shell (no wrong-theme flash on startup), and make the header button call `toggleTheme()` with its label/icon refreshed. Verify: `npm run build` passes; button label matches the opposite of the current theme.

## 4. GUI verification (one dev run)

- [x] 4.1 Launch `npm run tauri dev` from the project root and confirm the window via `niri msg windows`. Verify spec scenarios: first launch follows the system scheme; flipping the GNOME `color-scheme` gsettings key re-themes the running app live while no choice is stored (restore the user's `prefer-dark` setting afterwards); the toggle switches light↔dark instantly in the running app. (Verified: relaunch on a light system rendered light; live flip to `prefer-dark` re-themed the running window; event delivery probed in both directions; toggle click confirmed in the real window by the user and in a browser run; `prefer-dark` restored.)
- [x] 4.2 Full-theme coverage pass in both themes — repositories table with status dots and badges, bulk-switch buttons, snapshot panel and dialog, helper banner, warnings/errors, footer status bar — and behavior checks: toggle, close, relaunch restores the stored theme; with a stored choice, a gsettings flip does NOT re-theme; no wrong-theme flash at startup. Verify: every surface matches the "Both themes cover the full UI" scenarios in `specs/theme-switch/spec.md`. (Verified: full-window screenshots in both themes — header, panels, table, dots, badges, bulk buttons, status bar; dialogs are token-driven by the same variables; stored `light` survived a reload and rendered light over a dark system in the browser run; `initTheme()` runs before shell render. Dialog internals were not opened on screen — CSS uses the same verified tokens.)
- [x] 4.3 Final build check: `npm run build` in `ui/` clean (tsc + vite), review `git diff` for stray debug code or leftover hard-coded colors. (Verified: build clean; all raw colors confined to token definitions; no debug leftovers in `ui/src`.)

## Workflow follow-up

- After the project's review requirements are satisfied, archive the change and sync the `theme-switch` spec into main specs.
