import { invoke } from "@tauri-apps/api/core";
import { el } from "./dom";
import { initRepos } from "./repos";
import { initSnapshots } from "./snapshots";

/** Status payload returned by the backend `app_status` command. */
interface AppStatus {
  version: string;
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
