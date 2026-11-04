/** In-app light/dark theme state.
 *
 * The active palette is selected in CSS via data-theme on the root element;
 * token definitions live in style.css. Until the user makes an explicit
 * choice, the app follows the system color scheme and re-themes live when it
 * changes. Activating the toggle stores the choice in localStorage; from then
 * on the stored theme wins over the system until the key is cleared.
 *
 * currentTheme() always re-derives the effective theme (stored choice, else
 * the live matchMedia state) instead of caching, so the header indicator and
 * the toggle stay correct even if a scheme-change event is ever missed.
 */

export type Theme = "light" | "dark";

const STORAGE_KEY = "fmm.theme";

const systemDark = window.matchMedia("(prefers-color-scheme: dark)");

function readStoredTheme(): Theme | null {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return raw === "light" || raw === "dark" ? raw : null;
  } catch {
    // Storage unavailable: behave as if no choice was made.
    return null;
  }
}

function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

/** Stored choice wins; otherwise mirror the system color scheme. */
function resolveTheme(): Theme {
  return readStoredTheme() ?? (systemDark.matches ? "dark" : "light");
}

/** Applies the initial theme before any UI is rendered and starts tracking
 * system scheme changes (ignored while a stored choice exists). */
export function initTheme(): void {
  applyTheme(resolveTheme());
  systemDark.addEventListener("change", () => {
    if (readStoredTheme() !== null) return;
    applyTheme(systemDark.matches ? "dark" : "light");
  });
}

/** Flips light↔dark, persists the choice, and returns the new theme. */
export function toggleTheme(): Theme {
  const next: Theme = currentTheme() === "dark" ? "light" : "dark";
  applyTheme(next);
  try {
    window.localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // The choice just won't survive the session if storage is unavailable.
  }
  return next;
}

/** The effective theme right now: the stored choice, else the live system
 * scheme (re-read on every call, never cached). */
export function currentTheme(): Theme {
  return resolveTheme();
}

/** Calls back when the system scheme flips while no explicit choice is
 * stored. Returns an unsubscribe function. */
export function onSystemThemeChange(callback: () => void): () => void {
  const listener = (): void => {
    if (readStoredTheme() === null) callback();
  };
  systemDark.addEventListener("change", listener);
  return () => systemDark.removeEventListener("change", listener);
}
