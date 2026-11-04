import type { CmdErrorPayload, SnapshotInfo } from "./api";
import { api, notifyApplied } from "./api";
import {
  anyDialogOpen,
  confirmDelete,
  confirmRestore,
  helperBanner,
  showBusy,
  showErrorDialog,
  showInfoDialog,
  showReport,
} from "./apply";
import { el } from "./dom";

/** The snapshots panel: list of snapshots with restore and delete actions. */
export function initSnapshots(container: HTMLElement): void {
  const header = el("div", "panel__header", "");
  header.append(el("h2", "panel__title", "Snapshots"));
  const refresh = el("button", "btn", "Refresh");
  refresh.addEventListener("click", () => void load());
  header.append(refresh);

  const body = el("div", "panel__body", "");
  container.append(header, body);

  async function load(): Promise<void> {
    body.replaceChildren(el("p", "panel__loading", "Loading snapshots…"));
    try {
      const [snaps, banners] = await Promise.all([api.listSnapshots(), bannersFor()]);
      render(body, banners, snaps);
    } catch (err) {
      body.replaceChildren(
        el("p", "panel__error", `Snapshots cannot be listed: ${(err as { message: string }).message}`),
      );
    }
  }

  window.addEventListener("fmm:applied", () => void load());
  void load();

  /** Banners above the list: the helper setup offer, then the legacy
   * snapshot migration offer (both null when nothing applies). */
  async function bannersFor(): Promise<HTMLElement[]> {
    const banners = await Promise.all([helperBanner(), legacyBanner()]);
    return banners.filter((banner): banner is HTMLElement => banner !== null);
  }

  /** One-click migration offer while snapshots still live in the legacy
   * root `/etc/yum.repos.d.bak` (null when there is nothing to move). */
  async function legacyBanner(): Promise<HTMLElement | null> {
    let legacy: SnapshotInfo[];
    try {
      legacy = await api.legacySnapshots();
    } catch {
      return null; // informational only; never block the panel
    }
    if (legacy.length === 0) {
      return null;
    }
    const banner = el("div", "helper-banner", "");
    const text = el("div", "helper-banner__text", "");
    text.append(
      el("strong", "", `${legacy.length} snapshot(s) still live in /etc/yum.repos.d.bak. `),
    );
    text.append(
      el(
        "span",
        "helper-banner__hint",
        "Snapshots are kept in your user directory now. Moving them asks for authorization once.",
      ),
    );
    const move = el("button", "btn btn--primary", "Move snapshots");
    move.addEventListener("click", () => void migrateFlow());
    banner.append(text, move);
    return banner;
  }

  function render(target: HTMLElement, banners: HTMLElement[], snaps: SnapshotInfo[]): void {
    target.replaceChildren();
    for (const banner of banners) {
      target.append(banner);
    }
    if (snaps.length === 0) {
      target.append(el("p", "panel__empty", "No snapshots yet. One is created automatically before every change."));
      return;
    }
    const list = el("ul", "snapshot-list", "");
    for (const snap of snaps) {
      const item = el("li", "snapshot-list__item", "");
      const info = el("div", "snapshot-list__info", "");
      info.append(el("span", "snapshot-list__timestamp", formatTimestamp(snap.timestamp)));
      info.append(el("span", "snapshot-list__tag", snap.tag || "—"));
      const actions = el("div", "snapshot-list__actions", "");
      const restore = el("button", "btn btn--small", "Restore");
      restore.addEventListener("click", () => void restoreFlow(snap));
      const remove = el("button", "btn btn--small btn--danger", "Delete");
      remove.addEventListener("click", () => void deleteFlow(snap));
      actions.append(restore, remove);
      item.append(info, actions);
      list.append(item);
    }
    target.append(list);
  }

  async function restoreFlow(snap: SnapshotInfo): Promise<void> {
    let extraFiles: string[] = [];
    try {
      extraFiles = await api.previewRestore(snap.id);
    } catch (err) {
      showErrorDialog(err as CmdErrorPayload);
      return;
    }
    const decision = await confirmRestore(snap.id, extraFiles, true);
    if (!decision.confirmed) {
      return;
    }
    const busy = showBusy("Waiting for authorization and restoring…");
    try {
      const report = await api.applyRestore(snap.id, decision.purge, decision.refreshCache);
      showReport(report);
      notifyApplied();
    } catch (err) {
      showErrorDialog(err as CmdErrorPayload);
    } finally {
      busy.close();
    }
  }

  /** Deletes one snapshot after an explicit confirmation. Unprivileged: the
   * snapshots live in the user's own directory. The list refreshes even on
   * error, so an already-missing snapshot disappears from the stale list. */
  async function deleteFlow(snap: SnapshotInfo): Promise<void> {
    if (anyDialogOpen()) {
      return;
    }
    const title = `Snapshot ${formatTimestamp(snap.timestamp)}${snap.tag ? ` (${snap.tag})` : ""} will be deleted.`;
    const confirmed = await confirmDelete(title);
    if (!confirmed) {
      return;
    }
    try {
      await api.deleteSnapshot(snap.id);
    } catch (err) {
      showErrorDialog(err as CmdErrorPayload);
    }
    await load();
  }

  /** Moves the legacy snapshots into the user root under one polkit prompt. */
  async function migrateFlow(): Promise<void> {
    if (anyDialogOpen()) {
      return;
    }
    const busy = showBusy("Waiting for authorization and moving snapshots…");
    try {
      const report = await api.migrateLegacySnapshots();
      if (report.ok) {
        showInfoDialog("Snapshots moved", `${report.moved.length} snapshot(s) moved into the user directory.`);
      } else {
        showInfoDialog(
          "Migration finished with errors",
          `${report.moved.length} snapshot(s) moved, ${report.failed.length} failed: ${report.failed.join(", ")}`,
        );
      }
    } catch (err) {
      showErrorDialog(err as CmdErrorPayload);
    } finally {
      busy.close();
    }
    await load();
  }
}

/** `20261104T120000Z` → `2026-11-04 12:00:00 UTC`. */
function formatTimestamp(ts: string): string {
  const m = /^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z$/.exec(ts);
  if (!m) {
    return ts;
  }
  const [, y, mo, d, h, mi, s] = m;
  return `${y}-${mo}-${d} ${h}:${mi}:${s} UTC`;
}
