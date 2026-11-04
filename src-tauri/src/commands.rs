//! Application commands exposed to the frontend.
//!
//! Thin adapter: inventory/plans come from `fmm-core`, privileged work goes
//! through the `fmm-apply` helper invoked with `pkexec`.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use fmm_core::error::Error;
use fmm_core::plan::{ApplyPlan, ApplyReport, PlanOutcome, RepoAction};
use fmm_core::reposd::{
    DEFAULT_REPO_DIR, Inventory, LEGACY_SNAPSHOT_ROOT, RepoLocations, inventory,
};
use fmm_core::snapshot::{MigrateReport, SnapshotInfo};

/// Status reported by the backend to the shell.
#[derive(Debug, Serialize)]
pub struct AppStatus {
    /// Application version.
    pub version: &'static str,
}

/// Returns application status for the shell status area.
#[tauri::command]
pub fn app_status() -> AppStatus {
    AppStatus {
        version: fmm_core::version(),
    }
}

/// Application state managed by Tauri.
///
/// Repository locations are not stored: every command re-runs the layout
/// detection, so a layout change on disk (e.g. after an OS upgrade) is picked
/// up by the next inventory or preview without restarting the application.
pub struct AppState {
    /// User snapshot root (`$XDG_DATA_HOME/fmm/snapshots`), overridable for
    /// development and tests with `FMM_SNAPSHOT_ROOT`.
    pub snapshot_root: PathBuf,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            snapshot_root: snapshot_root_from_env(),
        }
    }
}

/// Resolves the snapshot root: `FMM_SNAPSHOT_ROOT`, else the user snapshot
/// root from the environment.
fn snapshot_root_from_env() -> PathBuf {
    match std::env::var("FMM_SNAPSHOT_ROOT") {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => fmm_core::reposd::user_snapshot_root(),
    }
}

/// Ensures the snapshot root exists before it is used: the unprivileged
/// application creates it as the user, so the helper's ownership hand-over
/// has a user-owned directory to write into.
pub fn ensure_snapshot_root(state: &AppState) -> Result<(), CmdError> {
    std::fs::create_dir_all(&state.snapshot_root).map_err(|e| CmdError {
        code: "io".to_string(),
        message: format!("cannot create {}: {e}", state.snapshot_root.display()),
    })
}

/// Serializable error for the frontend: a machine-readable `code` plus a
/// human-readable `message`. Codes the UI reacts to specifically: `denied`,
/// `stale_plan`, `helper_missing`.
#[derive(Debug, Serialize)]
pub struct CmdError {
    pub code: String,
    pub message: String,
}

impl From<Error> for CmdError {
    fn from(err: Error) -> Self {
        match err {
            Error::NotFound(message) => Self {
                code: "not_found".to_string(),
                message,
            },
            Error::Io { path, source } => Self {
                code: "io".to_string(),
                message: format!("{}: {source}", path.display()),
            },
            Error::Parse { file, reason } => Self {
                code: "parse".to_string(),
                message: format!("{}: {reason}", file.display()),
            },
            Error::NotSwitchable { id, reason } => Self {
                code: "not_switchable".to_string(),
                message: format!("{id}: {reason}"),
            },
            Error::StalePlan(message) => Self {
                code: "stale_plan".to_string(),
                message,
            },
            Error::Snapshot(message) => Self {
                code: "snapshot_failed".to_string(),
                message,
            },
            Error::Helper { code, message } => Self { code, message },
        }
    }
}

/// Reads every `*.repo` file in the repository locations of the detected
/// layout, merged with dnf5 precedence (local definitions win).
#[tauri::command]
pub fn list_repos(_state: tauri::State<AppState>) -> Result<Inventory, CmdError> {
    let locations = RepoLocations::detect();
    Ok(inventory(&locations)?)
}

/// Lists the snapshots in the snapshot root, newest first.
#[tauri::command]
pub fn list_snapshots(state: tauri::State<AppState>) -> Result<Vec<SnapshotInfo>, CmdError> {
    Ok(fmm_core::snapshot::list_snapshots(&state.snapshot_root)?)
}

/// Deletes one snapshot from the user snapshot root. Unprivileged by design:
/// snapshots live in the user's own data directory.
#[tauri::command]
pub fn delete_snapshot(state: tauri::State<AppState>, snapshot_id: String) -> Result<(), CmdError> {
    Ok(fmm_core::snapshot::delete_snapshot(
        &state.snapshot_root,
        &snapshot_id,
    )?)
}

/// Snapshots still living in the legacy root, for the migration notice. Empty
/// when the legacy root is missing or unreadable — the migration itself is
/// performed by the root helper, which sees the truth.
#[tauri::command]
pub fn legacy_snapshots() -> Vec<SnapshotInfo> {
    legacy_snapshots_with(std::env::var("FMM_LEGACY_SNAPSHOT_ROOT").ok().as_deref())
}

fn legacy_snapshots_with(env_value: Option<&str>) -> Vec<SnapshotInfo> {
    let root = match env_value {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(LEGACY_SNAPSHOT_ROOT),
    };
    fmm_core::snapshot::list_snapshots(&root).unwrap_or_default()
}

/// Moves the legacy snapshots into the user snapshot root with a single
/// polkit prompt (`fmm-apply migrate-snapshots`), the same one-prompt pattern
/// as the helper install.
#[tauri::command]
pub fn migrate_legacy_snapshots(state: tauri::State<AppState>) -> Result<MigrateReport, CmdError> {
    ensure_snapshot_root(&state)?;
    migrate_legacy_snapshots_with(
        "pkexec",
        &helper_path(),
        &state.snapshot_root,
        std::env::var("FMM_LEGACY_SNAPSHOT_ROOT")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .as_deref(),
    )
}

/// Runs `pkexec <helper> migrate-snapshots` and parses its JSON report. An
/// empty stdout means the authorization was dismissed; a non-empty but
/// unparsable stdout is a `bad_report`. A report with `ok: false` (some moves
/// failed) is returned as-is for the UI to show.
fn migrate_legacy_snapshots_with(
    pkexec: &str,
    helper: &Path,
    snapshot_root: &Path,
    legacy_root: Option<&Path>,
) -> Result<MigrateReport, CmdError> {
    if !helper.exists() {
        return Err(CmdError {
            code: "helper_missing".to_string(),
            message: format!(
                "privileged helper {} is not installed — see README (install fmm-apply and the polkit policy)",
                helper.display()
            ),
        });
    }
    let mut command = Command::new(pkexec);
    command.arg(helper).arg("migrate-snapshots");
    if let Some(legacy) = legacy_root {
        command.arg("--legacy-root").arg(legacy);
    }
    command.arg("--snapshot-root").arg(snapshot_root);
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|child| child.wait_with_output())
        .map_err(|e| CmdError {
            code: "denied".to_string(),
            message: format!("cannot run pkexec: {e}"),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        // pkexec produces no output when the authentication dialog is
        // dismissed or denied.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() {
            "not authorized: the authentication request was dismissed".to_string()
        } else {
            format!("not authorized: {}", stderr.trim())
        };
        return Err(CmdError {
            code: "denied".to_string(),
            message,
        });
    }
    let report: MigrateReport = serde_json::from_str(trimmed).map_err(|e| CmdError {
        code: "bad_report".to_string(),
        message: format!("the helper produced an unreadable report: {e}"),
    })?;
    Ok(report)
}

/// Builds the plan (or a no-change outcome) for one repository action,
/// targeting the file that effectively defines the section.
#[tauri::command]
pub fn preview_action(
    _state: tauri::State<AppState>,
    repo_id: String,
    action: RepoAction,
) -> Result<PlanOutcome, CmdError> {
    let locations = RepoLocations::detect();
    Ok(fmm_core::plan::build_action_plan(
        &locations, &repo_id, &action,
    )?)
}

/// Builds the bulk plan (or a no-change outcome) for switching every
/// repository the mirror carries to that mirror in one batch.
#[tauri::command]
pub fn preview_bulk_switch(
    _state: tauri::State<AppState>,
    mirror: fmm_core::catalog::Mirror,
) -> Result<fmm_core::bulk::BulkOutcome, CmdError> {
    let locations = RepoLocations::detect();
    Ok(fmm_core::bulk::build_bulk_plan(&locations, mirror)?)
}

/// `*.repo` files a restore would keep (or purge) because the snapshot
/// predates them.
#[tauri::command]
pub fn preview_restore(
    state: tauri::State<AppState>,
    snapshot_id: String,
) -> Result<Vec<String>, CmdError> {
    Ok(fmm_core::snapshot::restore_preview(
        Path::new(DEFAULT_REPO_DIR),
        &state.snapshot_root,
        &snapshot_id,
    )?)
}

/// Applies a switch plan produced by `preview_action`, asking polkit for
/// authorization. `refresh_cache` controls the post-write `dnf` refresh. The
/// plan's snapshot root is filled from the application state before the run.
#[tauri::command]
pub fn apply_plan(
    state: tauri::State<AppState>,
    plan: serde_json::Value,
    refresh_cache: bool,
) -> Result<ApplyReport, CmdError> {
    let mut plan: ApplyPlan = serde_json::from_value(plan).map_err(|e| CmdError {
        code: "bad_plan".to_string(),
        message: e.to_string(),
    })?;
    plan.refresh_cache = refresh_cache;
    ensure_snapshot_root(&state)?;
    let plan = prepare_plan(plan, state.snapshot_root.as_path())?;
    run_helper(&plan)
}

/// Fills the plan's snapshot root from the resolved state (UI plans arrive
/// without it) and refuses a plan that ends up without one — the helper
/// rejects such plans loudly, and so does the application.
fn prepare_plan(mut plan: ApplyPlan, snapshot_root: &Path) -> Result<ApplyPlan, CmdError> {
    if plan.snapshot_root.is_empty() {
        plan.snapshot_root = snapshot_root.to_string_lossy().into_owned();
    }
    if plan.snapshot_root.is_empty() {
        return Err(CmdError {
            code: "bad_plan".to_string(),
            message: "plan without a snapshot root".to_string(),
        });
    }
    Ok(plan)
}

/// Restores the repository directory from a snapshot. Restore always targets
/// the local directory (`/etc/yum.repos.d`) — on the F45+ layout the packaged
/// location is never modified.
#[tauri::command]
pub fn apply_restore(
    state: tauri::State<AppState>,
    snapshot_id: String,
    purge: bool,
    refresh_cache: bool,
) -> Result<ApplyReport, CmdError> {
    ensure_snapshot_root(&state)?;
    let mut plan = fmm_core::snapshot::build_restore_plan(
        Path::new(DEFAULT_REPO_DIR),
        &state.snapshot_root,
        &snapshot_id,
        purge,
    )?;
    plan.refresh_cache = refresh_cache;
    let plan = prepare_plan(plan, state.snapshot_root.as_path())?;
    run_helper(&plan)
}

/// Installed location of the privileged helper.
const INSTALLED_HELPER: &str = "/usr/libexec/fmm-apply";
/// Installed location of the polkit policy.
const POLICY_INSTALLED: &str = "/usr/share/polkit-1/actions/org.fmm.desktop.apply.policy";
/// Polkit policy embedded at build time (source: `packaging/polkit/`).
const POLICY_TEXT: &str = include_str!("../../packaging/polkit/org.fmm.desktop.apply.policy");

/// Availability of the privileged components, as shown by the setup banner.
#[derive(Debug, Serialize)]
pub struct HelperStatus {
    /// The helper binary is installed at the expected path.
    pub installed: bool,
    /// Path the app invokes the helper from.
    pub helper_path: String,
    /// The polkit policy is installed.
    pub policy_installed: bool,
    /// Plan-protocol version the installed helper reports (`None`: too old
    /// to answer, or the probe failed).
    pub plan_version: Option<u32>,
    /// Installed but older than the application's protocol version: the
    /// banner offers the one-click update.
    pub outdated: bool,
}

/// Reports whether the privileged helper and its policy are installed, and
/// whether the installed helper speaks the current plan protocol.
#[tauri::command]
pub fn helper_status() -> HelperStatus {
    helper_status_with(std::env::var("FMM_APPLY_HELPER").ok().as_deref())
}

fn helper_status_with(env_value: Option<&str>) -> HelperStatus {
    let helper = match env_value {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(INSTALLED_HELPER),
    };
    let installed = helper.is_file();
    let plan_version = if installed {
        helper_plan_version(&helper)
    } else {
        None
    };
    HelperStatus {
        installed,
        helper_path: helper.to_string_lossy().into_owned(),
        policy_installed: Path::new(POLICY_INSTALLED).exists(),
        outdated: installed && plan_version != Some(fmm_core::plan::PLAN_VERSION),
        plan_version,
    }
}

/// Asks the helper for its plan-protocol version: `fmm-apply version` prints
/// a bare integer and exits. Executed directly, without polkit — a version
/// probe grants nothing and touches nothing. A helper too old to know the
/// flag (or any exec failure) reports `None`; the banner treats that as
/// outdated. The mode never reads stdin, so the probe cannot hang on it.
///
/// Spawning is retried briefly: on a busy host (a parallel test suite
/// especially) a spawn can transiently fail with `EAGAIN`/`ETXTBSY`, and the
/// probe must not report a perfectly current helper as too old because of
/// that. A persistent failure still resolves to `None`.
fn helper_plan_version(helper: &Path) -> Option<u32> {
    let mut output = None;
    for delay_ms in [0u64, 10, 25] {
        if delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }
        match Command::new(helper)
            .arg("version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|child| child.wait_with_output())
        {
            Ok(o) => {
                output = Some(o);
                break;
            }
            Err(_) => continue,
        }
    }
    let output = output?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .ok()
}

/// Result of the in-app setup: what was installed.
#[derive(Debug, Serialize)]
pub struct InstallReport {
    /// Absolute paths installed by the privileged steps.
    pub installed: Vec<String>,
}

/// Installs the helper binary and the polkit policy from the app itself:
/// one `pkexec /bin/sh -c` invocation running root-owned `/usr/bin/install`
/// per file, covered by a single authorization prompt. Idempotent — safe to
/// run again after a rebuild.
#[tauri::command]
pub fn install_helper() -> Result<InstallReport, CmdError> {
    install_helper_with("pkexec")
}

fn install_helper_with(pkexec: &str) -> Result<InstallReport, CmdError> {
    install_with(pkexec, helper_source().as_deref())
}

/// Runs the install with a pre-resolved helper source (`None` maps to
/// `helper_missing`); split from `install_helper_with` so tests can drive the
/// full flow without touching the process environment.
fn install_with(pkexec: &str, helper_src: Option<&Path>) -> Result<InstallReport, CmdError> {
    let helper_src = helper_src.ok_or_else(|| CmdError {
        code: "helper_missing".to_string(),
        message: "helper binary not found — build it first: cargo build -p fmm-apply".to_string(),
    })?;
    let policy_src = write_policy_temp()?;
    let (script, installed) = install_script(helper_src, &policy_src);
    let args = [
        OsString::from("/bin/sh"),
        OsString::from("-c"),
        OsString::from(script),
    ];
    let result = run_pkexec(pkexec, &args);
    // The staged policy is removed whichever way the install went.
    let _ = std::fs::remove_file(&policy_src);
    result.map(|()| InstallReport { installed })
}

/// Resolves the helper binary to install: `FMM_APPLY_HELPER`, then next to
/// the running executable (dev layout `target/{debug,release}/`), then the
/// installed path (binary step is then skipped as a no-op).
fn helper_source() -> Option<PathBuf> {
    let env_value = std::env::var("FMM_APPLY_HELPER").ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    helper_source_with(env_value.as_deref(), exe_dir.as_deref())
}

fn helper_source_with(env_value: Option<&str>, exe_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = env_value.filter(|p| !p.is_empty()) {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(dir) = exe_dir {
        let candidate = dir.join("fmm-apply");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let installed = PathBuf::from(INSTALLED_HELPER);
    installed.is_file().then_some(installed)
}

/// Writes the embedded policy to a fresh temp file (mode 0644) for `install`.
fn write_policy_temp() -> Result<PathBuf, CmdError> {
    let dir = std::env::temp_dir();
    for n in 0..1000u32 {
        let path = dir.join(format!("fmm-policy-{}-{n}.policy", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            // Name taken (parallel run): try the next suffix.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(CmdError {
                    code: "io".to_string(),
                    message: format!("cannot stage the polkit policy: {e}"),
                });
            }
            Ok(mut file) => {
                file.write_all(POLICY_TEXT.as_bytes())
                    .map_err(|e| CmdError {
                        code: "io".to_string(),
                        message: format!("cannot stage the polkit policy: {e}"),
                    })?;
                #[cfg(unix)]
                file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o644))
                    .map_err(|e| CmdError {
                        code: "io".to_string(),
                        message: format!("cannot stage the polkit policy: {e}"),
                    })?;
                return Ok(path);
            }
        }
    }
    Err(CmdError {
        code: "io".to_string(),
        message: "cannot stage the polkit policy: no free temp name".to_string(),
    })
}

/// Shell single-quotes a path for the `sh -c` install script: every
/// interpolated value is app-controlled, but may contain spaces or quotes.
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

/// The privileged install as ONE pkexec invocation: a shell script running
/// root-owned `/usr/bin/install` once per file, joined by `&&` so a dismissed
/// prompt or any failure installs nothing. Returns the script and the
/// destinations it installs (in order, for the report). The helper step is
/// skipped when the resolved source already is the installed path.
fn install_script(helper_src: &Path, policy_src: &Path) -> (String, Vec<String>) {
    let mut commands = Vec::new();
    let mut installed = Vec::new();
    if helper_src != Path::new(INSTALLED_HELPER) {
        commands.push(install_command(
            &["-D", "-m", "0755", "-o", "root", "-g", "root"],
            helper_src,
            Path::new(INSTALLED_HELPER),
        ));
        installed.push(INSTALLED_HELPER.to_string());
    }
    commands.push(install_command(
        &["-D", "-m", "0644", "-o", "root", "-g", "root"],
        policy_src,
        Path::new(POLICY_INSTALLED),
    ));
    installed.push(POLICY_INSTALLED.to_string());
    (commands.join(" && "), installed)
}

fn install_command(flags: &[&str], source: &Path, dest: &Path) -> String {
    let mut parts: Vec<String> = vec!["/usr/bin/install".to_string()];
    parts.extend(flags.iter().map(|f| f.to_string()));
    parts.push(shell_quote(source));
    parts.push(shell_quote(dest));
    parts.join(" ")
}

/// Runs `pkexec <args…>`; a non-zero exit or spawn failure is reported as
/// `denied` (authorization dismissed or the privileged step failed).
fn run_pkexec(pkexec: &str, args: &[OsString]) -> Result<(), CmdError> {
    let output = Command::new(pkexec)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|child| child.wait_with_output())
        .map_err(|e| CmdError {
            code: "denied".to_string(),
            message: format!("cannot run pkexec: {e}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if stderr.trim().is_empty() {
            "the authorization request was dismissed".to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(CmdError {
            code: "denied".to_string(),
            message: detail,
        });
    }
    Ok(())
}

/// Resolves the helper path: `FMM_APPLY_HELPER` override, else the installed
/// path.
fn helper_path() -> PathBuf {
    match std::env::var("FMM_APPLY_HELPER") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(INSTALLED_HELPER),
    }
}

fn run_helper(plan: &ApplyPlan) -> Result<ApplyReport, CmdError> {
    run_helper_with("pkexec", &helper_path(), plan)
}

/// Spawns `pkexec <helper>` with the plan on stdin and parses the report.
/// `pkexec` is resolved through `PATH`, which lets tests substitute a stub.
fn run_helper_with(pkexec: &str, helper: &Path, plan: &ApplyPlan) -> Result<ApplyReport, CmdError> {
    if !helper.exists() {
        return Err(CmdError {
            code: "helper_missing".to_string(),
            message: format!(
                "privileged helper {} is not installed — see README (install fmm-apply and the polkit policy)",
                helper.display()
            ),
        });
    }
    let json = serde_json::to_string(plan).map_err(|e| CmdError {
        code: "bad_plan".to_string(),
        message: e.to_string(),
    })?;

    let output = Command::new(pkexec)
        .arg(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .expect("stdin is piped")
                .write_all(json.as_bytes())?;
            drop(child.stdin.take()); // signal EOF
            child.wait_with_output()
        })
        .map_err(|e| CmdError {
            code: "denied".to_string(),
            message: format!("cannot run pkexec: {e}"),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        // pkexec produces no output when the authentication dialog is
        // dismissed or denied.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() {
            "not authorized: the authentication request was dismissed".to_string()
        } else {
            format!("not authorized: {}", stderr.trim())
        };
        return Err(CmdError {
            code: "denied".to_string(),
            message,
        });
    }
    let report: ApplyReport = serde_json::from_str(trimmed).map_err(|e| CmdError {
        code: "bad_report".to_string(),
        message: format!("the helper produced an unreadable report: {e}"),
    })?;
    if !report.ok {
        return Err(CmdError {
            code: report.code.clone().unwrap_or_else(|| "helper".to_string()),
            message: report
                .message
                .clone()
                .unwrap_or_else(|| "the privileged operation failed".to_string()),
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmm_core::plan::{PLAN_VERSION, PlanAction};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    fn stub_plan() -> ApplyPlan {
        ApplyPlan {
            plan_version: PLAN_VERSION,
            action: PlanAction::Switch,
            snapshot_root: String::new(),
            snapshot_tag: "test".to_string(),
            refresh_cache: false,
            purge: false,
            snapshot: None,
            files: vec![],
        }
    }

    #[test]
    fn prepare_plan_fills_the_root_from_the_state() {
        let state = AppState {
            snapshot_root: PathBuf::from("/home/u/.local/share/fmm/snapshots"),
        };
        let plan = prepare_plan(stub_plan(), &state.snapshot_root).unwrap();
        assert_eq!(plan.snapshot_root, state.snapshot_root.to_string_lossy());

        // A plan that already carries a root keeps it.
        let mut carried = stub_plan();
        carried.snapshot_root = "/tmp/other".to_string();
        assert_eq!(
            prepare_plan(carried, &state.snapshot_root)
                .unwrap()
                .snapshot_root,
            "/tmp/other"
        );
    }

    #[test]
    fn ensure_snapshot_root_creates_a_user_owned_directory() {
        let state = AppState {
            snapshot_root: temp("ensure").join("snaps"),
        };
        assert!(!state.snapshot_root.exists());
        ensure_snapshot_root(&state).unwrap();
        assert!(state.snapshot_root.is_dir());
    }

    #[test]
    fn legacy_snapshots_list_the_override_root_and_tolerate_a_missing_one() {
        // Missing root: empty, never an error.
        let dir = temp("legacy-missing");
        assert!(legacy_snapshots_with(Some(dir.join("nope").to_str().unwrap())).is_empty());
        // Fake legacy snapshots are listed newest first.
        let legacy = temp("legacy-list");
        fs::create_dir_all(legacy.join("20261101T000000Z-switch-fedora-yandex")).unwrap();
        let listed = legacy_snapshots_with(Some(legacy.to_str().unwrap()));
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].tag, "switch-fedora-yandex");
    }

    #[test]
    fn migrate_reports_pass_through_the_stub_pkexec() {
        let dir = temp("migrate-ok");
        let helper = stub(
            &dir,
            "fmm-apply",
            "cat > /dev/null; echo '{\"ok\":true,\"moved\":[\"20261101T000000Z-x\"],\"failed\":[]}'",
        );
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let report = migrate_legacy_snapshots_with(
            pkexec.to_str().unwrap(),
            &helper,
            &dir.join("snaps"),
            None,
        )
        .unwrap();
        assert!(report.ok);
        assert_eq!(report.moved, vec!["20261101T000000Z-x".to_string()]);
    }

    #[test]
    fn migrate_partial_failure_returns_the_report_as_is() {
        let dir = temp("migrate-fail");
        let helper = stub(
            &dir,
            "fmm-apply",
            "cat > /dev/null; echo '{\"ok\":false,\"moved\":[],\"failed\":[\"a\"]}'; exit 1",
        );
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let report = migrate_legacy_snapshots_with(
            pkexec.to_str().unwrap(),
            &helper,
            &dir.join("snaps"),
            None,
        )
        .unwrap();
        assert!(!report.ok);
        assert_eq!(report.failed, vec!["a".to_string()]);
    }

    #[test]
    fn migrate_dismissed_authorization_and_garbage_are_distinct_errors() {
        let dir = temp("migrate-bad");
        let helper = stub(&dir, "fmm-apply", "exit 1");
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let err = migrate_legacy_snapshots_with(
            pkexec.to_str().unwrap(),
            &helper,
            &dir.join("snaps"),
            None,
        )
        .unwrap_err();
        assert_eq!(err.code, "denied");

        let helper = stub(&dir, "fmm-apply2", "echo garbage");
        let err = migrate_legacy_snapshots_with(
            pkexec.to_str().unwrap(),
            &helper,
            &dir.join("snaps"),
            None,
        )
        .unwrap_err();
        assert_eq!(err.code, "bad_report");

        let err = migrate_legacy_snapshots_with(
            pkexec.to_str().unwrap(),
            Path::new("/fmm/no/such/helper"),
            &dir.join("snaps"),
            None,
        )
        .unwrap_err();
        assert_eq!(err.code, "helper_missing");
    }

    /// Writes an executable stub script and returns its path. The name gets a
    /// per-call suffix: parallel test threads re-writing and re-executing the
    /// same stub file race into `ETXTBSY` on spawn.
    fn stub(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static STUB_SEQ: AtomicU32 = AtomicU32::new(0);
        let path = dir.join(format!(
            "{name}-{}",
            STUB_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        path
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fmm-app-test-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a file under its exact name (no suffix) — for path-resolution
    /// tests that look a file up by name and never execute it.
    fn stub_exact(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        path
    }

    #[test]
    fn helper_missing_is_a_distinct_error() {
        let err =
            run_helper_with("pkexec", Path::new("/fmm/no/such/helper"), &stub_plan()).unwrap_err();
        assert_eq!(err.code, "helper_missing");
        assert!(err.message.contains("not installed"));
    }

    #[test]
    fn successful_report_passes_through() {
        let dir = temp("ok");
        let helper = stub(
            &dir,
            "helper",
            "cat > /dev/null; echo '{\"ok\":true,\"changed\":[\"/x\"],\"unchanged\":[]}'",
        );
        // Stub pkexec: run the helper directly (no polkit in tests).
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let report = run_helper_with(pkexec.to_str().unwrap(), &helper, &stub_plan()).unwrap();
        assert!(report.ok);
        assert_eq!(report.changed, vec!["/x".to_string()]);
    }

    #[test]
    fn failing_report_maps_to_the_report_code() {
        let dir = temp("fail");
        let helper = stub(
            &dir,
            "helper",
            "cat > /dev/null; echo '{\"ok\":false,\"code\":\"stale_plan\",\"message\":\"changed on disk\"}'",
        );
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let err = run_helper_with(pkexec.to_str().unwrap(), &helper, &stub_plan()).unwrap_err();
        assert_eq!(err.code, "stale_plan");
        assert!(err.message.contains("changed on disk"));
    }

    #[test]
    fn empty_output_means_denied() {
        let dir = temp("denied");
        let helper = stub(&dir, "helper", "exit 1");
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let err = run_helper_with(pkexec.to_str().unwrap(), &helper, &stub_plan()).unwrap_err();
        assert_eq!(err.code, "denied");
        assert!(err.message.contains("not authorized"));
    }

    #[test]
    fn cmd_error_from_domain_error_keeps_codes() {
        let err: CmdError = Error::StalePlan("file changed".to_string()).into();
        assert_eq!(err.code, "stale_plan");
        let err: CmdError = Error::Helper {
            code: "io".to_string(),
            message: "disk full".to_string(),
        }
        .into();
        assert_eq!(err.code, "io");
    }

    #[test]
    fn helper_source_prefers_env_then_exe_dir_then_installed() {
        let dir = temp("src");
        // Env wins when the file exists.
        let env_helper = stub(&dir, "env-helper", "exit 0");
        let exe_dir = temp("src-exe");
        let exe_helper = stub_exact(&exe_dir, "fmm-apply", "exit 0");
        assert_eq!(
            helper_source_with(Some(env_helper.to_str().unwrap()), Some(&exe_dir)),
            Some(env_helper.clone())
        );
        // Env pointing nowhere falls through to the exe sibling.
        assert_eq!(
            helper_source_with(Some("/no/such/helper"), Some(&exe_dir)),
            Some(exe_helper.clone())
        );
        // Neither env nor sibling → the installed path, but only when the
        // helper is actually installed on this host (a machine managed by the
        // app has it at the real path, a dev box may not).
        let expected_installed = if Path::new(INSTALLED_HELPER).is_file() {
            Some(PathBuf::from(INSTALLED_HELPER))
        } else {
            None
        };
        assert_eq!(
            helper_source_with(None, Some(&temp("src-empty"))),
            expected_installed
        );
    }

    #[test]
    fn helper_status_follows_the_env_override() {
        let dir = temp("status");
        let helper = stub(&dir, "fmm-apply", "exit 0");
        let status = helper_status_with(Some(helper.to_str().unwrap()));
        assert!(status.installed);
        assert_eq!(status.helper_path, helper.to_string_lossy());
        // Without the override the status reports the installed location.
        assert_eq!(helper_status_with(None).helper_path, INSTALLED_HELPER);
    }

    #[test]
    fn helper_status_flags_a_stale_or_unknown_version() {
        let dir = temp("status-version");
        // Current helper: reports the application's plan version.
        let current = stub(&dir, "current", &format!("echo {PLAN_VERSION}"));
        let status = helper_status_with(Some(current.to_str().unwrap()));
        assert!(status.installed);
        assert_eq!(status.plan_version, Some(PLAN_VERSION));
        assert!(!status.outdated);
        // Older helper: a version, but not the current one.
        let older = stub(&dir, "older", "echo 1");
        let status = helper_status_with(Some(older.to_str().unwrap()));
        assert_eq!(status.plan_version, Some(1));
        assert!(status.outdated);
        // Too old to know the `version` flag: non-zero exit, unknown version.
        let ancient = stub(&dir, "ancient", "exit 2");
        let status = helper_status_with(Some(ancient.to_str().unwrap()));
        assert_eq!(status.plan_version, None);
        assert!(status.outdated);
        // Not installed: nothing is outdated, there is just no helper.
        let status = helper_status_with(Some("/fmm/no/such/helper"));
        assert!(!status.installed);
        assert!(!status.outdated);
        assert_eq!(status.plan_version, None);
    }

    #[test]
    fn shell_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(shell_quote(Path::new("/tmp/fmm-apply")), "'/tmp/fmm-apply'");
        assert_eq!(
            shell_quote(Path::new("/tmp/my dir/fmm-apply")),
            "'/tmp/my dir/fmm-apply'"
        );
        assert_eq!(
            shell_quote(Path::new("/tmp/it's/fmm-apply")),
            "'/tmp/it'\\''s/fmm-apply'"
        );
    }

    #[test]
    fn install_script_installs_both_files_in_one_command() {
        let (script, installed) = install_script(
            Path::new("/tmp/src/fmm-apply"),
            Path::new("/tmp/staged.policy"),
        );
        // One `&&` chain: exactly two install commands, helper first.
        assert_eq!(script.matches("/usr/bin/install").count(), 2);
        assert!(script.contains(
            "/usr/bin/install -D -m 0755 -o root -g root '/tmp/src/fmm-apply' '/usr/libexec/fmm-apply'"
        ));
        assert!(script.contains(
            "/usr/bin/install -D -m 0644 -o root -g root '/tmp/staged.policy' '/usr/share/polkit-1/actions/org.fmm.desktop.apply.policy'"
        ));
        assert_eq!(
            installed,
            vec![INSTALLED_HELPER.to_string(), POLICY_INSTALLED.to_string()]
        );
    }

    #[test]
    fn install_script_skips_reinstall_and_quotes_paths() {
        // Source already installed → only the policy command remains.
        let (script, installed) =
            install_script(Path::new(INSTALLED_HELPER), Path::new("/tmp/staged.policy"));
        assert_eq!(script.matches("/usr/bin/install").count(), 1);
        assert!(!script.contains(" && "));
        assert_eq!(installed, vec![POLICY_INSTALLED.to_string()]);
        // Spaces and single quotes in the source path stay literal.
        let (script, _) = install_script(
            Path::new("/tmp/my dir/it's/fmm-apply"),
            Path::new("/tmp/staged.policy"),
        );
        assert!(script.contains("'/tmp/my dir/it'\\''s/fmm-apply'"));
    }

    /// Policy files currently staged by this test process (`fmm-policy-<pid>-*`).
    fn staged_policy_count() -> usize {
        let prefix = format!("fmm-policy-{}-", std::process::id());
        fs::read_dir(std::env::temp_dir())
            .expect("temp dir is readable")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .count()
    }

    /// Serializes the tests that stage (and count) real policy temp files.
    static POLICY_STAGING_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn install_flow_runs_once_and_cleans_up_with_stub_pkexec() {
        let _staging = POLICY_STAGING_LOCK.lock().unwrap();
        let dir = temp("install");
        // Stub pkexec records each call and succeeds without running the install.
        let counter = dir.join("calls");
        let pkexec = stub(
            &dir,
            "pkexec",
            &format!("echo x >> {}; exit 0", shell_quote(&counter)),
        );
        // Helper source: sibling "fmm-apply" in a fake exe dir.
        let exe_dir = temp("install-exe");
        let helper_src = stub(&exe_dir, "fmm-apply", "exit 0");

        let before = staged_policy_count();
        let report = install_with(pkexec.to_str().unwrap(), Some(&helper_src)).expect("install ok");
        assert_eq!(
            report.installed,
            vec![INSTALLED_HELPER.to_string(), POLICY_INSTALLED.to_string()]
        );
        // ONE pkexec invocation covered both files; the staged policy is gone.
        assert_eq!(fs::read_to_string(&counter).unwrap().lines().count(), 1);
        assert_eq!(staged_policy_count(), before);
    }

    #[test]
    fn install_failure_reports_denied_and_still_cleans_up() {
        let _staging = POLICY_STAGING_LOCK.lock().unwrap();
        let dir = temp("install-fail");
        let counter = dir.join("calls");
        let pkexec = stub(
            &dir,
            "pkexec",
            &format!("echo x >> {}; exit 3", shell_quote(&counter)),
        );

        let before = staged_policy_count();
        let err = install_with(
            pkexec.to_str().unwrap(),
            Some(Path::new("/tmp/no-such/fmm-apply")),
        )
        .unwrap_err();
        assert_eq!(err.code, "denied");
        assert_eq!(fs::read_to_string(&counter).unwrap().lines().count(), 1);
        assert_eq!(staged_policy_count(), before);
    }

    #[test]
    fn install_without_helper_source_fails_before_pkexec() {
        let dir = temp("install-missing");
        let counter = dir.join("calls");
        let pkexec = stub(
            &dir,
            "pkexec",
            &format!("echo x >> {}; exit 0", shell_quote(&counter)),
        );

        let err = install_with(pkexec.to_str().unwrap(), None).unwrap_err();
        assert_eq!(err.code, "helper_missing");
        assert!(!counter.exists());
    }

    #[test]
    fn write_policy_temp_stages_the_embedded_policy() {
        let _staging = POLICY_STAGING_LOCK.lock().unwrap();
        let policy_src = write_policy_temp().expect("stage policy");
        assert!(
            fs::read_to_string(&policy_src)
                .unwrap()
                .contains("org.fmm.desktop.apply")
        );
        let mode = fs::metadata(&policy_src).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o644);
        fs::remove_file(&policy_src).unwrap();
    }
}
