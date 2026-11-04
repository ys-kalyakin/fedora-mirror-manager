//! Privileged helper (`fmm-apply`): applies repository plans as root.
//!
//! Invoked via `pkexec /usr/libexec/fmm-apply`. Reads an [`fmm_core::plan::ApplyPlan`]
//! as JSON from stdin, performs the requested action (switch or restore) with
//! an unconditional pre-write snapshot, and writes an
//! [`fmm_core::plan::ApplyReport`] as JSON to stdout. Exit code 0 means the
//! report was produced (`report.ok` says whether the work succeeded); exit 1
//! means the helper itself failed before/while producing a report.
//!
//! `fmm-apply migrate-snapshots` is a second, plan-less mode: a one-shot
//! migration of the legacy snapshot root into the user snapshot root, covered
//! by the same single polkit prompt (see [`migrate_main`]). `fmm-apply
//! version` prints the plan-protocol version for the application's stale
//! install probe (no polkit, no I/O).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use fmm_core::error::Error;
use fmm_core::plan::{ApplyPlan, ApplyReport, CacheReport, PlanAction};
use fmm_core::snapshot::{self, MigrateReport};
use sha2::{Digest, Sha256};

/// Default `dnf` executable (resolved through the helper's PATH).
const DEFAULT_DNF_BIN: &str = "dnf";
/// How many characters of dnf output are kept in the report.
const OUTPUT_TAIL_CHARS: usize = 2000;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("version") {
        // Plan-protocol version probe: the application execs the installed
        // helper directly (no polkit) to detect stale installs. Prints a
        // bare integer and exits without reading stdin.
        println!("{}", fmm_core::plan::PLAN_VERSION);
        return;
    }
    if argv.first().map(String::as_str) == Some("migrate-snapshots") {
        migrate_main(&argv[1..]);
        return;
    }
    plan_main();
}

/// Plan mode: one plan on stdin, one report on stdout.
fn plan_main() {
    let args = Args::parse();
    let report = match run(&args) {
        Ok(report) => report,
        Err(failure) => failure.into(),
    };
    let ok = report.ok;
    match serde_json::to_string(&report) {
        Ok(json) => {
            println!("{json}");
            std::process::exit(if ok { 0 } else { 1 });
        }
        Err(err) => {
            eprintln!("fmm-apply: cannot serialize report: {err}");
            std::process::exit(1);
        }
    }
}

/// Parsed command-line arguments of the plan mode. The snapshot root is not
/// among them: the plan carries it (the application resolves the user root).
struct Args {
    repo_dir: PathBuf,
    dnf_bin: String,
}

impl Args {
    fn parse() -> Self {
        let mut repo_dir = PathBuf::from(fmm_core::reposd::DEFAULT_REPO_DIR);
        let mut dnf_bin = DEFAULT_DNF_BIN.to_string();
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--repo-dir" => repo_dir = it.next().map(PathBuf::from).unwrap_or(repo_dir),
                "--dnf-bin" => {
                    if let Some(v) = it.next() {
                        dnf_bin = v;
                    }
                }
                other => {
                    eprintln!("fmm-apply: unknown argument: {other}");
                    std::process::exit(2);
                }
            }
        }
        Self { repo_dir, dnf_bin }
    }
}

/// Parsed arguments of the migration mode. `--snapshot-root` is a dev/test
/// override; both roots default to the app-resolved locations.
struct MigrateArgs {
    snapshot_root: PathBuf,
    legacy_root: PathBuf,
}

impl MigrateArgs {
    fn parse(argv: &[String]) -> Self {
        let mut snapshot_root = fmm_core::reposd::user_snapshot_root();
        let mut legacy_root = PathBuf::from(fmm_core::reposd::LEGACY_SNAPSHOT_ROOT);
        let mut it = argv.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--snapshot-root" => {
                    snapshot_root = it.next().map(PathBuf::from).unwrap_or(snapshot_root)
                }
                "--legacy-root" => {
                    legacy_root = it.next().map(PathBuf::from).unwrap_or(legacy_root)
                }
                other => {
                    eprintln!("fmm-apply: unknown argument: {other}");
                    std::process::exit(2);
                }
            }
        }
        Self {
            snapshot_root,
            legacy_root,
        }
    }
}

/// Migration mode: move the legacy snapshot root into the user snapshot root
/// under one polkit prompt and print a
/// [`fmm_core::snapshot::MigrateReport`] as JSON on stdout.
fn migrate_main(argv: &[String]) {
    let args = MigrateArgs::parse(argv);
    let report = match run_migrate(&args) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("fmm-apply: {message}");
            std::process::exit(1);
        }
    };
    match serde_json::to_string(&report) {
        Ok(json) => {
            println!("{json}");
            std::process::exit(if report.ok { 0 } else { 1 });
        }
        Err(err) => {
            eprintln!("fmm-apply: cannot serialize report: {err}");
            std::process::exit(1);
        }
    }
}

/// The migration: create the user root, move the legacy snapshots, hand each
/// moved snapshot over to the invoking user; `migrate_snapshots` already
/// removed the legacy root when the moves left it empty.
fn run_migrate(args: &MigrateArgs) -> Result<MigrateReport, String> {
    std::fs::create_dir_all(&args.snapshot_root)
        .map_err(|e| format!("cannot create {}: {e}", args.snapshot_root.display()))?;
    let mut report = snapshot::migrate_snapshots(&args.legacy_root, &args.snapshot_root);
    let uid = invoking_uid(
        std::env::var("PKEXEC_UID").ok().as_deref(),
        &args.snapshot_root,
    )?;
    let moved = std::mem::take(&mut report.moved);
    for name in moved {
        match chown_tree(&args.snapshot_root.join(&name), uid) {
            Ok(()) => report.moved.push(name),
            Err(message) => {
                eprintln!("fmm-apply: {message}");
                report.failed.push(name);
                report.ok = false;
            }
        }
    }
    Ok(report)
}

fn run(args: &Args) -> Result<ApplyReport, Failure> {
    let mut stdin = std::io::stdin().lock();
    let mut input = String::new();
    stdin
        .read_to_string(&mut input)
        .map_err(|e| Failure::new("io", format!("cannot read plan: {e}")))?;
    let plan: ApplyPlan = serde_json::from_str(&input)
        .map_err(|e| Failure::new("bad_plan", format!("cannot parse plan: {e}")))?;
    if plan.plan_version != fmm_core::plan::PLAN_VERSION {
        return Err(Failure::new(
            "bad_plan",
            format!("unsupported plan version {}", plan.plan_version),
        ));
    }
    match plan.action {
        PlanAction::Switch => apply_switch(&plan, args),
        PlanAction::Restore => apply_restore(&plan, args),
    }
}

/// A helper failure reported as `{ok: false, code, message}`.
#[derive(Debug)]
struct Failure {
    code: String,
    message: String,
}

impl Failure {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        match err {
            Error::StalePlan(m) => Self::new("stale_plan", m),
            Error::Snapshot(m) => Self::new("snapshot_failed", m),
            Error::Io { path, source } => Self::new("io", format!("{}: {source}", path.display())),
            other => Self::new("io", other.to_string()),
        }
    }
}

impl From<Failure> for ApplyReport {
    fn from(f: Failure) -> Self {
        ApplyReport {
            ok: false,
            code: Some(f.code),
            message: Some(f.message),
            snapshot: None,
            changed: Vec::new(),
            unchanged: Vec::new(),
            extra_files: Vec::new(),
            cache: None,
        }
    }
}

/// The report builder for a successful (or warning-carrying) run.
#[derive(Default)]
struct Report {
    snapshot: Option<String>,
    changed: Vec<String>,
    unchanged: Vec<String>,
    extra_files: Vec<String>,
    cache: Option<CacheReport>,
}

impl Report {
    fn into_report(self, failure: Option<Failure>) -> ApplyReport {
        match failure {
            None => ApplyReport {
                ok: true,
                code: None,
                message: None,
                snapshot: self.snapshot,
                changed: self.changed,
                unchanged: self.unchanged,
                extra_files: self.extra_files,
                cache: self.cache,
            },
            Some(f) => {
                let mut report: ApplyReport = f.into();
                // The snapshot may exist even when a later step failed.
                report.snapshot = self.snapshot;
                report
            }
        }
    }
}

/// Switch action: snapshot → hand it over → verify hashes → write → prune →
/// (refresh cache).
fn apply_switch(plan: &ApplyPlan, args: &Args) -> Result<ApplyReport, Failure> {
    let snap_root = plan_root(plan)?;
    let snap = snapshot::create_snapshot(&args.repo_dir, &snap_root, &plan.snapshot_tag)
        .map_err(Failure::from)?;
    let mut report = Report {
        snapshot: Some(snap.to_string_lossy().into_owned()),
        ..Default::default()
    };
    if let Err(failure) = hand_over_snapshot(&snap_root, &snap) {
        // The snapshot exists: report it alongside the failure.
        return Ok(report.into_report(Some(failure)));
    }

    // Verify every file first: a mismatch aborts before the first write.
    let mut to_write = Vec::new();
    let mut to_create = Vec::new();
    for file in &plan.files {
        let path = Path::new(&file.path);
        if file.before_sha256.is_empty() {
            // Sentinel: the plan creates the file (F45+ overriding copy); it
            // must not exist yet, or the plan is stale.
            if path.exists() {
                return Err(Failure::new(
                    "stale_plan",
                    format!(
                        "{} already exists (the plan was built to create it)",
                        file.path
                    ),
                ));
            }
            to_create.push(file);
            continue;
        }
        let current = std::fs::read(path)
            .map_err(|e| Failure::new("stale_plan", format!("cannot read {}: {e}", file.path)))?;
        let actual = sha256_hex(&current);
        if actual != file.before_sha256 {
            return Err(Failure::new(
                "stale_plan",
                format!("{} changed on disk since the plan was built", file.path),
            ));
        }
        if current == file.content.as_bytes() {
            report.unchanged.push(file.path.clone());
        } else {
            to_write.push(file);
        }
    }

    for file in to_write {
        write_in_place(Path::new(&file.path), file.content.as_bytes())?;
        report.changed.push(file.path.clone());
    }

    for file in to_create {
        create_in_place(Path::new(&file.path), file.content.as_bytes())?;
        report.changed.push(file.path.clone());
    }

    if plan.refresh_cache {
        report.cache = Some(refresh_cache(&args.dnf_bin));
    }
    Ok(report.into_report(None))
}

/// Restore action: snapshot the current state → hand it over → copy the
/// snapshot back → keep or purge extra files → (refresh cache).
fn apply_restore(plan: &ApplyPlan, args: &Args) -> Result<ApplyReport, Failure> {
    let snap_dir = plan
        .snapshot
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| Failure::new("bad_plan", "restore plan without a snapshot path"))?;
    if !snap_dir.is_dir() {
        return Err(Failure::new(
            "io",
            format!("snapshot {} not found", snap_dir.display()),
        ));
    }

    let snap_root = plan_root(plan)?;
    let snap = snapshot::create_snapshot(&args.repo_dir, &snap_root, &plan.snapshot_tag)
        .map_err(Failure::from)?;
    let mut report = Report {
        snapshot: Some(snap.to_string_lossy().into_owned()),
        ..Default::default()
    };
    if let Err(failure) = hand_over_snapshot(&snap_root, &snap) {
        return Ok(report.into_report(Some(failure)));
    }

    snapshot::copy_tree(&snap_dir, &args.repo_dir)?;
    for entry in std::fs::read_dir(&snap_dir)
        .map_err(|e| Failure::new("io", format!("cannot read {}: {e}", snap_dir.display())))?
    {
        let entry = entry
            .map_err(|e| Failure::new("io", format!("cannot read {}: {e}", snap_dir.display())))?;
        report.changed.push(
            args.repo_dir
                .join(entry.file_name())
                .to_string_lossy()
                .into_owned(),
        );
    }

    // Files that appeared after the snapshot: keep and report, or purge.
    for name in snapshot::extra_repo_files(&args.repo_dir, &snap_dir)? {
        if plan.purge {
            std::fs::remove_file(args.repo_dir.join(&name))
                .map_err(|e| Failure::new("io", format!("cannot purge {}: {e}", name)))?;
            report
                .changed
                .push(args.repo_dir.join(&name).to_string_lossy().into_owned());
        } else {
            report.extra_files.push(name);
        }
    }

    if plan.refresh_cache {
        report.cache = Some(refresh_cache(&args.dnf_bin));
    }
    Ok(report.into_report(None))
}

/// The plan's snapshot root is authoritative: the application resolves the
/// user snapshot root and fills the field; the helper never guesses a
/// directory.
fn plan_root(plan: &ApplyPlan) -> Result<PathBuf, Failure> {
    if plan.snapshot_root.is_empty() {
        return Err(Failure::new("bad_plan", "plan without a snapshot root"));
    }
    Ok(PathBuf::from(&plan.snapshot_root))
}

/// Hands the freshly created snapshot over to the invoking user, so the
/// unprivileged application can list, preview and delete it. A failure is a
/// snapshot failure: it aborts the privileged write before any file changes.
fn hand_over_snapshot(snapshot_root: &Path, snap: &Path) -> Result<(), Failure> {
    let uid = invoking_uid(std::env::var("PKEXEC_UID").ok().as_deref(), snapshot_root)
        .map_err(|m| Failure::new("snapshot_failed", m))?;
    chown_tree(snap, uid).map_err(|m| Failure::new("snapshot_failed", m))
}

/// The uid snapshots are handed to: pkexec sets `PKEXEC_UID` to the invoking
/// user; without it (manual runs) the snapshot root's owner is used — the
/// unprivileged application created that root as the user beforehand.
#[cfg(unix)]
fn invoking_uid(pkexec_uid: Option<&str>, snapshot_root: &Path) -> Result<u32, String> {
    use std::os::unix::fs::MetadataExt;
    if let Some(value) = pkexec_uid.map(str::trim).filter(|v| !v.is_empty()) {
        return value
            .parse::<u32>()
            .map_err(|e| format!("invalid PKEXEC_UID {value:?}: {e}"));
    }
    std::fs::metadata(snapshot_root)
        .map(|meta| meta.uid())
        .map_err(|e| format!("cannot stat {}: {e}", snapshot_root.display()))
}

#[cfg(not(unix))]
fn invoking_uid(_pkexec_uid: Option<&str>, _snapshot_root: &Path) -> Result<u32, String> {
    Ok(0)
}

/// Recursively hands `path` over to `uid` (group ownership and file modes
/// untouched, `std::os::unix::fs::chown` with `None` group).
#[cfg(unix)]
fn chown_tree(path: &Path, uid: u32) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
    if meta.is_dir() {
        let read =
            std::fs::read_dir(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        for entry in read {
            let entry = entry.map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            chown_tree(&entry.path(), uid)?;
        }
    }
    std::os::unix::fs::chown(path, Some(uid), None)
        .map_err(|e| format!("cannot change owner of {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn chown_tree(_path: &Path, _uid: u32) -> Result<(), String> {
    Ok(())
}

/// Creates the file (mode 0644; ownership follows the running root), making
/// the parent directory when needed.
fn create_in_place(path: &Path, content: &[u8]) -> Result<(), Failure> {
    use std::fs::OpenOptions;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(parent) = path.parent().filter(|parent| !parent.exists()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| Failure::new("io", format!("cannot create {}: {e}", parent.display())))?;
    }
    #[cfg(unix)]
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(path)
        .map_err(|e| Failure::new("io", format!("cannot create {}: {e}", path.display())))?;
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| Failure::new("io", format!("cannot create {}: {e}", path.display())))?;
    file.write_all(content)
        .map_err(|e| Failure::new("io", format!("cannot write {}: {e}", path.display())))?;
    file.sync_all()
        .map_err(|e| Failure::new("io", format!("cannot sync {}: {e}", path.display())))?;
    Ok(())
}

/// Overwrites the file in place (same inode: owner, mode and SELinux context
/// survive, like the reference script's `cat > file`).
fn write_in_place(path: &Path, content: &[u8]) -> Result<(), Failure> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|e| Failure::new("io", format!("cannot open {}: {e}", path.display())))?;
    file.write_all(content)
        .map_err(|e| Failure::new("io", format!("cannot write {}: {e}", path.display())))?;
    file.sync_all()
        .map_err(|e| Failure::new("io", format!("cannot sync {}: {e}", path.display())))?;
    Ok(())
}

/// Runs `dnf clean all` then `dnf makecache`. A makecache failure is a
/// warning in the report, never a failure of the applied work.
fn refresh_cache(dnf_bin: &str) -> CacheReport {
    let mut output = String::new();
    let mut ok = true;
    let steps: [&[&str]; 2] = [&["clean", "all"], &["makecache"]];
    for step in steps {
        match Command::new(dnf_bin).args(step).output() {
            Ok(out) => {
                output.push_str(&String::from_utf8_lossy(&out.stdout));
                output.push_str(&String::from_utf8_lossy(&out.stderr));
                if !out.status.success() && step[0] == "makecache" {
                    ok = false;
                }
            }
            Err(e) => {
                output.push_str(&format!("cannot run {dnf_bin}: {e}\n"));
                ok = false;
            }
        }
    }
    let tail_start = output.len().saturating_sub(OUTPUT_TAIL_CHARS);
    CacheReport {
        ran: true,
        ok,
        output_tail: output[tail_start..].to_string(),
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn plan_mode_defaults_to_the_system_repo_dir() {
        // Only exercise the field defaults; parsing real argv is covered by
        // the integration tests.
        let args = Args {
            repo_dir: PathBuf::from(fmm_core::reposd::DEFAULT_REPO_DIR),
            dnf_bin: DEFAULT_DNF_BIN.to_string(),
        };
        assert_eq!(args.repo_dir, PathBuf::from("/etc/yum.repos.d"));
        assert_eq!(args.dnf_bin, "dnf");
    }

    #[test]
    fn migrate_mode_defaults_to_the_user_and_legacy_roots() {
        let args = MigrateArgs::parse(&[]);
        assert_eq!(args.snapshot_root, fmm_core::reposd::user_snapshot_root());
        assert_eq!(
            args.legacy_root,
            PathBuf::from(fmm_core::reposd::LEGACY_SNAPSHOT_ROOT)
        );
    }

    #[test]
    fn migrate_mode_accepts_explicit_roots() {
        let argv = vec![
            "--snapshot-root".to_string(),
            "/tmp/snaps".to_string(),
            "--legacy-root".to_string(),
            "/tmp/legacy".to_string(),
        ];
        let args = MigrateArgs::parse(&argv);
        assert_eq!(args.snapshot_root, PathBuf::from("/tmp/snaps"));
        assert_eq!(args.legacy_root, PathBuf::from("/tmp/legacy"));
    }

    fn stub_plan() -> ApplyPlan {
        ApplyPlan {
            plan_version: fmm_core::plan::PLAN_VERSION,
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
    fn plan_without_a_snapshot_root_is_rejected() {
        let mut plan = stub_plan();
        assert_eq!(plan_root(&plan).unwrap_err().code, "bad_plan");
        plan.snapshot_root = "/tmp/snaps".to_string();
        assert_eq!(plan_root(&plan).unwrap(), PathBuf::from("/tmp/snaps"));
    }

    #[test]
    #[cfg(unix)]
    fn chown_tree_to_the_current_uid_is_a_no_op_success() {
        use std::os::unix::fs::MetadataExt;
        let dir = std::env::temp_dir().join(format!("fmm-apply-chown-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/f.repo"), "[x]\n").unwrap();
        let uid = std::fs::metadata(&dir).unwrap().uid();
        assert_eq!(invoking_uid(Some(&uid.to_string()), &dir).unwrap(), uid);
        chown_tree(&dir, uid).expect("chown tree");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(unix)]
    fn invoking_uid_falls_back_to_the_snapshot_root_owner() {
        use std::os::unix::fs::MetadataExt;
        let dir = std::env::temp_dir().join(format!("fmm-apply-uid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let uid = std::fs::metadata(&dir).unwrap().uid();
        assert_eq!(invoking_uid(None, &dir).unwrap(), uid);
        assert!(invoking_uid(Some("not-a-number"), &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
