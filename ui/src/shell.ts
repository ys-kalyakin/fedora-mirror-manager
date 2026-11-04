import { invoke } from "@tauri-apps/api/core";
import { el } from "./dom";
import { initRepos } from "./repos";
import { initSnapshots } from "./snapshots";
import { currentTheme, onSystemThemeChange, toggleTheme, type Theme } from "./theme";

/** Status payload returned by the backend `app_status` command. */
interface AppStatus {
  version: string;
}

/* Header toggle icons show the theme the click leads to: a sun while dark
   (switch to light), a moon while light (switch to dark). */
const SUN_SVG =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10Zm0-6h0a1 1 0 0 1 1 1v2a1 1 0 1 1-2 0V2a1 1 0 0 1 1-1Zm0 20a1 1 0 0 1-1-1v-2a1 1 0 1 1 2 0v2a1 1 0 0 1-1 1Zm10-9a1 1 0 0 1-1 1h-2a1 1 0 1 1 0-2h2a1 1 0 0 1 1 1ZM6 12a1 1 0 0 1-1 1H3a1 1 0 1 1 0-2h2a1 1 0 0 1 1 1Zm12.07-7.07a1 1 0 0 1 0 1.41l-1.41 1.42a1 1 0 1 1-1.42-1.42l1.42-1.41a1 1 0 0 1 1.41 0ZM8.76 15.24a1 1 0 0 1 0 1.41l-1.42 1.42a1 1 0 1 1-1.41-1.42l1.41-1.41a1 1 0 0 1 1.42 0Zm9.31 1.41a1 1 0 0 1-1.41 0l-1.42-1.41a1 1 0 0 1 1.42-1.42l1.41 1.42a1 1 0 0 1 0 1.41ZM8.76 8.76a1 1 0 0 1-1.42 0L5.93 7.34a1 1 0 0 1 1.41-1.41l1.42 1.41a1 1 0 0 1 0 1.42Z"/></svg>';
const MOON_SVG =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M21.64 13a1 1 0 0 0-1.05-.14 8.05 8.05 0 0 1-3.37.73 8.15 8.15 0 0 1-8.14-8.14 8.59 8.59 0 0 1 .25-2A1 1 0 0 0 8 2.36a10.14 10.14 0 1 0 14 11.69 1 1 0 0 0-.36-1.05Zm-9.5 6.69A8.14 8.14 0 0 1 7.08 8.22v.27a10.15 10.15 0 0 0 10.14 10.14 9.79 9.79 0 0 0 2.1-.22 8.11 8.11 0 0 1-7.18 1.28Z"/></svg>';

function themeButton(): HTMLButtonElement {
  const button = el("button", "btn app-header__theme-toggle", "");
  button.type = "button";
  const sync = (): void => {
    const next: Theme = currentTheme() === "dark" ? "light" : "dark";
    button.innerHTML = next === "dark" ? MOON_SVG : SUN_SVG;
    button.setAttribute(
      "aria-label",
      next === "dark" ? "Switch to dark theme" : "Switch to light theme",
    );
  };
  button.addEventListener("click", () => {
    toggleTheme();
    sync();
  });
  // The icon shows the theme the click leads to; re-sync on hover/focus and
  // on system scheme changes as a safety net against a missed change event.
  button.addEventListener("mouseenter", sync);
  button.addEventListener("focus", sync);
  onSystemThemeChange(sync);
  sync();
  return button;
}

async function loadStatus(statusEl: HTMLElement): Promise<void> {
  try {
    const status = await invoke<AppStatus>("app_status");
    statusEl.textContent = `Backend version: ${status.version}`;
  } catch (err) {
    statusEl.classList.add("status--error");
    const message = err instanceof Error ? err.message : String(err);
    statusEl.textContent = `Backend unavailable: ${message}`;
  }
}

/** Builds the application shell, the feature panels, and the status handshake. */
export function initShell(): void {
  const root = document.querySelector<HTMLElement>("#app");
  if (!root) {
    throw new Error("missing #app root element");
  }

  const header = el("header", "app-header", "");
  header.append(el("h1", "app-header__title", "Fedora Mirror Manager"));
  header.append(themeButton());

  const main = el("main", "app-main", "");
  const reposPanel = el("section", "panel", "");
  reposPanel.id = "repos-panel";
  const snapshotsPanel = el("section", "panel", "");
  snapshotsPanel.id = "snapshots-panel";
  main.append(reposPanel, snapshotsPanel);

  const status = el("footer", "app-status", "Checking backend…");
  root.append(header, main, status);

  initRepos(reposPanel);
  initSnapshots(snapshotsPanel);

  void loadStatus(status);
}
