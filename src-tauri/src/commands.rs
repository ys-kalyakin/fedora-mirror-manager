//! Application commands exposed to the frontend.
//!
//! Thin adapter: inventory/plans come from `fmm-core`, privileged work goes
//! through the `fmm-apply` helper invoked with `pkexec`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use fmm_core::error::Error;
use fmm_core::plan::{ApplyPlan, ApplyReport, PlanOutcome, RepoAction};
use fmm_core::reposd::{inventory, Inventory, RepoLocations, DEFAULT_REPO_DIR, DEFAULT_SNAPSHOT_ROOT};
use fmm_core::snapshot::SnapshotInfo;

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
    /// Snapshot root (`/etc/yum.repos.d.bak`).
    pub snapshot_root: PathBuf,
}

impl Default for AppState {
    fn default() -> Self {
        Self { snapshot_root: PathBuf::from(DEFAULT_SNAPSHOT_ROOT) }
    }
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
            Error::NotFound(message) => Self { code: "not_found".to_string(), message },
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
            Error::StalePlan(message) => Self { code: "stale_plan".to_string(), message },
            Error::Snapshot(message) => Self { code: "snapshot_failed".to_string(), message },
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

/// Builds the plan (or a no-change outcome) for one repository action,
/// targeting the file that effectively defines the section.
#[tauri::command]
pub fn preview_action(_state: tauri::State<AppState>, repo_id: String, action: RepoAction) -> Result<PlanOutcome, CmdError> {
    let locations = RepoLocations::detect();
    Ok(fmm_core::plan::build_action_plan(&locations, &repo_id, &action)?)
}

/// `*.repo` files a restore would keep (or purge) because the snapshot
/// predates them.
#[tauri::command]
pub fn preview_restore(state: tauri::State<AppState>, snapshot_id: String) -> Result<Vec<String>, CmdError> {
    Ok(fmm_core::snapshot::restore_preview(Path::new(DEFAULT_REPO_DIR), &state.snapshot_root, &snapshot_id)?)
}

/// Applies a switch plan produced by `preview_action`, asking polkit for
/// authorization. `refresh_cache` controls the post-write `dnf` refresh.
#[tauri::command]
pub fn apply_plan(plan: serde_json::Value, refresh_cache: bool) -> Result<ApplyReport, CmdError> {
    let mut plan: ApplyPlan = serde_json::from_value(plan)
        .map_err(|e| CmdError { code: "bad_plan".to_string(), message: e.to_string() })?;
    plan.refresh_cache = refresh_cache;
    run_helper(&plan)
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
    let mut plan = fmm_core::snapshot::build_restore_plan(Path::new(DEFAULT_REPO_DIR), &state.snapshot_root, &snapshot_id, purge)?;
    plan.refresh_cache = refresh_cache;
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
}

/// Reports whether the privileged helper and its policy are installed.
#[tauri::command]
pub fn helper_status() -> HelperStatus {
    helper_status_with(std::env::var("FMM_APPLY_HELPER").ok().as_deref())
}

fn helper_status_with(env_value: Option<&str>) -> HelperStatus {
    let helper = match env_value {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from(INSTALLED_HELPER),
    };
    HelperStatus {
        installed: helper.is_file(),
        helper_path: helper.to_string_lossy().into_owned(),
        policy_installed: Path::new(POLICY_INSTALLED).exists(),
    }
}

/// Result of the in-app setup: what was installed.
#[derive(Debug, Serialize)]
pub struct InstallReport {
    /// Absolute paths installed by the privileged steps.
    pub installed: Vec<String>,
}

/// Installs the helper binary and the polkit policy from the app itself:
/// two `pkexec /usr/bin/install` steps, each covered by an authorization
/// prompt. Idempotent — safe to run again after a rebuild.
#[tauri::command]
pub fn install_helper() -> Result<InstallReport, CmdError> {
    install_helper_with("pkexec")
}

fn install_helper_with(pkexec: &str) -> Result<InstallReport, CmdError> {
    let helper_src = helper_source().ok_or_else(|| CmdError {
        code: "helper_missing".to_string(),
        message: "helper binary not found — build it first: cargo build -p fmm-apply".to_string(),
    })?;
    let policy_src = write_policy_temp()?;
    let steps = install_steps(&helper_src, &policy_src);

    let mut installed = Vec::new();
    for step in &steps {
        run_pkexec(pkexec, step)?;
        installed.push(step.last().expect("step always ends with the destination").to_string_lossy().into_owned());
    }
    let _ = std::fs::remove_file(&policy_src);
    Ok(InstallReport { installed })
}

/// Resolves the helper binary to install: `FMM_APPLY_HELPER`, then next to
/// the running executable (dev layout `target/{debug,release}/`), then the
/// installed path (binary step is then skipped as a no-op).
fn helper_source() -> Option<PathBuf> {
    let env_value = std::env::var("FMM_APPLY_HELPER").ok();
    let exe_dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
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
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            // Name taken (parallel run): try the next suffix.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(CmdError { code: "io".to_string(), message: format!("cannot stage the polkit policy: {e}") })
            }
            Ok(mut file) => {
                file.write_all(POLICY_TEXT.as_bytes())
                    .map_err(|e| CmdError { code: "io".to_string(), message: format!("cannot stage the polkit policy: {e}") })?;
                #[cfg(unix)]
                file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o644))
                    .map_err(|e| CmdError { code: "io".to_string(), message: format!("cannot stage the polkit policy: {e}") })?;
                return Ok(path);
            }
        }
    }
    Err(CmdError { code: "io".to_string(), message: "cannot stage the polkit policy: no free temp name".to_string() })
}

/// The privileged install steps: helper binary (skipped when the resolved
/// source already is the installed path) and the polkit policy. Direct
/// arguments to the root-owned `/usr/bin/install` — no shell, no quoting.
fn install_steps(helper_src: &Path, policy_src: &Path) -> Vec<Vec<PathBuf>> {
    let mut steps = Vec::new();
    if helper_src != Path::new(INSTALLED_HELPER) {
        steps.push(install_step(&[
            "-D", "-m", "0755", "-o", "root", "-g", "root",
        ], helper_src, Path::new(INSTALLED_HELPER)));
    }
    steps.push(install_step(&["-D", "-m", "0644", "-o", "root", "-g", "root"], policy_src, Path::new(POLICY_INSTALLED)));
    steps
}

fn install_step(flags: &[&str], source: &Path, dest: &Path) -> Vec<PathBuf> {
    let mut args: Vec<PathBuf> = vec![PathBuf::from("/usr/bin/install")];
    args.extend(flags.iter().map(PathBuf::from));
    args.push(source.to_path_buf());
    args.push(dest.to_path_buf());
    args
}

/// Runs `pkexec <args…>`; a non-zero exit or spawn failure is reported as
/// `denied` (authorization dismissed or the privileged step failed).
fn run_pkexec(pkexec: &str, args: &[PathBuf]) -> Result<(), CmdError> {
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
        return Err(CmdError { code: "denied".to_string(), message: detail });
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
    let json = serde_json::to_string(plan)
        .map_err(|e| CmdError { code: "bad_plan".to_string(), message: e.to_string() })?;

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
        return Err(CmdError { code: "denied".to_string(), message });
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
    use fmm_core::plan::{PlanAction, PLAN_VERSION};
    use std::fs;

    fn stub_plan() -> ApplyPlan {
        ApplyPlan {
            plan_version: PLAN_VERSION,
            action: PlanAction::Switch,
            snapshot_tag: "test".to_string(),
            refresh_cache: false,
            purge: false,
            snapshot: None,
            files: vec![],
        }
    }

    /// Writes an executable stub script and returns its path.
    fn stub(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        path
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fmm-app-test-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn helper_missing_is_a_distinct_error() {
        let err = run_helper_with("pkexec", Path::new("/fmm/no/such/helper"), &stub_plan()).unwrap_err();
        assert_eq!(err.code, "helper_missing");
        assert!(err.message.contains("not installed"));
    }

    #[test]
    fn successful_report_passes_through() {
        let dir = temp("ok");
        let helper = stub(&dir, "helper", "cat > /dev/null; echo '{\"ok\":true,\"changed\":[\"/x\"],\"unchanged\":[]}'");
        // Stub pkexec: run the helper directly (no polkit in tests).
        let pkexec = stub(&dir, "pkexec", "exec \"$@\"");
        let report = run_helper_with(pkexec.to_str().unwrap(), &helper, &stub_plan()).unwrap();
        assert!(report.ok);
        assert_eq!(report.changed, vec!["/x".to_string()]);
    }

    #[test]
    fn failing_report_maps_to_the_report_code() {
        let dir = temp("fail");
        let helper = stub(&dir, "helper", "cat > /dev/null; echo '{\"ok\":false,\"code\":\"stale_plan\",\"message\":\"changed on disk\"}'");
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
        let err: CmdError = Error::Helper { code: "io".to_string(), message: "disk full".to_string() }.into();
        assert_eq!(err.code, "io");
    }

    #[test]
    fn helper_source_prefers_env_then_exe_dir_then_installed() {
        let dir = temp("src");
        // Env wins when the file exists.
        let env_helper = stub(&dir, "env-helper", "exit 0");
        let exe_dir = temp("src-exe");
        let exe_helper = stub(&exe_dir, "fmm-apply", "exit 0");
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
        assert_eq!(helper_source_with(None, Some(&temp("src-empty"))), expected_installed);
    }

    #[test]
    fn install_steps_cover_binary_and_policy_and_skip_reinstall() {
        let policy = Path::new("/tmp/fmm-policy.policy");
        // Fresh source → two steps: helper, then policy.
        let steps = install_steps(Path::new("/tmp/fmm-apply"), policy);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0][0], PathBuf::from("/usr/bin/install"));
        assert_eq!(steps[0][1], PathBuf::from("-D"));
        assert_eq!(steps[0].last().unwrap(), Path::new(INSTALLED_HELPER));
        assert_eq!(steps[1].last().unwrap(), Path::new(POLICY_INSTALLED));
        // Source already installed → only the policy step remains.
        let steps = install_steps(Path::new(INSTALLED_HELPER), policy);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].last().unwrap(), Path::new(POLICY_INSTALLED));
    }

    #[test]
    fn install_flow_runs_and_cleans_up_with_stub_pkexec() {
        let dir = temp("install");
        // Stub pkexec succeeds without running the real install.
        let pkexec = stub(&dir, "pkexec", "exit 0");
        // Helper source: sibling "fmm-apply" next to a fake exe dir.
        let exe_dir = temp("install-exe");
        stub(&exe_dir, "fmm-apply", "exit 0");

        // Temporarily point the resolution at the fake exe dir via the env slot.
        let helper_src = exe_dir.join("fmm-apply");
        let env_value = helper_src.to_str().unwrap();
        // install_helper_with resolves via helper_source(); emulate the dev
        // layout by checking the env branch through the status too.
        let status = helper_status_with(Some(env_value));
        assert!(status.installed);

        // The full flow: resolve, stage policy, run both steps via the stub.
        let src = helper_source_with(Some(env_value), None).unwrap();
        let policy_src = write_policy_temp().expect("stage policy");
        let steps = install_steps(&src, &policy_src);
        for step in &steps {
            run_pkexec(pkexec.to_str().unwrap(), step).expect("stub step");
        }
        assert_eq!(steps.len(), 2);
        // The staged policy file exists until cleanup; verify content round-trip.
        let staged = std::fs::read_to_string(&policy_src).unwrap();
        assert!(staged.contains("org.fmm.desktop.apply"));
        std::fs::remove_file(&policy_src).unwrap();
    }
}
