import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// Types mirroring the fmm-core serde models (snake_case fields on the wire).
// ---------------------------------------------------------------------------

export type Source = "yandex" | "hyperdedic" | "metalink" | "other" | "none";

export type Switchability = "switchable" | "cisco" | "not-switchable";

export type Layout = "legacy" | "f45-plus";

export type Origin = "packaged" | "local";

export interface RepoEntry {
  id: string;
  name: string;
  file: string;
  path: string;
  origin: Origin;
  enabled: boolean;
  source: Source;
  url: string | null;
  kind: string;
  switchability: Switchability;
}

export interface FileError {
  file: string;
  message: string;
}

export interface Inventory {
  layout: Layout;
  entries: RepoEntry[];
  errors: FileError[];
}

export interface SnapshotInfo {
  id: string;
  timestamp: string;
  tag: string;
}

export interface PlanPreview {
  file: string;
  target_path: string;
  new_baseurl: string | null;
  set_enabled: string | null;
  removed_lines: string[];
  /** The target file does not exist yet and will be created by the helper. */
  creates_file: boolean;
  /** Packaged .repo file the new content derives from, if any. */
  source_path: string | null;
}

export interface PlanFile {
  path: string;
  before_sha256: string;
  content: string;
}

export interface ApplyPlan {
  plan_version: number;
  action: "switch" | "restore";
  /** User snapshot root; the backend fills it before the helper runs. */
  snapshot_root?: string;
  snapshot_tag: string;
  refresh_cache: boolean;
  purge?: boolean;
  snapshot?: string | null;
  files: PlanFile[];
}

export interface CacheReport {
  ran: boolean;
  ok: boolean;
  output_tail: string;
}

export interface ApplyReport {
  ok: boolean;
  code: string | null;
  message: string | null;
  snapshot: string | null;
  changed: string[];
  unchanged: string[];
  extra_files: string[];
  cache: CacheReport | null;
}

export type Mirror = "yandex" | "hyperdedic";

export type CiscoMode = "disable" | "codecs" | "keep";

export type RepoAction = { type: "switch"; mirror: Mirror } | { type: "cisco"; mode: CiscoMode };

export type PlanOutcome =
  | { outcome: "no-change"; reason: string }
  | { outcome: "plan"; preview: PlanPreview; plan: ApplyPlan };

/** One rewritten section inside a bulk file preview. */
export interface BulkSectionPreview {
  repo_id: string;
  new_baseurl: string;
  removed_lines: string[];
}

/** One file the bulk plan writes (or creates), for the preview dialog. */
export interface BulkFilePreview {
  file: string;
  target_path: string;
  /** The target file does not exist yet and will be created by the helper. */
  creates_file: boolean;
  /** Packaged .repo file the new content derives from, if any. */
  source_path: string | null;
  sections: BulkSectionPreview[];
}

/** A repository excluded from the bulk switch, with the reason. */
export interface BulkSkip {
  repo_id: string;
  reason: string;
}

/** Combined preview of a bulk switch. */
export interface BulkPreview {
  mirror: Mirror;
  files: BulkFilePreview[];
  skipped: BulkSkip[];
}

export type BulkOutcome =
  | { outcome: "no-change"; reason: string; skipped: BulkSkip[] }
  | { outcome: "plan"; preview: BulkPreview; plan: ApplyPlan };

/** Error shape every backend failure is normalized to. */
export interface CmdErrorPayload {
  code: string;
  message: string;
}

function toCmdError(err: unknown): CmdErrorPayload {
  if (err && typeof err === "object" && "code" in err && "message" in err) {
    return err as CmdErrorPayload;
  }
  return { code: "unknown", message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw toCmdError(err);
  }
}

// ---------------------------------------------------------------------------
// Backend commands.
// ---------------------------------------------------------------------------

export interface HelperStatus {
  installed: boolean;
  helper_path: string;
  policy_installed: boolean;
  /** Plan-protocol version the installed helper reports (null: unknown). */
  plan_version: number | null;
  /** Installed but older than the application's protocol version. */
  outdated: boolean;
}

export interface InstallReport {
  installed: string[];
}

/** Result of the one-click legacy-snapshot migration. */
export interface MigrateReport {
  ok: boolean;
  moved: string[];
  failed: string[];
}

export const api = {
  listRepos: () => call<Inventory>("list_repos"),
  listSnapshots: () => call<SnapshotInfo[]>("list_snapshots"),
  deleteSnapshot: (snapshotId: string) => call<void>("delete_snapshot", { snapshotId }),
  legacySnapshots: () => call<SnapshotInfo[]>("legacy_snapshots"),
  migrateLegacySnapshots: () => call<MigrateReport>("migrate_legacy_snapshots"),
  previewAction: (repoId: string, action: RepoAction) =>
    call<PlanOutcome>("preview_action", { repoId, action }),
  previewBulkSwitch: (mirror: Mirror) =>
    call<BulkOutcome>("preview_bulk_switch", { mirror }),
  previewRestore: (snapshotId: string) => call<string[]>("preview_restore", { snapshotId }),
  applyPlan: (plan: ApplyPlan, refreshCache: boolean) =>
    call<ApplyReport>("apply_plan", { plan, refreshCache }),
  applyRestore: (snapshotId: string, purge: boolean, refreshCache: boolean) =>
    call<ApplyReport>("apply_restore", { snapshotId, purge, refreshCache }),
  helperStatus: () => call<HelperStatus>("helper_status"),
  installHelper: () => call<InstallReport>("install_helper"),
};

/** True for RPM Fusion repository families (HyperDedic does not mirror them). */
export function isRpmFusion(kind: string): boolean {
  return kind.startsWith("rf-");
}

/** Fires after any successful privileged apply so panels can refresh. */
export function notifyApplied(): void {
  window.dispatchEvent(new CustomEvent("fmm:applied"));
}
