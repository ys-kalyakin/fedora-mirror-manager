//! Switch-plan building over the fixture repo directory (tasks 2.1-2.2).

mod common;

use fmm_core::catalog::{CISCO_CODECS_BASEURL, Mirror, baseurl};
use fmm_core::error::Error;
use fmm_core::plan::{CiscoMode, PlanAction, PlanOutcome, RepoAction, build_action_plan};
use fmm_core::reposd::{RepoFile, RepoLocations};
use std::path::Path;

#[test]
fn switch_plan_carries_expected_baseurl_and_preview() {
    let dir = common::fixture_repo_dir();
    let outcome = build_action_plan(
        &RepoLocations::legacy(&dir),
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan, got NoChange");
    };

    // The planned baseurl equals the reference mapping for the family.
    let expected = baseurl(Mirror::Yandex, fmm_core::classify::RepoKey::Updates).unwrap();
    assert_eq!(preview.new_baseurl.as_deref(), Some(expected.as_str()));
    assert_eq!(preview.set_enabled, None);
    // The commented baseurl and the metalink line are removed.
    assert!(
        preview
            .removed_lines
            .iter()
            .any(|l| l.contains("metalink="))
    );
    assert!(
        preview
            .removed_lines
            .iter()
            .any(|l| l.contains("#baseurl="))
    );
    // The preview names the effective target file.
    assert_eq!(
        preview.target_path,
        dir.join("fedora-updates.repo").to_string_lossy()
    );
    // Legacy rewrite: an existing file, no packaged source involved.
    assert!(!preview.creates_file);
    assert_eq!(preview.source_path, None);

    assert_eq!(plan.action, PlanAction::Switch);
    assert_eq!(plan.snapshot_tag, "switch-updates-yandex");
    assert_eq!(plan.files.len(), 1);
    assert!(plan.files[0].path.ends_with("fedora-updates.repo"));
    // Fresh file: before hash must match the current content on disk.
    let current = std::fs::read_to_string(&plan.files[0].path).unwrap();
    assert_eq!(
        plan.files[0].before_sha256,
        fmm_core::plan::sha256_hex(current.as_bytes())
    );
    // New content places the baseurl under the header.
    assert!(plan.files[0].content.contains("[updates]\nbaseurl=https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/"));
}

#[test]
fn switch_touches_only_the_target_section() {
    let dir = common::fixture_repo_dir();
    let outcome = build_action_plan(
        &RepoLocations::legacy(&dir),
        "fedora",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else {
        panic!("expected a plan")
    };
    let content = &plan.files[0].content;

    // Sibling section is byte-identical.
    let original = std::fs::read_to_string(&plan.files[0].path).unwrap();
    fn sibling(text: &str) -> &str {
        text.split_once("[fedora-debuginfo]").unwrap().1
    }
    assert_eq!(sibling(&original), sibling(content));

    // Signing configuration survives.
    assert!(content.contains("gpgcheck=1"));
    assert!(
        content.contains("gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-$releasever-$basearch")
    );
    assert!(content.contains("enabled=1"));
    // Exactly one baseurl line in the fedora section, no metalink left there.
    let fedora_block = content
        .split_once("[fedora-debuginfo]")
        .unwrap()
        .0
        .to_string();
    assert_eq!(fedora_block.matches("baseurl=").count(), 1);
    assert!(!fedora_block.contains("metalink="));
    assert!(!fedora_block.contains("#baseurl="));
}

#[test]
fn already_on_mirror_reports_no_change() {
    let dir = common::fixture_repo_dir();
    // First switch, then apply the planned content to disk, then plan again.
    let outcome = build_action_plan(
        &RepoLocations::legacy(&dir),
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else {
        panic!("expected a plan")
    };
    std::fs::write(&plan.files[0].path, &plan.files[0].content).unwrap();

    let outcome = build_action_plan(
        &RepoLocations::legacy(&dir),
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    match outcome {
        PlanOutcome::NoChange { reason } => {
            assert!(reason.contains("already on the Yandex mirror"))
        }
        PlanOutcome::Plan { .. } => panic!("expected NoChange"),
    }
}

#[test]
fn hyperdedic_rejects_rpmfusion_and_accepts_fedora() {
    let dir = common::fixture_repo_dir();
    let err = build_action_plan(
        &RepoLocations::legacy(&dir),
        "rpmfusion-free",
        &RepoAction::Switch {
            mirror: Mirror::Hyperdedic,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotSwitchable { .. }), "got {err:?}");
    assert!(err.to_string().contains("does not carry"));

    let outcome = build_action_plan(
        &RepoLocations::legacy(&dir),
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Hyperdedic,
        },
    )
    .expect("plan");
    assert!(matches!(outcome, PlanOutcome::Plan { .. }));
}

#[test]
fn cisco_modes_build_expected_plans() {
    let dir = common::fixture_repo_dir();
    let locations = RepoLocations::legacy(&dir);

    // disable: enabled=0, URL lines stay.
    let outcome = build_action_plan(
        &locations,
        "fedora-cisco-openh264",
        &RepoAction::Cisco {
            mode: CiscoMode::Disable,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan")
    };
    assert_eq!(preview.set_enabled.as_deref(), Some("0"));
    assert_eq!(preview.new_baseurl, None);
    assert_eq!(plan.snapshot_tag, "cisco-disable-fedora-cisco-openh264");
    let content = &plan.files[0].content;
    assert!(content.contains("[fedora-cisco-openh264]\nenabled=0"));
    assert!(content.contains(
        "metalink=https://mirrors.fedoraproject.org/metalink?repo=fedora-cisco-openh264-$releasever"
    ));

    // codecs: codecs baseurl + enabled=1.
    let outcome = build_action_plan(
        &locations,
        "fedora-cisco-openh264",
        &RepoAction::Cisco {
            mode: CiscoMode::Codecs,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan")
    };
    assert_eq!(preview.new_baseurl.as_deref(), Some(CISCO_CODECS_BASEURL));
    let content = &plan.files[0].content;
    assert!(content.contains(&format!(
        "[fedora-cisco-openh264]\nbaseurl={CISCO_CODECS_BASEURL}"
    )));
    assert!(content.contains("enabled=1"));

    // keep: nothing to do.
    let outcome = build_action_plan(
        &locations,
        "fedora-cisco-openh264",
        &RepoAction::Cisco {
            mode: CiscoMode::Keep,
        },
    )
    .expect("plan");
    match outcome {
        PlanOutcome::NoChange { .. } => {}
        _ => panic!("expected NoChange for keep"),
    }
}

#[test]
fn non_switchable_and_unknown_repositories_are_rejected() {
    let dir = common::fixture_repo_dir();
    let err = build_action_plan(
        &RepoLocations::legacy(&dir),
        "google-chrome",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotSwitchable { .. }));
    let err = build_action_plan(
        &RepoLocations::legacy(&dir),
        "no-such-repo",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}

#[test]
fn restore_plan_points_at_the_snapshot() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let snap =
        fmm_core::snapshot::create_snapshot(&repo_dir, &snap_root, "test-tag").expect("snapshot");
    let snap_id = snap.file_name().unwrap().to_string_lossy().into_owned();

    let plan = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, &snap_id, false)
        .expect("restore plan");
    assert_eq!(plan.action, PlanAction::Restore);
    assert_eq!(plan.snapshot_tag, "before-restore");
    assert_eq!(plan.snapshot.as_deref(), Some(snap.to_str().unwrap()));
    assert!(!plan.purge);
    assert!(plan.files.is_empty());

    // Unknown and hostile ids are rejected.
    let err = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, "no-such", false)
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
    let err =
        fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, "../..", false).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}

#[test]
fn parsed_plan_file_round_trips_through_repo_file_parser() {
    // The planned content must itself parse and keep the section structure.
    let dir = common::fixture_repo_dir();
    let outcome = build_action_plan(
        &RepoLocations::legacy(&dir),
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else {
        panic!("expected a plan")
    };
    let f = RepoFile::from_text(
        Path::new(&plan.files[0].path).to_path_buf(),
        &plan.files[0].content,
    );
    assert_eq!(f.sections().len(), 1);
    let url = f.section("updates").unwrap().url.clone().unwrap();
    assert_eq!(url.0, fmm_core::reposd::UrlKind::BaseUrl);
}

// ---------------------------------------------------------------------------
// F45+ layout: overriding copies in the local directory.
// ---------------------------------------------------------------------------

#[test]
fn f45_first_switch_builds_a_create_plan_for_the_shadow() {
    let (packaged, local) = common::fixture_f45_dirs();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan")
    };

    // The target is the shadow in the local dir, not the packaged file.
    let shadow = local.join("fedora-updates.repo");
    assert_eq!(plan.files[0].path, shadow.to_string_lossy());
    assert_eq!(preview.target_path, shadow.to_string_lossy());
    // The preview marks a created file and names the packaged source.
    assert!(preview.creates_file);
    let packaged_source = packaged
        .join("fedora-updates.repo")
        .to_string_lossy()
        .into_owned();
    assert_eq!(preview.source_path, Some(packaged_source));
    // Create-new sentinel: the shadow must not exist at apply time.
    assert_eq!(plan.files[0].before_sha256, "");
    // Content derives from the packaged file with only the section rewritten.
    assert!(plan.files[0].content.contains("[updates]\nbaseurl=https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/"));
    assert!(!plan.files[0].content.contains("metalink="));
    // The packaged file itself keeps the metalink (nothing wrote to it).
    let packaged_content = std::fs::read_to_string(packaged.join("fedora-updates.repo")).unwrap();
    assert!(packaged_content.contains("metalink="));
}

#[test]
fn f45_second_switch_rewrites_the_shadow_in_place_from_the_packaged_source() {
    let (packaged, local) = common::fixture_f45_dirs();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    // First switch creates the shadow (simulate the helper).
    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else {
        panic!("expected a plan")
    };
    let shadow = local.join("fedora-updates.repo");
    std::fs::write(&shadow, &plan.files[0].content).unwrap();

    // Re-picking the same mirror is a no-change, not a rewrite.
    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    assert!(
        matches!(outcome, PlanOutcome::NoChange { .. }),
        "re-picking the current mirror must be NoChange"
    );

    // A different mirror rewrites the shadow in place, based on the packaged file.
    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Hyperdedic,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan")
    };
    assert_eq!(plan.files[0].path, shadow.to_string_lossy());
    // Rewritten in place, but the content still derives from the packaged file.
    assert!(!preview.creates_file);
    let packaged_source = packaged
        .join("fedora-updates.repo")
        .to_string_lossy()
        .into_owned();
    assert_eq!(preview.source_path, Some(packaged_source));
    let current = std::fs::read_to_string(&shadow).unwrap();
    assert_eq!(
        plan.files[0].before_sha256,
        fmm_core::plan::sha256_hex(current.as_bytes())
    );
    assert!(
        plan.files[0].content.contains(
            "mirror.hyperdedic.ru/fedora/linux/updates/$releasever/Everything/$basearch/"
        )
    );
    assert!(!plan.files[0].content.contains("mirror.yandex.ru"));
    // Unrelated packaged content flows through (countme line from the fixture).
    assert!(plan.files[0].content.contains("countme=1"));
}

#[test]
fn f45_nochange_holds_even_when_the_packaged_source_drifted() {
    let (packaged, local) = common::fixture_f45_dirs();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else {
        panic!("expected a plan")
    };
    let shadow = local.join("fedora-updates.repo");
    std::fs::write(&shadow, &plan.files[0].content).unwrap();

    // The packaged file changes (package update) after the shadow was created.
    let packaged_path = packaged.join("fedora-updates.repo");
    let updated = std::fs::read_to_string(&packaged_path)
        .unwrap()
        .replace("countme=1", "countme=1\nmetadata_expire=1h");
    std::fs::write(&packaged_path, updated).unwrap();

    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .expect("plan");
    match outcome {
        PlanOutcome::NoChange { reason } => {
            assert!(reason.contains("already on the Yandex mirror"))
        }
        PlanOutcome::Plan { .. } => panic!("expected NoChange for a re-pick"),
    }
}

#[test]
fn f45_local_override_with_a_recognized_name_is_rewritten_in_place() {
    let (packaged, local) = common::fixture_f45_dirs();
    // A local file with a unique but family-recognizable name owns the section
    // (classification is substring-based over id + file name).
    let fork = local.join("fedora-updates-fork.repo");
    std::fs::write(
        &fork,
        "[updates]\nname=Local fork\nbaseurl=https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/\nenabled=1\n",
    )
    .unwrap();

    let locations = RepoLocations::f45_plus(&packaged, &local);
    let outcome = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Hyperdedic,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan")
    };
    assert_eq!(plan.files[0].path, fork.to_string_lossy());
    assert_eq!(preview.target_path, fork.to_string_lossy());
    // A local file without a packaged sibling rewrites its own content only.
    assert!(!preview.creates_file);
    assert_eq!(preview.source_path, None);
    let current = std::fs::read_to_string(&fork).unwrap();
    assert_eq!(
        plan.files[0].before_sha256,
        fmm_core::plan::sha256_hex(current.as_bytes())
    );
    assert!(plan.files[0].content.contains("mirror.hyperdedic.ru"));
}

#[test]
fn f45_existing_shadow_file_without_the_section_is_not_overwritten() {
    let (packaged, local) = common::fixture_f45_dirs();
    // A file with the packaged name exists but does not define the section.
    std::fs::write(
        local.join("fedora-updates.repo"),
        "[something-else]\nname=Other\nenabled=1\n",
    )
    .unwrap();

    let locations = RepoLocations::f45_plus(&packaged, &local);
    let err = build_action_plan(
        &locations,
        "updates",
        &RepoAction::Switch {
            mirror: Mirror::Yandex,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::NotSwitchable { .. }), "got {err:?}");
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn f45_cisco_modes_target_the_shadow_too() {
    let (packaged, local) = common::fixture_f45_dirs();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let outcome = build_action_plan(
        &locations,
        "fedora-cisco-openh264",
        &RepoAction::Cisco {
            mode: CiscoMode::Disable,
        },
    )
    .expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else {
        panic!("expected a plan")
    };
    assert_eq!(
        plan.files[0].path,
        local.join("fedora-cisco-openh264.repo").to_string_lossy()
    );
    assert_eq!(plan.files[0].before_sha256, "");
    assert!(
        plan.files[0]
            .content
            .contains("[fedora-cisco-openh264]\nenabled=0")
    );
}

// ---------------------------------------------------------------------------
// Bulk switch (one mirror → every switchable repository it carries)
// ---------------------------------------------------------------------------

use fmm_core::bulk::{BulkOutcome, build_bulk_plan};

/// Writes a two-section combo file: EPEL testing + EPEL modular, both metalink.
fn write_combo_repo(dir: &Path) {
    std::fs::write(
        dir.join("combo.repo"),
        "[epel-testing]\n\
         name=EPEL testing\n\
         metalink=https://mirrors.fedoraproject.org/metalink?repo=epel-testing\n\
         enabled=0\n\
         gpgcheck=1\n\
         \n\
         [epel-modular]\n\
         name=EPEL modular\n\
         metalink=https://mirrors.fedoraproject.org/metalink?repo=epel-modular\n\
         enabled=0\n\
         gpgcheck=1\n",
    )
    .unwrap();
}

fn expect_bulk_plan(
    outcome: BulkOutcome,
) -> (fmm_core::bulk::BulkPreview, fmm_core::plan::ApplyPlan) {
    match outcome {
        BulkOutcome::Plan { preview, plan } => (preview, plan),
        BulkOutcome::NoChange { reason, skipped } => {
            panic!("expected a bulk plan, got NoChange: {reason} (skipped: {skipped:?})")
        }
    }
}

#[test]
fn bulk_yandex_switches_every_switchable_repository() {
    let dir = common::fixture_repo_dir();
    let (preview, plan) = expect_bulk_plan(
        build_bulk_plan(&RepoLocations::legacy(&dir), Mirror::Yandex).expect("plan"),
    );

    // Everything switchable and not yet Yandex is in the batch, as one file
    // rewrite per .repo file; the snapshot covers the whole batch.
    assert_eq!(plan.snapshot_tag, "switch-all-yandex");
    assert_eq!(plan.action, PlanAction::Switch);
    assert!(!plan.refresh_cache);
    let mut paths: Vec<&str> = plan
        .files
        .iter()
        .map(|f| f.path.rsplit('/').next().unwrap())
        .collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec![
            "epel.repo",
            "fedora-updates-testing.repo",
            "fedora-updates.repo",
            "fedora.repo",
            "rpmfusion-free.repo",
            "rpmfusion-nonfree-updates.repo",
        ]
    );
    // Per-file hash guards: each matches the current on-disk content.
    for file in &plan.files {
        let current = std::fs::read_to_string(&file.path).unwrap();
        assert_eq!(
            file.before_sha256,
            fmm_core::plan::sha256_hex(current.as_bytes())
        );
    }
    // The fedora section received the Yandex baseurl and lost the metalink;
    // the non-switchable fedora-debuginfo section is byte-identical.
    let fedora = &plan
        .files
        .iter()
        .find(|f| f.path.ends_with("fedora.repo"))
        .unwrap()
        .content;
    assert!(fedora.contains("baseurl=https://mirror.yandex.ru/fedora/linux/releases/$releasever/Everything/$basearch/os/"));
    // The fedora section lost its metalink (the not-switchable
    // fedora-debuginfo section below keeps its own, byte-identical).
    assert!(!fedora.contains("metalink?repo=fedora-$releasever"));
    assert!(
        fedora
            .contains("[fedora-debuginfo]\nname=Fedora $releasever - $basearch - Debug\nmetalink=")
    );

    // Already-on-mirror is reported, everything else excluded is silent.
    let already: Vec<_> = preview
        .skipped
        .iter()
        .filter(|s| s.repo_id == "fedora-yandex-example")
        .collect();
    assert_eq!(already.len(), 1);
    assert_eq!(already[0].reason, "already on the Yandex mirror");
    assert!(preview.files.iter().all(|f| !f.creates_file));
    assert!(preview.files.iter().all(|f| f.source_path.is_none()));
}

#[test]
fn bulk_hyperdedic_skips_rpmfusion_with_reason() {
    let dir = common::fixture_repo_dir();
    let (preview, plan) = expect_bulk_plan(
        build_bulk_plan(&RepoLocations::legacy(&dir), Mirror::Hyperdedic).expect("plan"),
    );

    let names: Vec<&str> = plan
        .files
        .iter()
        .map(|f| f.path.rsplit('/').next().unwrap())
        .collect();
    // Fedora and EPEL only: no RPM Fusion file is touched. The Yandex example
    // file IS a candidate here — it is not on HyperDedic yet.
    assert_eq!(names.len(), 5);
    assert!(!names.iter().any(|n| n.contains("rpmfusion")));
    let skipped: Vec<_> = preview
        .skipped
        .iter()
        .filter(|s| s.repo_id.starts_with("rpmfusion"))
        .collect();
    assert_eq!(skipped.len(), 2);
    for skip in skipped {
        assert_eq!(
            skip.reason,
            "HyperDedic does not mirror RPM Fusion repositories"
        );
    }
}

#[test]
fn bulk_merges_two_sections_of_one_file_into_one_rewrite() {
    let dir = common::fixture_repo_dir();
    write_combo_repo(&dir);
    let (preview, plan) = expect_bulk_plan(
        build_bulk_plan(&RepoLocations::legacy(&dir), Mirror::Yandex).expect("plan"),
    );

    // Exactly one plan entry for combo.repo carrying both sections; the
    // baseurl lands directly under each header, URL lines are gone.
    let combo: Vec<_> = plan
        .files
        .iter()
        .filter(|f| f.path.ends_with("combo.repo"))
        .collect();
    assert_eq!(combo.len(), 1);
    let entry = combo[0];
    assert!(entry.content.contains(
        "baseurl=https://mirror.yandex.ru/fedora/epel/testing/$releasever/Everything/$basearch/"
    ));
    assert!(
        entry.content.contains(
            "baseurl=https://mirror.yandex.ru/fedora/epel/$releasever/Modular/$basearch/"
        )
    );
    assert!(!entry.content.contains("metalink="));
    // Both sections keep their non-URL lines.
    assert!(entry.content.contains(
        "[epel-testing]\nbaseurl=https://mirror.yandex.ru/fedora/epel/testing/$releasever/Everything/$basearch/\nname=EPEL testing\nenabled=0\ngpgcheck=1"
    ));
    assert!(entry.content.contains("[epel-modular]"));
    // One preview group with two sections, removed lines per section.
    let preview_combo: Vec<_> = preview
        .files
        .iter()
        .filter(|f| f.file == "combo.repo")
        .collect();
    assert_eq!(preview_combo.len(), 1);
    assert_eq!(preview_combo[0].sections.len(), 2);
    let ids: Vec<&str> = preview_combo[0]
        .sections
        .iter()
        .map(|s| s.repo_id.as_str())
        .collect();
    assert_eq!(ids, vec!["epel-testing", "epel-modular"]);
    assert!(
        preview_combo[0]
            .sections
            .iter()
            .all(|s| s.removed_lines.len() == 1)
    );
}

#[test]
fn bulk_f45_creates_one_overriding_copy_per_packaged_file() {
    let (packaged, local) = common::fixture_f45_dirs();
    write_combo_repo(&packaged);
    let locations = RepoLocations::f45_plus(&packaged, &local);
    let (preview, plan) =
        expect_bulk_plan(build_bulk_plan(&locations, Mirror::Yandex).expect("plan"));

    let combo: Vec<_> = plan
        .files
        .iter()
        .filter(|f| f.path == local.join("combo.repo").to_string_lossy())
        .collect();
    assert_eq!(combo.len(), 1, "one overriding copy for two sections");
    assert_eq!(combo[0].before_sha256, "", "create sentinel");
    assert!(combo[0].content.contains(
        "[epel-testing]\nbaseurl=https://mirror.yandex.ru/fedora/epel/testing/$releasever/Everything/$basearch/"
    ));
    // The packaged location is never written.
    assert!(
        plan.files
            .iter()
            .all(|f| !f.path.starts_with(packaged.to_string_lossy().as_ref()))
    );
    let before_packaged = std::fs::read_to_string(packaged.join("combo.repo")).unwrap();
    assert_eq!(
        fmm_core::plan::sha256_hex(before_packaged.as_bytes()),
        fmm_core::plan::sha256_hex(&std::fs::read(packaged.join("combo.repo")).unwrap())
    );
    // Preview: created file, derived from the packaged source, both sections.
    let pv = preview
        .files
        .iter()
        .find(|f| f.file == "combo.repo")
        .unwrap();
    assert!(pv.creates_file);
    assert_eq!(
        pv.source_path.as_deref(),
        Some(packaged.join("combo.repo").to_string_lossy().as_ref())
    );
    assert_eq!(pv.sections.len(), 2);
}

#[test]
fn bulk_f45_rewrites_existing_shadow_in_place_from_the_packaged_source() {
    let (packaged, local) = common::fixture_f45_dirs();
    write_combo_repo(&packaged);
    // An outdated shadow: same sections, but missing a packaged update line.
    std::fs::write(
        local.join("combo.repo"),
        "[epel-testing]\nname=EPEL testing\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=epel-testing\nenabled=0\n\n[epel-modular]\nname=EPEL modular\nmetalink=https://mirrors.fedoraproject.org/metalink?repo=epel-modular\nenabled=0\n",
    )
    .unwrap();
    // Simulate a package update that touched the packaged file after the shadow.
    let packaged_text = std::fs::read_to_string(packaged.join("combo.repo")).unwrap();
    let drifted = packaged_text.replacen("gpgcheck=1", "gpgcheck=1\nmetadata_expire=6h", 1);
    assert_ne!(drifted, packaged_text);
    std::fs::write(packaged.join("combo.repo"), drifted).unwrap();

    let locations = RepoLocations::f45_plus(&packaged, &local);
    let (preview, plan) =
        expect_bulk_plan(build_bulk_plan(&locations, Mirror::Yandex).expect("plan"));

    let combo: Vec<_> = plan
        .files
        .iter()
        .filter(|f| f.path.ends_with("combo.repo"))
        .collect();
    assert_eq!(combo.len(), 1);
    // Rewritten in place (no create sentinel), rebuilt from the CURRENT
    // packaged file: the drift line is carried over, both sections switched.
    assert_ne!(combo[0].before_sha256, "");
    let shadow_now = std::fs::read_to_string(local.join("combo.repo")).unwrap();
    assert_eq!(
        combo[0].before_sha256,
        fmm_core::plan::sha256_hex(shadow_now.as_bytes())
    );
    assert!(combo[0].content.contains("metadata_expire=6h"));
    assert!(
        combo[0]
            .content
            .contains("baseurl=https://mirror.yandex.ru/fedora/epel/testing/")
    );
    assert!(!combo[0].content.contains("metalink="));
    let pv = preview
        .files
        .iter()
        .find(|f| f.file == "combo.repo")
        .unwrap();
    assert!(!pv.creates_file);
    assert_eq!(
        pv.source_path.as_deref(),
        Some(packaged.join("combo.repo").to_string_lossy().as_ref())
    );
}

#[test]
fn bulk_conflicting_dest_is_a_skip_not_a_batch_error() {
    // Fresh dirs (no other fixture repos): the conflicting combo file is the
    // only candidate, so the outcome must be NoChange with the skips.
    let packaged = common::empty_dir();
    let local = common::empty_dir();
    write_combo_repo(&packaged);
    // A local file with the packaged file's name that does not define the
    // sections: the single-switch conflict guard, as a per-repository skip.
    std::fs::write(
        local.join("combo.repo"),
        "[unrelated]\nname=Unrelated\nenabled=1\n",
    )
    .unwrap();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let outcome = build_bulk_plan(&locations, Mirror::Yandex).expect("planning succeeds");
    match outcome {
        BulkOutcome::NoChange { skipped, .. } => {
            let conflicts: Vec<_> = skipped
                .iter()
                .filter(|s| s.repo_id == "epel-testing" || s.repo_id == "epel-modular")
                .collect();
            assert_eq!(conflicts.len(), 2);
            assert!(conflicts.iter().all(|s| {
                s.reason
                    .contains("an overriding file named combo.repo already exists")
            }));
        }
        BulkOutcome::Plan { .. } => panic!("conflicting sections must not be planned"),
    }
}

#[test]
fn bulk_everything_already_on_the_mirror_is_nochange() {
    let dir = common::empty_dir();
    std::fs::write(
        dir.join("yandex.repo"),
        "[fedora]\nname=Fedora\nbaseurl=https://mirror.yandex.ru/fedora/linux/releases/$releasever/Everything/$basearch/os/\nenabled=1\n",
    )
    .unwrap();
    let outcome = build_bulk_plan(&RepoLocations::legacy(&dir), Mirror::Yandex).expect("plan");
    match outcome {
        BulkOutcome::NoChange { reason, skipped } => {
            assert_eq!(reason, "nothing to switch to the Yandex mirror");
            let already: Vec<_> = skipped.iter().filter(|s| s.repo_id == "fedora").collect();
            assert_eq!(already.len(), 1);
            assert_eq!(already[0].reason, "already on the Yandex mirror");
        }
        BulkOutcome::Plan { .. } => panic!("nothing to switch, expected NoChange"),
    }
}

#[test]
fn bulk_f45_unreadable_packaged_sibling_falls_back_to_the_shadow_content() {
    let packaged = common::empty_dir();
    let local = common::empty_dir();
    // The packaged sibling of the same name is unreadable (inventory surfaces
    // this as a file error), while the local shadow is valid and effective.
    std::fs::write(packaged.join("combo.repo"), [0xff, 0xfe, 0x00]).unwrap();
    write_combo_repo(&local);
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let (preview, plan) =
        expect_bulk_plan(build_bulk_plan(&locations, Mirror::Yandex).expect("plan"));
    let combo: Vec<_> = plan
        .files
        .iter()
        .filter(|f| f.path.ends_with("combo.repo"))
        .collect();
    assert_eq!(
        combo.len(),
        1,
        "the shadow is rewritten from its own content"
    );
    assert_ne!(combo[0].before_sha256, "");
    assert!(
        combo[0]
            .content
            .contains("baseurl=https://mirror.yandex.ru/fedora/epel/testing/")
    );
    let pv = preview
        .files
        .iter()
        .find(|f| f.file == "combo.repo")
        .unwrap();
    // Fallback: no packaged source is named in the preview.
    assert!(!pv.creates_file);
    assert_eq!(pv.source_path, None);
}

#[test]
fn bulk_empty_repo_directory_is_nochange() {
    let dir = common::empty_dir();
    let outcome = build_bulk_plan(&RepoLocations::legacy(&dir), Mirror::Yandex).expect("plan");
    match outcome {
        BulkOutcome::NoChange { reason, skipped } => {
            assert_eq!(reason, "nothing to switch to the Yandex mirror");
            assert!(skipped.is_empty());
        }
        BulkOutcome::Plan { .. } => panic!("empty directory, expected NoChange"),
    }
}
