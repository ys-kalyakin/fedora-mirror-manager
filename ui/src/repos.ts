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

/** Data columns the inventory table can be sorted by. The Action column is
    deliberately absent: it holds buttons, not data. */
type SortColumn = "repository" | "file" | "state" | "source" | "url";

type SortDir = "asc" | "desc";

type SortState = { column: SortColumn; dir: SortDir };

/** Header columns in display order; `sort: null` marks Action, which renders
    as a plain, non-interactive header cell. */
const COLUMNS: { sort: SortColumn | null; label: string }[] = [
  { sort: "repository", label: "Repository" },
  { sort: "file", label: "File" },
  { sort: "state", label: "State" },
  { sort: "source", label: "Source" },
  { sort: "url", label: "URL" },
  { sort: null, label: "Action" },
];

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

  // The inventory and its table element, kept so a header click can rebuild
  // just the table: banner, warnings and the layout hint are sort-independent,
  // and the banner comes from an async helper call that must not be re-run.
  let inventory: Inventory | null = null;
  let table: HTMLTableElement | null = null;
  // The user's column sort; null = unsorted (backend file order). Kept outside
  // render()/load() so Refresh and fmm:applied re-renders reuse the choice.
  let sort: SortState | null = null;

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
    inventory = inv;
    table = buildTable(inv);
    target.append(table);
  }

  // Rebuilds the table in place with the current sort (header clicks land here).
  function rebuildTable(): void {
    if (inventory === null || table === null) {
      return;
    }
    const next = buildTable(inventory);
    table.replaceWith(next);
    table = next;
  }

  function buildTable(inv: Inventory): HTMLTableElement {
    const table = el("table", "repo-table", "");
    const thead = el("thead", "", "");
    thead.append(buildHeadRow());
    const tbody = el("tbody", "", "");
    const entries = sort === null ? inv.entries : sortedEntries(inv.entries, sort.column, sort.dir);
    for (const entry of entries) {
      tbody.append(row(entry, inv.layout === "f45-plus"));
    }
    table.append(thead, tbody);
    return table;
  }

  // Every data column is a button (first click sorts ascending, a second click
  // on the same header reverses); Action stays a plain header cell.
  function buildHeadRow(): HTMLTableRowElement {
    const headRow = el("tr", "", "");
    for (const column of COLUMNS) {
      const key = column.sort;
      if (key === null) {
        headRow.append(el("th", "", column.label));
        continue;
      }
      const th = el("th", "", "");
      const active = sort !== null && sort.column === key ? sort.dir : null;
      if (active !== null) {
        th.classList.add(active === "asc" ? "repo-table__th--asc" : "repo-table__th--desc");
        th.setAttribute("aria-sort", active === "asc" ? "ascending" : "descending");
      }
      const button = el("button", "repo-table__sort", column.label);
      button.type = "button";
      button.addEventListener("click", () => {
        if (sort !== null && sort.column === key) {
          sort.dir = sort.dir === "asc" ? "desc" : "asc";
        } else {
          sort = { column: key, dir: "asc" };
        }
        rebuildTable();
      });
      th.append(button);
      headRow.append(th);
    }
    return headRow;
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

/** Orders entries by column and direction. Text columns compare
    case-insensitively, State ascending puts disabled first (false < true), and
    an entry without a URL sorts after any URL in both directions. Array#sort
    is stable, so entries with equal keys keep the delivered (file) order. The
    input array is copied, never mutated: the unsorted copy of the inventory
    stays the "default order" state. */
function sortedEntries(entries: RepoEntry[], column: SortColumn, dir: SortDir): RepoEntry[] {
  return [...entries].sort((a, b) => {
    // Applied before the direction flip, so a missing URL is last either way.
    if (column === "url" && (a.url === null || b.url === null)) {
      return a.url === b.url ? 0 : a.url === null ? 1 : -1;
    }
    const cmp = compareBy(a, b, column);
    return dir === "asc" ? cmp : -cmp;
  });
}

/** Ascending comparison of two entries by column; the caller has already
    ordered missing URLs last, so both URLs exist in the url case. */
function compareBy(a: RepoEntry, b: RepoEntry, column: SortColumn): number {
  switch (column) {
    case "repository":
      return a.id.localeCompare(b.id, undefined, { sensitivity: "base" });
    case "file":
      return a.file.localeCompare(b.file, undefined, { sensitivity: "base" });
    case "state":
      return Number(a.enabled) - Number(b.enabled);
    case "source":
      return SOURCE_LABELS[a.source].localeCompare(SOURCE_LABELS[b.source], undefined, { sensitivity: "base" });
    case "url":
      return (a.url ?? "").localeCompare(b.url ?? "", undefined, { sensitivity: "base" });
  }
}

function renderError(target: HTMLElement, err: { code: string; message: string }): void {
  target.replaceChildren();
  target.append(el("p", "panel__error", `Repositories cannot be listed: ${err.message}`));
  if (err.code === "not_found") {
    target.append(el("p", "panel__hint", "Make sure the repository directories named in the error exist (this tool manages Fedora dnf repositories)."));
  }
}
