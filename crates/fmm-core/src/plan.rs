//! Apply plans: exact privileged changes computed unprivileged, verified and
//! executed by the `fmm-apply` helper. The plan carries the full new file
//! content plus the hash of the current content so the helper can abort on a
//! stale plan (the file changed between preview and apply).

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::{self, Mirror};
use crate::classify::Switchability;
use crate::error::{Error, Result};
use crate::reposd::{self, RepoLocations, UrlKind, classify_source, inventory};

/// Version of the plan protocol between the app and the helper.
pub const PLAN_VERSION: u32 = 1;

/// A single file rewrite: the full new content plus the expected sha256 of
/// the current content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFile {
    /// Absolute path of the file to rewrite (or create).
    pub path: String,
    /// Hash the on-disk content must still have when the helper runs. The
    /// empty string is a sentinel: the file must not exist and is created
    /// (the first switch on the F45+ layout creates an overriding copy).
    pub before_sha256: String,
    /// Full new file content.
    pub content: String,
}

/// Action kind of a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanAction {
    /// Rewrite the files listed in the plan.
    Switch,
    /// Copy a snapshot directory back over the repo directory.
    Restore,
}

/// Plan executed by the privileged helper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPlan {
    /// Protocol version, see [`PLAN_VERSION`].
    pub plan_version: u32,
    /// What the helper should do.
    pub action: PlanAction,
    /// Tag for the automatic pre-write snapshot, e.g. `switch-updates-yandex`.
    pub snapshot_tag: String,
    /// Run `dnf clean all` + `dnf makecache` after a successful write.
    pub refresh_cache: bool,
    /// Restore only: delete `*.repo` files absent from the snapshot.
    #[serde(default)]
    pub purge: bool,
    /// Restore only: snapshot directory to restore from.
    #[serde(default)]
    pub snapshot: Option<String>,
    /// Switch only: files to rewrite (empty for restore).
    #[serde(default)]
    pub files: Vec<PlanFile>,
}

/// Result of the helper's cache-refresh step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheReport {
    /// The step ran (as opposed to being disabled).
    pub ran: bool,
    /// `dnf makecache` exited successfully.
    pub ok: bool,
    /// Tail of the combined dnf output for display.
    pub output_tail: String,
}

/// Report returned by the helper on stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyReport {
    /// All requested changes were applied.
    pub ok: bool,
    /// Machine-readable failure code (`stale_plan`, `snapshot_failed`, …).
    #[serde(default)]
    pub code: Option<String>,
    /// Human-readable failure description.
    #[serde(default)]
    pub message: Option<String>,
    /// Snapshot created before the write.
    #[serde(default)]
    pub snapshot: Option<String>,
    /// Files written or copied.
    #[serde(default)]
    pub changed: Vec<String>,
    /// Files already in the requested state and skipped.
    #[serde(default)]
    pub unchanged: Vec<String>,
    /// Restore: `*.repo` files kept that were not in the snapshot.
    #[serde(default)]
    pub extra_files: Vec<String>,
    /// Cache-refresh outcome, when the step was requested.
    #[serde(default)]
    pub cache: Option<CacheReport>,
}

/// A switching action the user picked for one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RepoAction {
    /// Switch a non-cisco repository to a mirror.
    Switch { mirror: Mirror },
    /// Cisco OpenH264 mode change.
    Cisco { mode: CiscoMode },
}

/// Cisco OpenH264 repository mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CiscoMode {
    /// `enabled=0`.
    Disable,
    /// Point at the official codecs mirror and enable.
    Codecs,
    /// Touch nothing.
    Keep,
}

/// Human-facing summary of a planned change, shown before authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPreview {
    /// `.repo` file that will be rewritten (base name).
    pub file: String,
    /// Full path of the file that will be created or rewritten.
    pub target_path: String,
    /// The `baseurl` that will be placed under the section header.
    pub new_baseurl: Option<String>,
    /// The `enabled` value that will be set, if any.
    pub set_enabled: Option<String>,
    /// Lines that will be removed from the section (as they appear now).
    pub removed_lines: Vec<String>,
    /// The target file does not exist yet and will be created by the helper
    /// (the first switch on the F45+ layout creates the overriding copy).
    pub creates_file: bool,
    /// Packaged `.repo` file the new content derives from, when the section
    /// is owned by a packaged file (None for plain local/legacy rewrites).
    pub source_path: Option<String>,
}

/// Result of planning one repository action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
// Transient value: built, serialized to the UI, dropped. The Plan variant
// carries both the preview and the full plan by design, so the size-difference
// lint is deliberately tolerated instead of boxing for its own sake.
#[allow(clippy::large_enum_variant)]
pub enum PlanOutcome {
    /// Nothing to do: the requested state is already in place.
    NoChange { reason: String },
    /// A plan is ready: preview for the user, files for the helper.
    Plan {
        preview: PlanPreview,
        plan: ApplyPlan,
    },
}

/// Builds the plan for one repository action. Returns [`PlanOutcome::NoChange`]
/// when the requested state is already in place.
///
/// The plan always targets the file that effectively defines the section
/// (dnf5 precedence: the local `/etc/yum.repos.d` definition wins over the
/// packaged one). On the F45+ layout the packaged location is never written:
/// a switch whose section is owned by a packaged file builds an overriding
/// copy in the local directory instead — created when absent (empty
/// `before_sha256` sentinel), rebuilt from the current packaged file when it
/// already exists.
pub fn build_action_plan(
    locations: &RepoLocations,
    repo_id: &str,
    action: &RepoAction,
) -> Result<PlanOutcome> {
    let inv = inventory(locations)?;
    let matches: Vec<&reposd::RepoEntry> = inv.entries.iter().filter(|e| e.id == repo_id).collect();
    match matches.len() {
        0 => return Err(Error::NotFound(format!("repository id {repo_id}"))),
        1 => {}
        _ => {
            return Err(Error::Parse {
                file: locations.local_dir.join(repo_id),
                reason: "repository id is defined in more than one file".to_string(),
            });
        }
    }
    let entry = matches[0];
    // Cisco repositories take only their dedicated modes; switchable
    // repositories take only mirror switches; everything else is rejected.
    match (entry.switchability, action) {
        (Switchability::Cisco, RepoAction::Cisco { .. })
        | (Switchability::Switchable, RepoAction::Switch { .. }) => {}
        (Switchability::Cisco, RepoAction::Switch { .. }) => {
            return Err(Error::NotSwitchable {
                id: repo_id.to_string(),
                reason: "cisco repositories only support disable/codecs modes".to_string(),
            });
        }
        _ => {
            return Err(Error::NotSwitchable {
                id: repo_id.to_string(),
                reason: "repository is not switchable".to_string(),
            });
        }
    }

    let (baseurl, set_enabled, snapshot_tag) = match action {
        RepoAction::Switch { mirror } => {
            let Some(url) = catalog::baseurl(*mirror, entry.kind) else {
                return Err(Error::NotSwitchable {
                    id: repo_id.to_string(),
                    reason: format!("{} does not carry this repository family", mirror.label()),
                });
            };
            (
                Some(url),
                None,
                format!("switch-{}-{}", repo_id, mirror.slug()),
            )
        }
        RepoAction::Cisco { mode } => match mode {
            CiscoMode::Disable => (
                None,
                Some("0".to_string()),
                format!("cisco-disable-{repo_id}"),
            ),
            CiscoMode::Codecs => (
                Some(catalog::CISCO_CODECS_BASEURL.to_string()),
                Some("1".to_string()),
                format!("cisco-codecs-{repo_id}"),
            ),
            CiscoMode::Keep => {
                return Ok(PlanOutcome::NoChange {
                    reason: "the 'keep' mode changes nothing".to_string(),
                });
            }
        },
    };

    let owner = Path::new(&entry.path);
    let (dest_path, before_sha256, new_content, removed_lines, creates_file, source_path) = if owner
        .starts_with(&locations.local_dir)
    {
        // The owning file is in the local directory: rewrite it in place.
        let before_content = fs_content(owner)?;
        // F45+: when the local file shadows a packaged file of the same name,
        // rebuild the content from the current packaged file so unrelated
        // package updates are not lost; otherwise rewrite its own content.
        let packaged_sibling = locations
            .packaged_dir
            .as_ref()
            .map(|packaged| packaged.join(&entry.file))
            .filter(|candidate| candidate.as_path() != owner && candidate.is_file());
        let (source, rebuilt, packaged_source) = match packaged_sibling {
            Some(packaged_path) => match reposd::RepoFile::load(&packaged_path) {
                Ok(repo_file) => {
                    let source_path = packaged_path.to_string_lossy().into_owned();
                    (repo_file, true, Some(source_path))
                }
                // The packaged source vanished or is unreadable: fall back to
                // the shadow's own content (rewrite in place).
                Err(_) => (reposd::RepoFile::load(owner)?, false, None),
            },
            None => (reposd::RepoFile::load(owner)?, false, None),
        };
        let (removed_lines, new_content) =
            rewrite(&source, repo_id, baseurl.as_deref(), set_enabled.as_deref())?;
        if new_content == before_content {
            return Ok(PlanOutcome::NoChange {
                reason: no_change_reason(action),
            });
        }
        // A rebuilt shadow whose section already carries the requested mirror
        // is a no-op even when the packaged source drifted: re-picking the
        // current mirror must not write anything.
        if rebuilt
            && matches!(action, RepoAction::Switch { .. })
            && classify_source(
                baseurl
                    .as_ref()
                    .map(|url| (UrlKind::BaseUrl, url.clone()))
                    .as_ref(),
            ) == entry.source
        {
            return Ok(PlanOutcome::NoChange {
                reason: no_change_reason(action),
            });
        }
        (
            owner.to_path_buf(),
            sha256_hex(before_content.as_bytes()),
            new_content,
            removed_lines,
            false,
            packaged_source,
        )
    } else {
        // The owning file is packaged: write the overriding copy into the
        // local directory (never the packaged location).
        let dest = locations.local_dir.join(&entry.file);
        if dest.exists() {
            return Err(Error::NotSwitchable {
                    id: repo_id.to_string(),
                    reason: "an overriding file with this name already exists in the local repository directory but does not define this repository".to_string(),
                });
        }
        let source = reposd::RepoFile::load(owner)?;
        let (removed_lines, new_content) =
            rewrite(&source, repo_id, baseurl.as_deref(), set_enabled.as_deref())?;
        if new_content == source.render() {
            return Ok(PlanOutcome::NoChange {
                reason: no_change_reason(action),
            });
        }
        let source_path = owner.to_string_lossy().into_owned();
        (
            dest,
            String::new(),
            new_content,
            removed_lines,
            true,
            Some(source_path),
        )
    };

    let preview = PlanPreview {
        file: entry.file.clone(),
        target_path: dest_path.to_string_lossy().into_owned(),
        new_baseurl: baseurl.clone(),
        set_enabled: set_enabled.clone(),
        removed_lines,
        creates_file,
        source_path,
    };
    let plan = ApplyPlan {
        plan_version: PLAN_VERSION,
        action: PlanAction::Switch,
        snapshot_tag,
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files: vec![PlanFile {
            path: dest_path.to_string_lossy().into_owned(),
            before_sha256,
            content: new_content,
        }],
    };
    Ok(PlanOutcome::Plan { preview, plan })
}

/// Applies the section rewrite to a parsed file and collects the lines the
/// rewrite removes (as they appear in the source content).
fn rewrite(
    source: &reposd::RepoFile,
    repo_id: &str,
    baseurl: Option<&str>,
    set_enabled: Option<&str>,
) -> Result<(Vec<String>, String)> {
    let (header_line, end_line) = {
        let sec = source
            .section(repo_id)
            .ok_or_else(|| Error::NotFound(format!("repository id {repo_id}")))?;
        (sec.header_line, sec.end_line)
    };
    let removed_lines: Vec<String> = source.lines()[header_line..end_line]
        .iter()
        .filter(|line| {
            (baseurl.is_some() && reposd::is_url_line(line))
                || (set_enabled.is_some() && reposd::is_enabled_line(line))
        })
        .cloned()
        .collect();
    let mut file = source.clone();
    file.rewrite_section(repo_id, baseurl, set_enabled);
    Ok((removed_lines, file.render()))
}

fn fs_content(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    String::from_utf8(bytes).map_err(|_| Error::Parse {
        file: path.to_path_buf(),
        reason: "file is not valid UTF-8".to_string(),
    })
}

fn no_change_reason(action: &RepoAction) -> String {
    match action {
        RepoAction::Switch { mirror } => format!("already on the {} mirror", mirror.label()),
        RepoAction::Cisco { .. } => "already in the requested state".to_string(),
    }
}

/// Lowercase hex sha256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
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
    fn sha256_hex_matches_known_vectors() {
        // echo -n "abc" | sha256sum
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn plan_serialization_round_trip() {
        let plan = ApplyPlan {
            plan_version: PLAN_VERSION,
            action: PlanAction::Switch,
            snapshot_tag: "switch-updates-yandex".to_string(),
            refresh_cache: true,
            purge: false,
            snapshot: None,
            files: vec![PlanFile {
                path: "/etc/yum.repos.d/fedora-updates.repo".to_string(),
                before_sha256: "abc".to_string(),
                content: "content".to_string(),
            }],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let back: ApplyPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back.plan_version, 1);
        assert_eq!(back.action, PlanAction::Switch);
        assert_eq!(back.snapshot_tag, "switch-updates-yandex");
        assert!(back.refresh_cache);
        assert!(!back.purge);
        assert_eq!(back.files.len(), 1);
        assert_eq!(back.files[0].path, "/etc/yum.repos.d/fedora-updates.repo");
    }

    #[test]
    fn preview_serialization_round_trip() {
        let preview = PlanPreview {
            file: "fedora.repo".to_string(),
            target_path: "/etc/yum.repos.d/fedora.repo".to_string(),
            new_baseurl: Some(
                "https://mirror.yandex.ru/fedora/linux/$releasever/Everything/$basearch/"
                    .to_string(),
            ),
            set_enabled: None,
            removed_lines: vec!["metalink=https://example".to_string()],
            creates_file: true,
            source_path: Some("/usr/share/dnf5/repos.d/fedora.repo".to_string()),
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains(r#""creates_file":true"#));
        assert!(json.contains(r#""source_path":"/usr/share/dnf5/repos.d/fedora.repo""#));
        let back: PlanPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(back.file, "fedora.repo");
        assert!(back.creates_file);
        assert_eq!(
            back.source_path.as_deref(),
            Some("/usr/share/dnf5/repos.d/fedora.repo")
        );
    }

    #[test]
    fn action_serialization_is_tagged() {
        let json = serde_json::to_string(&RepoAction::Switch {
            mirror: Mirror::Yandex,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"switch","mirror":"yandex"}"#);
        let json = serde_json::to_string(&RepoAction::Cisco {
            mode: CiscoMode::Codecs,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"cisco","mode":"codecs"}"#);
    }
}
