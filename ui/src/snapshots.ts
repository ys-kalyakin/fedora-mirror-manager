import type { SnapshotInfo } from "./api";
import { api, notifyApplied } from "./api";
import { confirmRestore, helperBanner, showErrorDialog, showBusy, showReport } from "./apply";
import { el } from "./dom";

/** The snapshots panel: list of snapshots with restore actions. */
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
      const [snaps, banner] = await Promise.all([api.listSnapshots(), helperBanner()]);
      render(body, snaps, banner);
    } catch (err) {
      body.replaceChildren(
        el("p", "panel__error", `Snapshots cannot be listed: ${(err as { message: string }).message}`),
      );
    }
  }

  window.addEventListener("fmm:applied", () => void load());
  void load();

  function render(target: HTMLElement, snaps: SnapshotInfo[], banner: HTMLElement | null): void {
    target.replaceChildren();
    if (banner) {
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
      const restore = el("button", "btn btn--small", "Restore");
      restore.addEventListener("click", () => void restoreFlow(snap));
      item.append(info, restore);
      list.append(item);
    }
    target.append(list);
  }

  async function restoreFlow(snap: SnapshotInfo): Promise<void> {
    let extraFiles: string[] = [];
    try {
      extraFiles = await api.previewRestore(snap.id);
    } catch (err) {
      showErrorDialog(err as { code: string; message: string });
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
      showErrorDialog(err as { code: string; message: string });
    } finally {
      busy.close();
    }
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
