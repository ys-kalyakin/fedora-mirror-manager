//! Bulk switch planning: point every switchable repository a mirror carries
//! at that mirror in one multi-file plan — one combined preview, one polkit
//! authorization, one pre-write snapshot.
//!
//! Candidates are grouped by the file the batch writes (several sections of
//! one `.repo` file are merged into a single rewrite; several sections of one
//! packaged file produce a single overriding copy). Per-file failures become
//! per-repository skips with reasons instead of failing the whole batch; only
//! an inventory-level failure aborts the planning.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::{self, Mirror};
use crate::classify::Switchability;
use crate::error::Result;
use crate::plan::{ApplyPlan, PLAN_VERSION, PlanAction, PlanFile, sha256_hex};
use crate::reposd::{self, RepoFile, RepoLocations, inventory};

/// A repository excluded from the bulk switch, with the reason shown to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkSkip {
    /// Repository id the skip is about.
    pub repo_id: String,
    /// Human-readable reason (already on the mirror, not carried, …).
    pub reason: String,
}

/// One rewritten section inside a bulk file preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkSectionPreview {
    /// Repository id whose section is rewritten.
    pub repo_id: String,
    /// The `baseurl` placed under the section header.
    pub new_baseurl: String,
    /// URL lines removed from the section (as they appear now).
    pub removed_lines: Vec<String>,
}

/// One file the bulk plan writes (or creates), for the preview dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkFilePreview {
    /// `.repo` file that will be rewritten or created (base name).
    pub file: String,
    /// Full path of the file that will be created or rewritten.
    pub target_path: String,
    /// The target file does not exist yet and will be created (F45+ overriding copy).
    pub creates_file: bool,
    /// Packaged `.repo` file the new content derives from, when applicable.
    pub source_path: Option<String>,
    /// The sections rewritten in this file.
    pub sections: Vec<BulkSectionPreview>,
}

/// Combined preview of a bulk switch, shown in a single confirmation dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkPreview {
    /// The mirror the batch switches to.
    pub mirror: Mirror,
    /// Files the batch writes (or creates), one entry per target file.
    pub files: Vec<BulkFilePreview>,
    /// Switchable repositories left out of the batch, with reasons.
    pub skipped: Vec<BulkSkip>,
}

/// Result of planning a bulk switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum BulkOutcome {
    /// Nothing to do: no eligible repository produced a change.
    NoChange {
        /// Human-readable summary ("nothing to switch to …").
        reason: String,
        /// Why each switchable repository was left out.
        skipped: Vec<BulkSkip>,
    },
    /// A batch plan is ready: combined preview for the user, files for the helper.
    Plan {
        preview: BulkPreview,
        plan: ApplyPlan,
    },
}

/// One candidate repository with the `baseurl` it would receive.
struct Candidate<'a> {
    entry: &'a reposd::RepoEntry,
    baseurl: String,
}

/// All candidates targeting one file, merged into a single rewrite.
struct FileBatch<'a> {
    /// File the helper writes (or creates).
    target: PathBuf,
    /// Packaged file the new content derives from (created overriding copies).
    packaged_source: Option<PathBuf>,
    /// The target does not exist yet and is created by the helper.
    creates_file: bool,
    /// `(repo id, baseurl)` per included section.
    sections: Vec<(String, String)>,
    /// Candidates behind `sections`, for skip reporting on batch failure.
    entries: Vec<Candidate<'a>>,
}

/// Plans the bulk switch of every eligible repository to `mirror`.
///
/// A repository is eligible when it is classified switchable, the mirror
/// carries its family (`catalog::baseurl` yields a URL), and it is not
/// already pointing at the mirror. Everything else is reported as skipped.
/// The returned plan writes each affected file once with all its included
/// sections rewritten; the helper's per-file hash guard then covers the whole
/// batch (one stale file aborts the batch before the first write).
pub fn build_bulk_plan(locations: &RepoLocations, mirror: Mirror) -> Result<BulkOutcome> {
    let inv = inventory(locations)?;

    let mut skipped: Vec<BulkSkip> = Vec::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    for entry in &inv.entries {
        if entry.switchability != Switchability::Switchable {
            // Cisco and not-switchable repositories are outside a mirror bulk
            // switch and are not reported as skips (the table explains them).
            continue;
        }
        let Some(baseurl) = catalog::baseurl(mirror, entry.kind) else {
            skipped.push(not_carried_skip(entry, mirror));
            continue;
        };
        if entry.source == mirror.source() {
            skipped.push(BulkSkip {
                repo_id: entry.id.clone(),
                reason: format!("already on the {} mirror", mirror.label()),
            });
            continue;
        }
        candidates.push(Candidate { entry, baseurl });
    }

    if candidates.is_empty() {
        return Ok(BulkOutcome::NoChange {
            reason: format!("nothing to switch to the {} mirror", mirror.label()),
            skipped,
        });
    }

    let mut batches: BTreeMap<PathBuf, FileBatch> = BTreeMap::new();
    for candidate in candidates {
        let owner = Path::new(&candidate.entry.path);
        let (target, packaged_source, creates_file) = if owner.starts_with(&locations.local_dir) {
            // The owning file is local: rewrite it in place.
            (owner.to_path_buf(), None, false)
        } else {
            // The owning file is packaged: the batch creates the overriding
            // copy in the local directory (never the packaged location).
            let dest = locations.local_dir.join(&candidate.entry.file);
            if dest.exists() {
                // Same guard as the single switch: an overriding file with
                // this name exists but does not define this repository.
                skipped.push(BulkSkip {
                    repo_id: candidate.entry.id.clone(),
                    reason: format!(
                        "an overriding file named {} already exists in the local repository directory but does not define this repository",
                        candidate.entry.file
                    ),
                });
                continue;
            }
            (dest, Some(owner.to_path_buf()), true)
        };
        let batch = batches.entry(target.clone()).or_insert_with(|| FileBatch {
            sections: Vec::new(),
            entries: Vec::new(),
            creates_file,
            packaged_source,
            target,
        });
        batch
            .sections
            .push((candidate.entry.id.clone(), candidate.baseurl.clone()));
        batch.entries.push(candidate);
    }

    let mut files: Vec<PlanFile> = Vec::new();
    let mut file_previews: Vec<BulkFilePreview> = Vec::new();
    for batch in batches.into_values() {
        build_batch(
            locations,
            mirror,
            batch,
            &mut skipped,
            &mut files,
            &mut file_previews,
        );
    }

    if files.is_empty() {
        return Ok(BulkOutcome::NoChange {
            reason: format!("nothing to switch to the {} mirror", mirror.label()),
            skipped,
        });
    }

    let plan = ApplyPlan {
        plan_version: PLAN_VERSION,
        action: PlanAction::Switch,
        snapshot_tag: format!("switch-all-{}", mirror.slug()),
        refresh_cache: false,
        purge: false,
        snapshot: None,
        files,
    };
    Ok(BulkOutcome::Plan {
        preview: BulkPreview {
            mirror,
            files: file_previews,
            skipped,
        },
        plan,
    })
}

/// Builds one batch's plan entry: loads the source content, merges every
/// included section's rewrite, compares with the current content, and appends
/// the `PlanFile` plus its preview — or turns the whole batch into skips.
fn build_batch(
    locations: &RepoLocations,
    mirror: Mirror,
    batch: FileBatch,
    skipped: &mut Vec<BulkSkip>,
    files: &mut Vec<PlanFile>,
    file_previews: &mut Vec<BulkFilePreview>,
) {
    // Source content to rewrite: the packaged file for created overriding
    // copies; for local files the packaged sibling when one shadows it
    // (rebuild rule), else the file's own content.
    let loaded: Result<(RepoFile, Option<String>)> = match &batch.packaged_source {
        Some(packaged) => {
            RepoFile::load(packaged).map(|f| (f, Some(packaged.to_string_lossy().into_owned())))
        }
        None => {
            let sibling = locations
                .packaged_dir
                .as_ref()
                .map(|packaged| {
                    packaged.join(batch.target.file_name().expect("target has a file name"))
                })
                .filter(|candidate| {
                    candidate.as_path() != batch.target.as_path() && candidate.is_file()
                });
            match sibling {
                Some(packaged) => match RepoFile::load(&packaged) {
                    Ok(f) => Ok((f, Some(packaged.to_string_lossy().into_owned()))),
                    // The packaged source vanished or is unreadable: fall
                    // back to the shadow's own content (single-switch rule).
                    Err(_) => RepoFile::load(&batch.target).map(|f| (f, None)),
                },
                None => RepoFile::load(&batch.target).map(|f| (f, None)),
            }
        }
    };
    let (source, source_path) = match loaded {
        Ok(ok) => ok,
        Err(err) => {
            for candidate in &batch.entries {
                skipped.push(BulkSkip {
                    repo_id: candidate.entry.id.clone(),
                    reason: format!("cannot read the repository's source file: {err}"),
                });
            }
            return;
        }
    };

    // URL lines removed per section, computed on the pristine source;
    // sections the loaded source does not define become skips.
    let mut sections: Vec<(String, String, Vec<String>)> = Vec::new();
    for (repo_id, baseurl) in &batch.sections {
        match source.section(repo_id) {
            Some(sec) => {
                let removed: Vec<String> = source.lines()[sec.header_line..sec.end_line]
                    .iter()
                    .filter(|line| reposd::is_url_line(line))
                    .cloned()
                    .collect();
                sections.push((repo_id.clone(), baseurl.clone(), removed));
            }
            None => skipped.push(BulkSkip {
                repo_id: repo_id.clone(),
                reason: format!(
                    "the source file the switch derives from does not define this repository: {}",
                    source_path.as_deref().unwrap_or("local file")
                ),
            }),
        }
    }
    if sections.is_empty() {
        return;
    }

    let mut file = source.clone();
    for (repo_id, baseurl, _) in &sections {
        file.rewrite_section(repo_id, Some(baseurl), None);
    }
    let new_content = file.render();

    let current = match std::fs::read(&batch.target) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(content) => Some(content),
            Err(_) => {
                for candidate in &batch.entries {
                    skipped.push(BulkSkip {
                        repo_id: candidate.entry.id.clone(),
                        reason: format!(
                            "cannot read {}: the file is not valid UTF-8",
                            batch.target.display()
                        ),
                    });
                }
                return;
            }
        },
        // Absent is expected exactly for created overriding copies.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            for candidate in &batch.entries {
                skipped.push(BulkSkip {
                    repo_id: candidate.entry.id.clone(),
                    reason: format!("cannot read {}", batch.target.display()),
                });
            }
            return;
        }
    };

    let before_sha256 = match current {
        Some(content) => {
            if batch.creates_file {
                // The overriding copy appeared after the candidate check:
                // treat it as the same conflict the single switch reports.
                for candidate in &batch.entries {
                    skipped.push(BulkSkip {
                        repo_id: candidate.entry.id.clone(),
                        reason: format!(
                            "an overriding file named {} already exists in the local repository directory but does not define this repository",
                            candidate.entry.file
                        ),
                    });
                }
                return;
            }
            if content == new_content {
                // Nothing the batch planned actually differs on disk.
                for candidate in &batch.entries {
                    skipped.push(BulkSkip {
                        repo_id: candidate.entry.id.clone(),
                        reason: format!("already on the {} mirror", mirror.label()),
                    });
                }
                return;
            }
            sha256_hex(content.as_bytes())
        }
        None => String::new(), // sentinel: the helper creates the file
    };

    let target_path = batch.target.to_string_lossy().into_owned();
    let file_name = batch
        .target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .into_owned();
    let preview_sections = sections
        .into_iter()
        .map(|(repo_id, new_baseurl, removed_lines)| BulkSectionPreview {
            repo_id,
            new_baseurl,
            removed_lines,
        })
        .collect();
    files.push(PlanFile {
        path: target_path.clone(),
        before_sha256,
        content: new_content,
    });
    file_previews.push(BulkFilePreview {
        file: file_name,
        target_path,
        creates_file: batch.creates_file,
        source_path,
        sections: preview_sections,
    });
}

/// Skip reason for a switchable repository the mirror does not carry.
fn not_carried_skip(entry: &reposd::RepoEntry, mirror: Mirror) -> BulkSkip {
    let reason = if mirror == Mirror::Hyperdedic && catalog::is_rpmfusion(entry.kind) {
        // Same wording as the per-row button tooltip.
        "HyperDedic does not mirror RPM Fusion repositories".to_string()
    } else {
        format!("{} does not carry this repository family", mirror.label())
    };
    BulkSkip {
        repo_id: entry.id.clone(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_outcome_serialization_round_trip() {
        let outcome = BulkOutcome::NoChange {
            reason: "nothing to switch to the Yandex mirror".to_string(),
            skipped: vec![BulkSkip {
                repo_id: "updates".to_string(),
                reason: "already on the Yandex mirror".to_string(),
            }],
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains(r#""outcome":"no-change""#));
        let back: BulkOutcome = serde_json::from_str(&json).unwrap();
        match back {
            BulkOutcome::NoChange { reason, skipped } => {
                assert_eq!(reason, "nothing to switch to the Yandex mirror");
                assert_eq!(skipped[0].repo_id, "updates");
            }
            BulkOutcome::Plan { .. } => panic!("wrong variant"),
        }
    }

    #[test]
    fn bulk_preview_serialization_round_trip() {
        let preview = BulkPreview {
            mirror: Mirror::Hyperdedic,
            files: vec![BulkFilePreview {
                file: "fedora.repo".to_string(),
                target_path: "/etc/yum.repos.d/fedora.repo".to_string(),
                creates_file: true,
                source_path: Some("/usr/share/dnf5/repos.d/fedora.repo".to_string()),
                sections: vec![BulkSectionPreview {
                    repo_id: "fedora".to_string(),
                    new_baseurl: "https://mirror.hyperdedic.ru/fedora/linux/releases/$releasever/Everything/$basearch/os/".to_string(),
                    removed_lines: vec!["metalink=https://example".to_string()],
                }],
            }],
            skipped: vec![BulkSkip {
                repo_id: "rpmfusion-free".to_string(),
                reason: "HyperDedic does not mirror RPM Fusion repositories".to_string(),
            }],
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains(r#""mirror":"hyperdedic""#));
        assert!(json.contains(r#""creates_file":true"#));
        let back: BulkPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(back.files.len(), 1);
        assert_eq!(back.files[0].sections[0].repo_id, "fedora");
        assert_eq!(back.skipped[0].repo_id, "rpmfusion-free");
    }
}
