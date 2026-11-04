//! Privileged helper (`fmm-apply`): applies repository plans as root.
//!
//! Invoked via `pkexec /usr/libexec/fmm-apply`. Reads an [`fmm_core::plan::ApplyPlan`]
//! as JSON from stdin, performs the requested action (switch or restore) with
//! an unconditional pre-write snapshot, and writes an
//! [`fmm_core::plan::ApplyReport`] as JSON to stdout. Exit code 0 means the
//! report was produced (`report.ok` says whether the work succeeded); exit 1
//! means the helper itself failed before/while producing a report.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use fmm_core::error::Error;
use fmm_core::plan::{ApplyPlan, ApplyReport, CacheReport, PlanAction};
use fmm_core::snapshot;
use sha2::{Digest, Sha256};

/// Default `dnf` executable (resolved through the helper's PATH).
const DEFAULT_DNF_BIN: &str = "dnf";
/// How many characters of dnf output are kept in the report.
const OUTPUT_TAIL_CHARS: usize = 2000;

fn main() {
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

/// Parsed command-line arguments.
struct Args {
    repo_dir: PathBuf,
    snapshot_root: PathBuf,
    dnf_bin: String,
}

impl Args {
    fn parse() -> Self {
        let mut repo_dir = PathBuf::from(fmm_core::reposd::DEFAULT_REPO_DIR);
        let mut snapshot_root = PathBuf::from(fmm_core::reposd::DEFAULT_SNAPSHOT_ROOT);
        let mut dnf_bin = DEFAULT_DNF_BIN.to_string();
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--repo-dir" => repo_dir = it.next().map(PathBuf::from).unwrap_or(repo_dir),
                "--snapshot-root" => {
                    snapshot_root = it.next().map(PathBuf::from).unwrap_or(snapshot_root)
                }
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
        Self {
            repo_dir,
            snapshot_root,
            dnf_bin,
        }
    }
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

/// Switch action: snapshot → verify hashes → write → prune → (refresh cache).
fn apply_switch(plan: &ApplyPlan, args: &Args) -> Result<ApplyReport, Failure> {
    let snap = snapshot::create_snapshot(&args.repo_dir, &args.snapshot_root, &plan.snapshot_tag)
        .map_err(Failure::from)?;
    let mut report = Report {
        snapshot: Some(snap.to_string_lossy().into_owned()),
        ..Default::default()
    };

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

/// Restore action: snapshot the current state → copy the snapshot back →
/// keep or purge extra files → (refresh cache).
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

    let snap = snapshot::create_snapshot(&args.repo_dir, &args.snapshot_root, &plan.snapshot_tag)
        .map_err(Failure::from)?;
    let mut report = Report {
        snapshot: Some(snap.to_string_lossy().into_owned()),
        ..Default::default()
    };

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
    fn args_default_to_system_paths() {
        // Only exercise the field defaults; parsing real argv is covered by
        // the integration tests.
        let args = Args {
            repo_dir: PathBuf::from(fmm_core::reposd::DEFAULT_REPO_DIR),
            snapshot_root: PathBuf::from(fmm_core::reposd::DEFAULT_SNAPSHOT_ROOT),
            dnf_bin: DEFAULT_DNF_BIN.to_string(),
        };
        assert_eq!(args.repo_dir, PathBuf::from("/etc/yum.repos.d"));
        assert_eq!(args.snapshot_root, PathBuf::from("/etc/yum.repos.d.bak"));
        assert_eq!(args.dnf_bin, "dnf");
    }
}
