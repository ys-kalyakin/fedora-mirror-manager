//! Snapshots of `/etc/yum.repos.d` under `/etc/yum.repos.d.bak`: naming,
//! listing, pruning and restore planning. The filesystem work lives here so
//! the `fmm-apply` helper and tests share one implementation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::plan::{ApplyPlan, PlanAction, PLAN_VERSION};

/// Number of snapshots kept in the snapshot root (reference default).
pub const KEEP_SNAPSHOTS: usize = 10;

/// One snapshot directory, as shown in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotInfo {
    /// Directory name (unique id), e.g. `20261104T120000Z-before-restore`.
    pub id: String,
    /// UTC timestamp part, e.g. `20261104T120000Z`.
    pub timestamp: String,
    /// Tag part, e.g. `before-restore` (may carry a collision suffix).
    pub tag: String,
}

/// Formats an epoch timestamp as UTC `YYYYMMDDThhmmssZ`.
pub fn format_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}T{h:02}{min:02}{s:02}Z",
        h = secs_of_day / 3_600,
        min = (secs_of_day % 3_600) / 60,
        s = secs_of_day % 60
    )
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 → (y, m, d).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Lists the snapshots in `snapshot_root`, newest first. A missing root is an
/// empty list, matching the reference `list` command.
pub fn list_snapshots(snapshot_root: &Path) -> Result<Vec<SnapshotInfo>> {
    let mut names = snapshot_names(snapshot_root)?;
    names.sort_unstable_by(|a, b| b.cmp(a)); // newest first (LC_ALL=C sort -r)
    Ok(names
        .into_iter()
        .map(|id| {
            let (timestamp, tag) = match id.split_once('-') {
                Some((ts, tag)) => (ts.to_string(), tag.to_string()),
                None => (id.clone(), String::new()),
            };
            SnapshotInfo { id, timestamp, tag }
        })
        .collect())
}

/// Snapshot directory names, filtered like the reference (`-name '20*'`).
fn snapshot_names(snapshot_root: &Path) -> Result<Vec<String>> {
    if !snapshot_root.is_dir() {
        return Ok(Vec::new());
    }
    let read = fs::read_dir(snapshot_root).map_err(|source| Error::Io {
        path: snapshot_root.to_path_buf(),
        source,
    })?;
    let mut names = Vec::new();
    for entry in read {
        let entry = entry.map_err(|source| Error::Io {
            path: snapshot_root.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("20") && entry.path().is_dir() {
            names.push(name);
        }
    }
    Ok(names)
}

/// Snapshot names to delete so that at most `keep` remain (newest first).
pub fn plan_prune(snapshot_root: &Path, keep: usize) -> Result<Vec<String>> {
    let mut names = snapshot_names(snapshot_root)?;
    names.sort_unstable_by(|a, b| b.cmp(a));
    Ok(names.into_iter().skip(keep).collect())
}

/// Creates a snapshot of `repo_dir` under `snapshot_root` tagged `tag` and
/// prunes old snapshots, keeping [`KEEP_SNAPSHOTS`]. Returns the snapshot path.
pub fn create_snapshot(repo_dir: &Path, snapshot_root: &Path, tag: &str) -> Result<PathBuf> {
    if !repo_dir.is_dir() {
        return Err(Error::Snapshot(format!("repository directory {} not found", repo_dir.display())));
    }
    fs::create_dir_all(snapshot_root).map_err(|source| Error::Io {
        path: snapshot_root.to_path_buf(),
        source,
    })?;

    // Collision suffix keeps the chronological sort stable (reference rule).
    let ts = format_utc(now_epoch());
    let mut dir = snapshot_root.join(format!("{ts}-{tag}"));
    let mut n = 0u32;
    while dir.exists() {
        n += 1;
        dir = snapshot_root.join(format!("{ts}-{tag}-{n:03}"));
    }
    fs::create_dir(&dir).map_err(|source| Error::Io { path: dir.clone(), source })?;

    copy_tree(repo_dir, &dir)?;

    for name in plan_prune(snapshot_root, KEEP_SNAPSHOTS)? {
        let target = snapshot_root.join(name);
        // Guard from the reference: never remove anything outside the root.
        if target.starts_with(snapshot_root) && !target.file_name().is_some_and(|f| f.to_string_lossy().contains("..")) {
            let _ = fs::remove_dir_all(&target);
        }
    }
    Ok(dir)
}

/// Copies the contents of `src` into `dst` recursively, preserving permission
/// bits (`fs::copy` semantics, like the reference `cp -a` for these files).
pub fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    let read = fs::read_dir(src).map_err(|source| Error::Io { path: src.to_path_buf(), source })?;
    for entry in read {
        let entry = entry.map_err(|source| Error::Io { path: src.to_path_buf(), source })?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type().map_err(|source| Error::Io { path: from.clone(), source })?;
        if file_type.is_dir() {
            fs::create_dir(&to).map_err(|source| Error::Io { path: to.clone(), source })?;
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|source| Error::Io { path: from.clone(), source })?;
        }
    }
    Ok(())
}

/// `*.repo` files present in `repo_dir` but absent from `snapshot_dir`.
pub fn extra_repo_files(repo_dir: &Path, snapshot_dir: &Path) -> Result<Vec<String>> {
    let mut extras = Vec::new();
    let read = fs::read_dir(repo_dir).map_err(|source| Error::Io { path: repo_dir.to_path_buf(), source })?;
    for entry in read {
        let entry = entry.map_err(|source| Error::Io { path: repo_dir.to_path_buf(), source })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".repo") && !snapshot_dir.join(&name).exists() {
            extras.push(name);
        }
    }
    extras.sort();
    Ok(extras)
}

/// Validates a snapshot id (no separators, no traversal, exists).
fn validated_snapshot_dir(snapshot_root: &Path, snapshot_id: &str) -> Result<PathBuf> {
    if snapshot_id.is_empty() || snapshot_id.contains('/') || snapshot_id.contains('\\') || snapshot_id.contains("..") {
        return Err(Error::NotFound(format!("invalid snapshot id: {snapshot_id:?}")));
    }
    let dir = snapshot_root.join(snapshot_id);
    if !dir.is_dir() {
        return Err(Error::NotFound(format!("snapshot not found: {snapshot_id}")));
    }
    Ok(dir)
}

/// `*.repo` files that would be kept (or purged) because they are not part of
/// the chosen snapshot. Shown in the restore dialog.
pub fn restore_preview(repo_dir: &Path, snapshot_root: &Path, snapshot_id: &str) -> Result<Vec<String>> {
    let dir = validated_snapshot_dir(snapshot_root, snapshot_id)?;
    extra_repo_files(repo_dir, &dir)
}

/// Builds the restore plan for `snapshot_id`. The helper snapshots the current
/// state first (tag `before-restore`), copies the snapshot contents back and
/// optionally purges later files; no file content travels in the plan.
pub fn build_restore_plan(
    _repo_dir: &Path,
    snapshot_root: &Path,
    snapshot_id: &str,
    purge: bool,
) -> Result<ApplyPlan> {
    let dir = validated_snapshot_dir(snapshot_root, snapshot_id)?;
    Ok(ApplyPlan {
        plan_version: PLAN_VERSION,
        action: PlanAction::Restore,
        snapshot_tag: "before-restore".to_string(),
        refresh_cache: false,
        purge,
        snapshot: Some(dir.to_string_lossy().into_owned()),
        files: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_utc_matches_known_timestamps() {
        assert_eq!(format_utc(0), "19700101T000000Z");
        assert_eq!(format_utc(86_400), "19700102T000000Z");
        assert_eq!(format_utc(1_700_000_000), "20231114T221320Z");
        assert_eq!(format_utc(1_767_225_600), "20260101T000000Z");
        // Leap-year day: 2024-02-29T12:00:00Z = 1709208000
        assert_eq!(format_utc(1_709_208_000), "20240229T120000Z");
    }

    #[test]
    fn rejects_invalid_snapshot_ids() {
        let root = Path::new("/tmp");
        assert!(validated_snapshot_dir(root, "").is_err());
        assert!(validated_snapshot_dir(root, "a/b").is_err());
        assert!(validated_snapshot_dir(root, "..").is_err());
        assert!(validated_snapshot_dir(root, "no-such-snapshot").is_err());
    }
}
