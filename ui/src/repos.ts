import type { Inventory, Layout, RepoEntry, Source } from "./api";
import { api, isRpmFusion, notifyApplied, type Mirror } from "./api";
import {
  anyDialogOpen,
  confirmBulkPlan,
  confirmPlan,
  helperBanner,
  showErrorDialog,
  showNoChange,
  showReport,
  showBusy,
} from "./apply";
import { el } from "./dom";

/** The mirrors offered by the bulk actions, in offer order. */
const BULK_MIRRORS: Mirror[] = ["yandex", "hyperdedic"];

const MIRROR_LABELS: Record<Mirror, string> = {
  yandex: "Yandex",
  hyperdedic: "HyperDedic",
};

const SOURCE_LABELS: Record<Source, string> = {
  yandex: "Yandex",
  hyperdedic: "HyperDedic",
  metalink: "metalink",
  other: "other",
  none: "none",
};

/** Where the effective repository definitions are read from. */
const LAYOUT_LABELS: Record<Layout, string> = {
  legacy: "Layout: classic — repositories in /etc/yum.repos.d",
  "f45-plus": "Layout: Fedora 45+ — packaged repos in /usr/share/dnf5/repos.d, overrides and third-party in /etc/yum.repos.d",
};

const EMPTY_LABELS: Record<Layout, string> = {
  legacy: "No repositories found in /etc/yum.repos.d.",
  "f45-plus": "No repositories found in /usr/share/dnf5/repos.d or /etc/yum.repos.d.",
};

/** The repositories panel: inventory table with per-repo switch controls. */
export function initRepos(container: HTMLElement): void {
  const header = el("div", "panel__header", "");
  header.append(el("h2", "panel__title", "Repositories"));
  // Bulk actions live in the panel header, next to Refresh (user's choice):
  // one button per mirror switches everything it can in one batch.
  const headerActions = el("div", "panel__actions", "");
  for (const mirror of BULK_MIRRORS) {
    headerActions.append(bulkButton(mirror));
  }
  const refresh = el("button", "btn", "Refresh");
  refresh.addEventListener("click", () => void load());
  headerActions.append(refresh);
  header.append(headerActions);

  const body = el("div", "panel__body", "");
  container.append(header, body);

  async function load(): Promise<void> {
    body.replaceChildren(el("p", "panel__loading", "Loading repositories…"));
    try {
      const [inv, banner] = await Promise.all([api.listRepos(), helperBanner()]);
      render(body, inv, banner);
    } catch (err) {
      renderError(body, err as { code: string; message: string });
    }
  }

  // Refresh after any successful privileged action (own or snapshots panel).
  window.addEventListener("fmm:applied", () => void load());
  void load();

  function render(target: HTMLElement, inv: Inventory, banner: HTMLElement | null): void {
    target.replaceChildren();
    if (banner) {
      target.append(banner);
    }
    for (const fileError of inv.errors) {
      target.append(el("p", "panel__warning", `Could not read ${fileError.file}: ${fileError.message}`));
    }
    if (inv.entries.length === 0) {
      target.append(el("p", "panel__empty", EMPTY_LABELS[inv.layout]));
      return;
    }
    target.append(el("p", "panel__hint", LAYOUT_LABELS[inv.layout]));
    const table = el("table", "repo-table", "");
    const thead = el("thead", "", "");
    const headRow = el("tr", "", "");
    for (const label of ["Repository", "File", "State", "Source", "URL", "Action"]) {
      headRow.append(el("th", "", label));
    }
    thead.append(headRow);
    const tbody = el("tbody", "", "");
    for (const entry of inv.entries) {
      tbody.append(row(entry, inv.layout === "f45-plus"));
    }
    table.append(thead, tbody);
    target.append(table);
  }

  function row(entry: RepoEntry, showOrigin: boolean): HTMLTableRowElement {
    const tr = document.createElement("tr");

    const idCell = el("td", "repo-table__id", "");
    idCell.append(el("span", "repo-table__name", entry.id));
    idCell.append(el("div", "repo-table__hint", entry.name));
    tr.append(idCell);

    // Hover reveals the absolute path of the effective file (on F45+ a
    // packaged entry resolves under /usr/share/dnf5/repos.d).
    const fileCell = el("td", "", entry.file);
    fileCell.title = entry.path;
    if (showOrigin) {
      const originLine = el("div", "repo-table__hint", "");
      originLine.append(el("span", `badge badge--origin-${entry.origin}`, entry.origin));
      fileCell.append(originLine);
    }
    tr.append(fileCell);

    const enabledCell = el("td", "", "");
    const state = entry.enabled ? "enabled" : "disabled";
    const dot = el("span", `repo-table__dot ${entry.enabled ? "repo-table__dot--on" : "repo-table__dot--off"}`, "");
    dot.setAttribute("role", "img");
    dot.setAttribute("title", state);
    dot.setAttribute("aria-label", state);
    enabledCell.append(dot);
    tr.append(enabledCell);

    const sourceCell = el("td", "", "");
    sourceCell.append(el("span", `badge badge--source-${entry.source}`, SOURCE_LABELS[entry.source]));
    tr.append(sourceCell);

    const urlCell = el("td", "repo-table__url", "");
    urlCell.textContent = entry.url ?? "—";
    urlCell.title = entry.url ?? "";
    tr.append(urlCell);

    // The td itself must stay a plain table cell (it has to stretch to the
    // row height so its border aligns with the other columns); the flex
    // wrapper lives on an inner div.
    const actionCell = el("td", "", "");
    const actions = el("div", "repo-table__actions", "");
    if (entry.switchability === "switchable") {
      actions.append(mirrorButton(entry, "yandex"));
      actions.append(mirrorButton(entry, "hyperdedic"));
    } else if (entry.switchability === "cisco") {
      actions.append(ciscoButton(entry, "disable"));
      actions.append(ciscoButton(entry, "codecs"));
    } else {
      actions.append(el("span", "repo-table__hint", "not switchable"));
    }
    actionCell.append(actions);
    tr.append(actionCell);
    return tr;
  }

  /** Bulk action: switches every repository that can be switched to the mirror. */
  function bulkButton(mirror: Mirror): HTMLButtonElement {
    const label = MIRROR_LABELS[mirror];
    const button = el("button", "btn btn--small", `All → ${label}`);
    button.title =
      mirror === "hyperdedic"
        ? "Switches every repository that can be switched to HyperDedic (RPM Fusion repositories are not mirrored there). Enabled flags are unchanged."
        : "Switches every repository that can be switched to Yandex. Enabled flags are unchanged.";
    button.addEventListener("click", () => void planBulkSwitch(mirror));
    return button;
  }

  async function planBulkSwitch(mirror: Mirror): Promise<void> {
    // A modal is up: this click is a leftover double-click, not a new action.
    if (anyDialogOpen()) {
      return;
    }
    const label = MIRROR_LABELS[mirror];
    try {
      const outcome = await api.previewBulkSwitch(mirror);
      if (outcome.outcome === "no-change") {
        showNoChange(outcome.reason);
        return;
      }
      const decision = await confirmBulkPlan(mirror, outcome.preview, true);
      if (!decision.confirmed) {
        return;
      }
      const busy = showBusy(`Waiting for authorization and switching everything possible to ${label}…`);
      try {
        const report = await api.applyPlan(outcome.plan, decision.refreshCache);
        showReport(report);
        notifyApplied();
      } catch (err) {
        showErrorDialog(err as { code: string; message: string });
      } finally {
        busy.close();
      }
    } catch (err) {
      showErrorDialog(err as { code: string; message: string });
    }
  }

  function mirrorButton(entry: RepoEntry, mirror: Mirror): HTMLButtonElement {    const button = el("button", "btn btn--small", `→ ${mirror === "yandex" ? "Yandex" : "HyperDedic"}`);
    if (mirror === "hyperdedic" && isRpmFusion(entry.kind)) {
      button.disabled = true;
      button.title = "HyperDedic does not mirror RPM Fusion repositories";
    }
    button.addEventListener("click", () => void planSwitch(entry, { type: "switch", mirror }));
    return button;
  }

  function ciscoButton(entry: RepoEntry, mode: "disable" | "codecs"): HTMLButtonElement {
    const button = el("button", "btn btn--small", mode === "disable" ? "Disable" : "Use codecs mirror");
    button.addEventListener("click", () => void planSwitch(entry, { type: "cisco", mode }));
    return button;
  }

  async function planSwitch(entry: RepoEntry, action: Parameters<typeof api.previewAction>[1]): Promise<void> {
    // A modal is up: this click is a leftover double-click, not a new action.
    if (anyDialogOpen()) {
      return;
    }
    try {
      const outcome = await api.previewAction(entry.id, action);
      if (outcome.outcome === "no-change") {
        showNoChange(outcome.reason);
        return;
      }
      const decision = await confirmPlan(
        "Switch repository",
        `${entry.id} (${entry.file})`,
        outcome.preview,
        true,
      );
      if (!decision.confirmed) {
        return;
      }
      const busy = showBusy("Waiting for authorization and applying changes…");
      try {
        const report = await api.applyPlan(outcome.plan, decision.refreshCache);
        showReport(report);
        notifyApplied();
      } catch (err) {
        showErrorDialog(err as { code: string; message: string });
      } finally {
        busy.close();
      }
    } catch (err) {
      showErrorDialog(err as { code: string; message: string });
    }
  }
}

function renderError(target: HTMLElement, err: { code: string; message: string }): void {
  target.replaceChildren();
  target.append(el("p", "panel__error", `Repositories cannot be listed: ${err.message}`));
  if (err.code === "not_found") {
    target.append(el("p", "panel__hint", "Make sure the repository directories named in the error exist (this tool manages Fedora dnf repositories)."));
  }
}
