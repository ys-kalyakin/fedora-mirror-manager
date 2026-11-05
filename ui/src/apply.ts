import type { ApplyReport, BulkFilePreview, BulkPreview, CmdErrorPayload, HelperStatus, Mirror, PlanPreview } from "./api";
import { api, notifyApplied } from "./api";
import { el } from "./dom";

/**
 * Shared dialogs for the privileged apply flow: confirmation with preview,
 * the applying state, the result report, and code-aware error reporting.
 *
 * The dialogs are plain fixed divs, not the native `<dialog>` element: on
 * Linux WebKitGTK (the Tauri webview) a showModal() dialog renders but never
 * receives mouse input, leaving the whole window blocked behind a modal that
 * cannot be dismissed. A fixed overlay behaves identically on every backend.
 */

/** A div-based modal overlay. `onDismiss` runs when the user presses Escape. */
function openModal(onDismiss?: () => void): { root: HTMLElement; close: () => void } {
  const root = el("div", "fmm-backdrop", "");
  const onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      modal.close();
      onDismiss?.();
    }
  };
  const modal = {
    root,
    close: () => {
      document.removeEventListener("keydown", onKey, true);
      root.remove();
    },
  };
  document.addEventListener("keydown", onKey, true);
  document.body.append(root);
  return modal;
}

/** Closes any open modal: at most one must ever be shown, so a double-click
 * can never stack identical dialogs into an inescapable pile. */
function closeOpenModals(): void {
  document.querySelectorAll(".fmm-backdrop").forEach((backdrop) => backdrop.remove());
}

/** True while any modal dialog is open (used to swallow extra clicks). */
export function anyDialogOpen(): boolean {
  return document.querySelector(".fmm-backdrop") !== null;
}

/** Build the confirmation dialog content for a switch preview. */
function previewRows(preview: PlanPreview): HTMLElement[] {
  const rows: HTMLElement[] = [];
  // F45+ overrides need the create/rewrite distinction spelled out, otherwise
  // the /etc target of a not-yet-existing file reads as a stale pre-F45 path.
  if (preview.creates_file) {
    rows.push(el("p", "dialog__row", `New file: ${preview.target_path}`));
    if (preview.source_path) {
      rows.push(el("p", "dialog__row dialog__hint", `Will be created — overrides ${preview.source_path}`));
    }
  } else {
    rows.push(el("p", "dialog__row", `File: ${preview.target_path}`));
    if (preview.source_path) {
      rows.push(el("p", "dialog__row dialog__hint", `Rewritten in place — content rebuilt from ${preview.source_path}`));
    }
  }
  if (preview.new_baseurl) {
    const row = el("p", "dialog__row", "New baseurl: ");
    row.append(el("code", "dialog__code", preview.new_baseurl));
    rows.push(row);
  }
  if (preview.set_enabled) {
    rows.push(el("p", "dialog__row", `Set enabled=${preview.set_enabled}`));
  }
  if (preview.removed_lines.length > 0) {
    const removed = el("details", "dialog__removed", "");
    removed.append(el("summary", "", `Replace ${preview.removed_lines.length} source line(s)`));
    const list = el("ul", "dialog__removed-list", "");
    for (const line of preview.removed_lines) {
      list.append(el("li", "dialog__code", line));
    }
    removed.append(list);
    rows.push(removed);
  }
  return rows;
}

/** Confirmation result: what the user decided in the dialog. */
export type Confirmation = { confirmed: boolean; refreshCache: boolean };

/** Decision returned by the restore dialog. */
export type RestoreDecision = Confirmation & { purge: boolean };

/**
 * Shows the confirmation dialog for a planned switch.
 * Resolves when the user closes the dialog.
 */
export function confirmPlan(
  title: string,
  intro: string,
  preview: PlanPreview,
  defaultRefresh: boolean,
): Promise<Confirmation> {
  return new Promise((resolve) => {
    closeOpenModals();
    const modal = openModal();
    const finish = (confirmed: boolean) => {
      modal.close();
      resolve({ confirmed, refreshCache });
    };
    const dialog = el("div", "fmm-dialog", "");
    dialog.append(el("h3", "dialog__title", title));
    dialog.append(el("p", "dialog__intro", intro));
    for (const row of previewRows(preview)) {
      dialog.append(row);
    }

    let refreshCache = defaultRefresh;
    const refreshBox = el("input", "dialog__checkbox", "") as HTMLInputElement;
    refreshBox.type = "checkbox";
    refreshBox.checked = defaultRefresh;
    refreshBox.addEventListener("change", () => {
      refreshCache = refreshBox.checked;
    });
    const refreshLabel = el("label", "dialog__option", "");
    refreshLabel.append(refreshBox, document.createTextNode("Refresh package cache (dnf clean all && dnf makecache)"));
    dialog.append(refreshLabel);

    const actions = el("div", "dialog__actions", "");
    const cancel = el("button", "btn", "Cancel");
    const apply = el("button", "btn btn--primary", "Apply…");
    cancel.addEventListener("click", () => finish(false));
    apply.addEventListener("click", () => finish(true));
    actions.append(cancel, apply);
    dialog.append(actions);
    modal.root.append(dialog);
  });
}

/** Display names for the mirrors (the wire values are kebab-case). */
const MIRROR_LABELS: Record<Mirror, string> = { yandex: "Yandex", hyperdedic: "HyperDedic" };

/** One file group of the bulk confirmation dialog: target marking + sections. */
function bulkFileGroup(file: BulkFilePreview): HTMLElement {
  const group = el("div", "dialog__file-group", "");
  if (file.creates_file) {
    group.append(el("p", "dialog__row", `New file: ${file.target_path}`));
    if (file.source_path) {
      group.append(el("p", "dialog__row dialog__hint", `Will be created — overrides ${file.source_path}`));
    }
  } else {
    group.append(el("p", "dialog__row", `File: ${file.target_path}`));
    if (file.source_path) {
      group.append(el("p", "dialog__row dialog__hint", `Rewritten in place — content rebuilt from ${file.source_path}`));
    }
  }
  const list = el("ul", "dialog__file-sections", "");
  for (const section of file.sections) {
    const item = el("li", "dialog__file-section", "");
    const urlRow = el("p", "dialog__row", "");
    urlRow.append(el("strong", "", section.repo_id), document.createTextNode(" → "));
    urlRow.append(el("code", "dialog__code", section.new_baseurl));
    item.append(urlRow);
    if (section.removed_lines.length > 0) {
      const removed = el("details", "dialog__removed", "");
      removed.append(el("summary", "", `Replace ${section.removed_lines.length} source line(s)`));
      const removedList = el("ul", "dialog__removed-list", "");
      for (const line of section.removed_lines) {
        removedList.append(el("li", "dialog__code", line));
      }
      removed.append(removedList);
      item.append(removed);
    }
    list.append(item);
  }
  group.append(list);
  return group;
}

/**
 * Confirmation dialog for a bulk switch: every affected file with its
 * sections and new baseurls, the skipped repositories with reasons, and the
 * cache-refresh option. Resolves when the user closes the dialog.
 */
export function confirmBulkPlan(
  mirror: Mirror,
  preview: BulkPreview,
  defaultRefresh: boolean,
): Promise<Confirmation> {
  return new Promise((resolve) => {
    closeOpenModals();
    const modal = openModal();
    const finish = (confirmed: boolean) => {
      modal.close();
      resolve({ confirmed, refreshCache });
    };
    const label = MIRROR_LABELS[mirror];
    const repoCount = preview.files.reduce((count, file) => count + file.sections.length, 0);
    const dialog = el("div", "fmm-dialog", "");
    dialog.append(el("h3", "dialog__title", `Switch all to ${label}`));
    dialog.append(
      el(
        "p",
        "dialog__intro",
        `${repoCount} ${repoCount === 1 ? "repository" : "repositories"} in ${preview.files.length} ${preview.files.length === 1 ? "file" : "files"} will be switched to ${label}.`,
      ),
    );

    const fileList = el("div", "dialog__bulk-files", "");
    for (const file of preview.files) {
      fileList.append(bulkFileGroup(file));
    }
    dialog.append(fileList);

    if (preview.skipped.length > 0) {
      const skipped = el("details", "dialog__removed", "");
      skipped.append(el("summary", "", `Skipped repositories (${preview.skipped.length})`));
      const list = el("ul", "dialog__removed-list", "");
      for (const skip of preview.skipped) {
        list.append(el("li", "dialog__code", `${skip.repo_id}: ${skip.reason}`));
      }
      skipped.append(list);
      dialog.append(skipped);
    }

    let refreshCache = defaultRefresh;
    const refreshBox = el("input", "dialog__checkbox", "") as HTMLInputElement;
    refreshBox.type = "checkbox";
    refreshBox.checked = defaultRefresh;
    refreshBox.addEventListener("change", () => {
      refreshCache = refreshBox.checked;
    });
    const refreshLabel = el("label", "dialog__option", "");
    refreshLabel.append(refreshBox, document.createTextNode("Refresh package cache (dnf clean all && dnf makecache)"));
    dialog.append(refreshLabel);

    const actions = el("div", "dialog__actions", "");
    const cancel = el("button", "btn", "Cancel");
    const apply = el("button", "btn btn--primary", "Apply…");
    cancel.addEventListener("click", () => finish(false));
    apply.addEventListener("click", () => finish(true));
    actions.append(cancel, apply);
    dialog.append(actions);
    modal.root.append(dialog);
  });
}

/** Opens the restore confirmation dialog with purge/refresh options. */export function confirmRestore(
  snapshotId: string,
  extraFiles: string[],
  defaultRefresh: boolean,
): Promise<RestoreDecision> {
  return new Promise((resolve) => {
    closeOpenModals();
    const modal = openModal();
    const finish = (confirmed: boolean) => {
      modal.close();
      resolve({ confirmed, purge: confirmed ? purge : false, refreshCache: confirmed ? refreshCache : defaultRefresh });
    };
    const dialog = el("div", "fmm-dialog", "");
    dialog.append(el("h3", "dialog__title", "Restore repositories from snapshot"));
    dialog.append(el("p", "dialog__intro", `Snapshot: ${snapshotId}`));
    dialog.append(el("p", "dialog__row", "Every *.repo file will be replaced with its snapshot content."));

    if (extraFiles.length > 0) {
      const note = el("p", "dialog__row dialog__warning", `${extraFiles.length} file(s) exist now but are not in the snapshot:`);
      const list = el("ul", "dialog__removed-list", "");
      for (const name of extraFiles) {
        list.append(el("li", "dialog__code", name));
      }
      note.append(list);
      dialog.append(note);
    }

    let purge = false;
    const purgeBox = el("input", "dialog__checkbox", "") as HTMLInputElement;
    purgeBox.type = "checkbox";
    purgeBox.disabled = extraFiles.length === 0;
    purgeBox.addEventListener("change", () => {
      purge = purgeBox.checked;
    });
    const purgeLabel = el("label", "dialog__option", "");
    purgeLabel.append(purgeBox, document.createTextNode("Delete files added after the snapshot"));
    dialog.append(purgeLabel);

    let refreshCache = defaultRefresh;
    const refreshBox = el("input", "dialog__checkbox", "") as HTMLInputElement;
    refreshBox.type = "checkbox";
    refreshBox.checked = defaultRefresh;
    refreshBox.addEventListener("change", () => {
      refreshCache = refreshBox.checked;
    });
    const refreshLabel = el("label", "dialog__option", "");
    refreshLabel.append(refreshBox, document.createTextNode("Refresh package cache (dnf clean all && dnf makecache)"));
    dialog.append(refreshLabel);

    const actions = el("div", "dialog__actions", "");
    const cancel = el("button", "btn", "Cancel");
    const apply = el("button", "btn btn--primary", "Restore…");
    cancel.addEventListener("click", () => finish(false));
    apply.addEventListener("click", () => finish(true));
    actions.append(cancel, apply);
    dialog.append(actions);
    modal.root.append(dialog);
  });
}

/** Modal busy indicator while pkexec/authorization is in progress. */
export function showBusy(text: string): { close: () => void } {
  closeOpenModals();
  const modal = openModal();
  const dialog = el("div", "fmm-dialog fmm-dialog--busy", "");
  dialog.append(el("p", "dialog__intro", text));
  modal.root.append(dialog);
  return { close: modal.close };
}

/** Confirmation for the unprivileged delete of one snapshot. `title` carries
 * the human-readable snapshot description; Escape or Cancel resolves false. */
export function confirmDelete(title: string): Promise<boolean> {
  return new Promise((resolve) => {
    closeOpenModals();
    const modal = openModal(() => resolve(false));
    const finish = (confirmed: boolean) => {
      modal.close();
      resolve(confirmed);
    };
    const dialog = el("div", "fmm-dialog", "");
    dialog.append(el("h3", "dialog__title", "Delete snapshot?"));
    dialog.append(el("p", "dialog__intro", title));
    dialog.append(
      el("p", "dialog__row dialog__warning", "The snapshot directory is removed permanently. This cannot be undone."),
    );
    const actions = el("div", "dialog__actions", "");
    const cancel = el("button", "btn", "Cancel");
    const remove = el("button", "btn btn--danger", "Delete");
    cancel.addEventListener("click", () => finish(false));
    remove.addEventListener("click", () => finish(true));
    actions.append(cancel, remove);
    dialog.append(actions);
    modal.root.append(dialog);
  });
}

/** Code-aware error text for the known backend failure modes. */
export function errorMessage(err: CmdErrorPayload): string {
  switch (err.code) {
    case "denied":
      return "Not authorized — the request was dismissed or denied. Nothing was changed.";
    case "install_failed":
      // The user authorized; show the real privileged-step failure.
      return err.message || "The privileged install step failed.";
    case "stale_plan":
      return "The repositories changed on disk since the preview. Refresh the list and try again.";
    case "helper_missing":
      return "The privileged helper is not installed. See the README section “Privileged helper installation” (fmm-apply + polkit policy).";
    case "not_switchable":
      return err.message;
    default:
      return err.message || "Unexpected error.";
  }
}

/** Reports a failure with the code-aware message. */
export function showErrorDialog(err: CmdErrorPayload): void {
  closeOpenModals();
  const modal = openModal();
  const dialog = el("div", "fmm-dialog", "");
  dialog.append(el("h3", "dialog__title", "Action failed"));
  dialog.append(el("p", "dialog__intro dialog__error", errorMessage(err)));
  const actions = el("div", "dialog__actions", "");
  const close = el("button", "btn", "Close");
  close.addEventListener("click", () => modal.close());
  actions.append(close);
  // The one missing piece can be fixed right here: offer the in-app setup.
  if (err.code === "helper_missing") {
    const install = el("button", "btn btn--primary", "Install helper");
    install.addEventListener("click", () => {
      modal.close();
      void runSetupFlow();
    });
    actions.prepend(install);
  }
  dialog.append(actions);
  modal.root.append(dialog);
}

/** Neutral information dialog (e.g. successful setup). */
export function showInfoDialog(title: string, message: string): void {
  closeOpenModals();
  const modal = openModal();
  const dialog = el("div", "fmm-dialog", "");
  dialog.append(el("h3", "dialog__title", title));
  dialog.append(el("p", "dialog__intro", message));
  const actions = el("div", "dialog__actions", "");
  const close = el("button", "btn btn--primary", "Close");
  close.addEventListener("click", () => modal.close());
  actions.append(close);
  dialog.append(actions);
  modal.root.append(dialog);
}

/**
 * In-app privileged helper setup: installs the helper binary and the polkit
 * policy under a single pkexec authorization. Resolves true when installed.
 */
export async function runSetupFlow(): Promise<boolean> {
  const busy = showBusy("Installing the privileged helper — one authorization prompt will appear…");
  try {
    const report = await api.installHelper();
    busy.close();
    showInfoDialog(
      "Setup complete",
      `Installed: ${report.installed.join(", ")}. Switching and restore are now available.`,
    );
    notifyApplied();
    return true;
  } catch (err) {
    busy.close();
    showErrorDialog(err as CmdErrorPayload);
    return false;
  }
}

/** Banner while the privileged helper is missing or outdated (null when
 * installed and current). Both variants lead into the same one-click setup. */
export async function helperBanner(): Promise<HTMLElement | null> {
  let status: HelperStatus;
  try {
    status = await api.helperStatus();
  } catch {
    return null; // status is informational only; never block the panel
  }
  if (status.installed && !status.outdated) {
    return null;
  }
  const banner = el("div", "helper-banner", "");
  const text = el("div", "helper-banner__text", "");
  if (status.outdated) {
    text.append(el("strong", "", "Privileged helper is outdated. "));
    text.append(
      el("span", "helper-banner__hint", "The installed helper speaks an older protocol than this application, so privileged actions fail until it is updated. Updating asks for authorization once."),
    );
  } else {
    text.append(el("strong", "", "Privileged helper is not installed. "));
    text.append(
      el("span", "helper-banner__hint", "Switching mirrors and restoring snapshots need it. Installing asks for authorization once."),
    );
  }
  const install = el("button", "btn btn--primary", status.outdated ? "Update helper" : "Install helper");
  install.addEventListener("click", () => void runSetupFlow());
  banner.append(text, install);
  return banner;
}

/** Shows the report of a successful apply (with optional cache warning). */
export function showReport(report: ApplyReport): void {
  closeOpenModals();
  const modal = openModal();
  const dialog = el("div", "fmm-dialog", "");
  dialog.append(el("h3", "dialog__title", "Done"));
  dialog.append(el("p", "dialog__row", `Snapshot saved: ${report.snapshot ?? "—"}`));
  dialog.append(el("p", "dialog__row", `Files changed: ${report.changed.length}`));
  if (report.unchanged.length > 0) {
    dialog.append(el("p", "dialog__row", `Files already up to date: ${report.unchanged.length}`));
  }
  if (report.extra_files.length > 0) {
    dialog.append(
      el("p", "dialog__row dialog__warning", `Kept files not present in the snapshot: ${report.extra_files.join(", ")}`),
    );
  }
  if (report.cache?.ran && !report.cache.ok) {
    const warn = el("details", "dialog__warning", "");
    warn.append(el("summary", "", "Package cache refresh failed (the changes themselves were applied)"));
    warn.append(el("pre", "dialog__code", report.cache.output_tail));
    dialog.append(warn);
  }
  const actions = el("div", "dialog__actions", "");
  const close = el("button", "btn btn--primary", "Close");
  close.addEventListener("click", () => modal.close());
  actions.append(close);
  dialog.append(actions);
  modal.root.append(dialog);
}

/** Informs about a no-change outcome. */
export function showNoChange(reason: string): void {
  closeOpenModals();
  const modal = openModal();
  const dialog = el("div", "fmm-dialog", "");
  dialog.append(el("h3", "dialog__title", "Nothing to do"));
  dialog.append(el("p", "dialog__intro", reason));
  const actions = el("div", "dialog__actions", "");
  const close = el("button", "btn", "Close");
  close.addEventListener("click", () => modal.close());
  actions.append(close);
  dialog.append(actions);
  modal.root.append(dialog);
}
