//! Integration tests: run the built `fmm-apply` binary against temp
//! directories with a stub `dnf`, without root or polkit (tasks 5.2-5.4).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use fmm_core::plan::{ApplyPlan, ApplyReport, PlanAction};
use fmm_core::snapshot;

/// Fresh temp directory.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fmm-apply-test-{}-{tag}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// A stub `dnf` that logs its invocation and exits with a configurable code.
fn dnf_stub(dir: &Path, exit_code: i32) -> PathBuf {
    let path = dir.join("fake-dnf.sh");
    let mut script = fs::File::create(&path).unwrap();
    writeln!(script, "#!/bin/sh").unwrap();
    writeln!(script, "echo \"dnf $*\" >> \"$FMM_DNF_LOG\"").unwrap();
    writeln!(script, "echo \"ran: dnf $*\" >&2").unwrap();
    writeln!(script, "exit {exit_code}").unwrap();
    drop(script);
    fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    path
}

/// Runs the helper binary with the plan on stdin; returns (report, raw stdout).
fn run_helper(plan: &ApplyPlan, repo_dir: &Path, snap_root: &Path, dnf_bin: &Path, dnf_log: &Path) -> ApplyReport {
    let json = serde_json::to_string(plan).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fmm-apply"))
        .arg("--repo-dir")
        .arg(repo_dir)
        .arg("--snapshot-root")
        .arg(snap_root)
        .arg("--dnf-bin")
        .arg(dnf_bin)
        .env("FMM_DNF_LOG", dnf_log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn helper");
    child.stdin.as_mut().unwrap().write_all(json.as_bytes()).unwrap();
    let out = child.wait_with_output().expect("wait helper");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim()).expect("report JSON on stdout")
}

/// Minimal switch plan for one file.
fn switch_plan(path: &Path, content: &str) -> ApplyPlan {
    use fmm_core::plan::{PlanFile, PLAN_VERSION};
    let before = fs::read(path).unwrap();
    ApplyPlan {
        plan_version: PLAN_VERSION,
        action: PlanAction::Switch,
        snapshot_tag: "switch-test".to_string(),
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files: vec![PlanFile {
            path: path.to_string_lossy().into_owned(),
            before_sha256: fmm_core::plan::sha256_hex(&before),
            content: content.to_string(),
        }],
    }
}

const REPO_A: &str = "[fedora]\nname=Fedora\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=fedora\ngpgcheck=1\n";

#[test]
fn successful_switch_writes_updates_the_snapshot_and_refreshes_cache() {
    let work = temp_dir("switch");
    let repo_dir = work.join("repos");
    let snap_root = work.join("snaps");
    fs::create_dir_all(&repo_dir).unwrap();
    let repo = repo_dir.join("fedora.repo");
    fs::write(&repo, REPO_A).unwrap();

    let new_content = "[fedora]\nname=Fedora\nbaseurl=https://mirror.yandex.ru/fedora/linux/releases/$releasever/Everything/$basearch/os/\ngpgcheck=1\n";
    let mut plan = switch_plan(&repo, new_content);
    plan.refresh_cache = true;

    let stub = dnf_stub(&work, 0);
    let log = work.join("dnf.log");
    let report = run_helper(&plan, &repo_dir, &snap_root, &stub, &log);

    assert!(report.ok, "report: {report:?}");
    assert_eq!(report.changed, vec![repo.to_string_lossy().into_owned()]);
    assert!(report.unchanged.is_empty());
    // File content applied.
    assert_eq!(fs::read_to_string(&repo).unwrap(), new_content);
    // Snapshot created with the pre-switch content.
    let snap = report.snapshot.expect("snapshot");
    assert_eq!(fs::read_to_string(Path::new(&snap).join("fedora.repo")).unwrap(), REPO_A);
    // Cache refreshed through the stub.
    assert!(report.cache.as_ref().expect("cache").ok);
    let logged = fs::read_to_string(&log).unwrap();
    assert!(logged.contains("dnf clean all"));
    assert!(logged.contains("dnf makecache"));
}

#[test]
fn identical_content_is_reported_as_unchanged_without_writing() {
    let work = temp_dir("noop");
    let repo_dir = work.join("repos");
    fs::create_dir_all(&repo_dir).unwrap();
    let repo = repo_dir.join("fedora.repo");
    fs::write(&repo, REPO_A).unwrap();

    // Plan content equals the current content.
    let plan = switch_plan(&repo, REPO_A);
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &repo_dir, &work.join("snaps"), &stub, &work.join("dnf.log"));

    assert!(report.ok, "report: {report:?}");
    assert!(report.changed.is_empty());
    assert_eq!(report.unchanged, vec![repo.to_string_lossy().into_owned()]);
    assert_eq!(fs::read_to_string(&repo).unwrap(), REPO_A);
}

#[test]
fn stale_plan_aborts_before_any_write() {
    let work = temp_dir("stale");
    let repo_dir = work.join("repos");
    fs::create_dir_all(&repo_dir).unwrap();
    let repo = repo_dir.join("fedora.repo");
    fs::write(&repo, REPO_A).unwrap();

    let mut plan = switch_plan(&repo, "[fedora]\nname=New\n");
    plan.files[0].before_sha256 = "0".repeat(64); // wrong hash
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &repo_dir, &work.join("snaps"), &stub, &work.join("dnf.log"));

    assert!(!report.ok);
    assert_eq!(report.code.as_deref(), Some("stale_plan"));
    // File untouched.
    assert_eq!(fs::read_to_string(&repo).unwrap(), REPO_A);
}

#[test]
fn empty_hash_creates_a_missing_file() {
    let work = temp_dir("create");
    let repo_dir = work.join("repos");
    fs::create_dir_all(&repo_dir).unwrap();
    let shadow = repo_dir.join("fedora-updates.repo");
    assert!(!shadow.exists());

    let plan = ApplyPlan {
        plan_version: fmm_core::plan::PLAN_VERSION,
        action: PlanAction::Switch,
        snapshot_tag: "switch-test".to_string(),
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files: vec![fmm_core::plan::PlanFile {
            path: shadow.to_string_lossy().into_owned(),
            before_sha256: String::new(), // create-new sentinel
            content: REPO_A.to_string(),
        }],
    };
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &repo_dir, &work.join("snaps"), &stub, &work.join("dnf.log"));

    assert!(report.ok, "report: {report:?}");
    assert_eq!(report.changed, vec![shadow.to_string_lossy().into_owned()]);
    assert_eq!(fs::read_to_string(&shadow).unwrap(), REPO_A);
    // Created with mode 0644.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&shadow).unwrap().permissions().mode() & 0o777, 0o644);
    }
}

#[test]
fn create_conflicts_with_an_existing_file_are_stale() {
    let work = temp_dir("create-conflict");
    let repo_dir = work.join("repos");
    fs::create_dir_all(&repo_dir).unwrap();
    let shadow = repo_dir.join("fedora-updates.repo");
    fs::write(&shadow, REPO_A).unwrap();

    let plan = ApplyPlan {
        plan_version: fmm_core::plan::PLAN_VERSION,
        action: PlanAction::Switch,
        snapshot_tag: "switch-test".to_string(),
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files: vec![fmm_core::plan::PlanFile {
            path: shadow.to_string_lossy().into_owned(),
            before_sha256: String::new(), // create-new sentinel
            content: "[updates]\nname=X\n".to_string(),
        }],
    };
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &repo_dir, &work.join("snaps"), &stub, &work.join("dnf.log"));

    assert!(!report.ok);
    assert_eq!(report.code.as_deref(), Some("stale_plan"));
    // The existing file is untouched.
    assert_eq!(fs::read_to_string(&shadow).unwrap(), REPO_A);
}

#[test]
fn snapshot_failure_aborts_the_write() {
    let work = temp_dir("snapfail");
    let repo_dir = work.join("repos");
    let stub = dnf_stub(&work, 0);
    // Plan for a file that does not even exist (the snapshot fails first).
    let plan = ApplyPlan {
        plan_version: fmm_core::plan::PLAN_VERSION,
        action: PlanAction::Switch,
        snapshot_tag: "switch-test".to_string(),
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files: vec![fmm_core::plan::PlanFile {
            path: work.join("missing.repo").to_string_lossy().into_owned(),
            before_sha256: "0".repeat(64),
            content: "[x]\n".to_string(),
        }],
    };
    let report = run_helper(&plan, &repo_dir, &work.join("snaps"), &stub, &work.join("dnf.log"));

    assert!(!report.ok);
    assert_eq!(report.code.as_deref(), Some("snapshot_failed"));
}

#[test]
fn makecache_failure_is_a_warning_not_a_failure() {
    let work = temp_dir("dnfwarn");
    let repo_dir = work.join("repos");
    fs::create_dir_all(&repo_dir).unwrap();
    let repo = repo_dir.join("fedora.repo");
    fs::write(&repo, REPO_A).unwrap();

    let mut plan = switch_plan(&repo, "[fedora]\nname=Fedora2\n");
    plan.refresh_cache = true;
    let stub = dnf_stub(&work, 1); // makecache fails
    let report = run_helper(&plan, &repo_dir, &work.join("snaps"), &stub, &work.join("dnf.log"));

    assert!(report.ok, "report: {report:?}");
    let cache = report.cache.expect("cache");
    assert!(!cache.ok);
    assert!(cache.output_tail.contains("dnf makecache"));
    assert_eq!(fs::read_to_string(&repo).unwrap(), "[fedora]\nname=Fedora2\n");
}

fn restore_setup(tag: &str, extra_file: bool) -> (PathBuf, PathBuf, PathBuf, String) {
    let work = temp_dir(tag);
    let repo_dir = work.join("repos");
    let snap_root = work.join("snaps");
    fs::create_dir_all(&repo_dir).unwrap();
    let repo = repo_dir.join("fedora.repo");
    fs::write(&repo, REPO_A).unwrap();
    // Snapshot the pristine state first, then diverge from it.
    let snap = snapshot::create_snapshot(&repo_dir, &snap_root, "pristine").unwrap();
    if extra_file {
        fs::write(repo_dir.join("added-later.repo"), "[added]\nname=x\n").unwrap();
    }
    fs::write(&repo, "[fedora]\nname=Mutated\n").unwrap();
    let snap_id = snap.file_name().unwrap().to_string_lossy().into_owned();
    (work, repo_dir, snap_root, snap_id)
}

#[test]
fn restore_without_purge_is_byte_for_byte_and_reports_extras() {
    let (work, repo_dir, snap_root, snap_id) = restore_setup("restore-keep", true);

    let mut plan = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, &snap_id, false).unwrap();
    plan.refresh_cache = true;
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &repo_dir, &snap_root, &stub, &work.join("dnf.log"));

    assert!(report.ok, "report: {report:?}");
    // The mutated file is back to the snapshot content.
    assert_eq!(fs::read_to_string(repo_dir.join("fedora.repo")).unwrap(), REPO_A);
    // The later file is kept and reported.
    assert!(repo_dir.join("added-later.repo").is_file());
    assert_eq!(report.extra_files, vec!["added-later.repo".to_string()]);
    // The pre-restore snapshot captured the mutated state.
    let pre_restore = report.snapshot.expect("snapshot");
    assert_eq!(
        fs::read_to_string(Path::new(&pre_restore).join("fedora.repo")).unwrap(),
        "[fedora]\nname=Mutated\n"
    );
}

#[test]
fn restore_with_purge_removes_later_files() {
    let (work, repo_dir, snap_root, snap_id) = restore_setup("restore-purge", true);

    let plan = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, &snap_id, true).unwrap();
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &repo_dir, &snap_root, &stub, &work.join("dnf.log"));

    assert!(report.ok, "report: {report:?}");
    assert!(report.extra_files.is_empty());
    assert!(!repo_dir.join("added-later.repo").exists());
    assert_eq!(fs::read_to_string(repo_dir.join("fedora.repo")).unwrap(), REPO_A);
}

#[test]
fn restore_plan_without_snapshot_path_is_rejected() {
    let work = temp_dir("restore-bad");
    let plan = ApplyPlan {
        plan_version: fmm_core::plan::PLAN_VERSION,
        action: PlanAction::Restore,
        snapshot_tag: "before-restore".to_string(),
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files: vec![],
    };
    let stub = dnf_stub(&work, 0);
    let report = run_helper(&plan, &work.join("repos"), &work.join("snaps"), &stub, &work.join("dnf.log"));
    assert!(!report.ok);
    assert_eq!(report.code.as_deref(), Some("bad_plan"));
}
