//! Switch-plan building over the fixture repo directory (tasks 2.1-2.2).

mod common;

use fmm_core::catalog::{baseurl, Mirror, CISCO_CODECS_BASEURL};
use fmm_core::error::Error;
use fmm_core::plan::{build_action_plan, CiscoMode, PlanAction, PlanOutcome, RepoAction};
use fmm_core::reposd::{RepoFile, RepoLocations};
use std::path::Path;

#[test]
fn switch_plan_carries_expected_baseurl_and_preview() {
    let dir = common::fixture_repo_dir();
    let outcome = build_action_plan(&RepoLocations::legacy(&dir), "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else {
        panic!("expected a plan, got NoChange");
    };

    // The planned baseurl equals the reference mapping for the family.
    let expected = baseurl(Mirror::Yandex, fmm_core::classify::RepoKey::Updates).unwrap();
    assert_eq!(preview.new_baseurl.as_deref(), Some(expected.as_str()));
    assert_eq!(preview.set_enabled, None);
    // The commented baseurl and the metalink line are removed.
    assert!(preview.removed_lines.iter().any(|l| l.contains("metalink=")));
    assert!(preview.removed_lines.iter().any(|l| l.contains("#baseurl=")));
    // The preview names the effective target file.
    assert_eq!(preview.target_path, dir.join("fedora-updates.repo").to_string_lossy());
    // Legacy rewrite: an existing file, no packaged source involved.
    assert!(!preview.creates_file);
    assert_eq!(preview.source_path, None);

    assert_eq!(plan.action, PlanAction::Switch);
    assert_eq!(plan.snapshot_tag, "switch-updates-yandex");
    assert_eq!(plan.files.len(), 1);
    assert!(plan.files[0].path.ends_with("fedora-updates.repo"));
    // Fresh file: before hash must match the current content on disk.
    let current = std::fs::read_to_string(&plan.files[0].path).unwrap();
    assert_eq!(plan.files[0].before_sha256, fmm_core::plan::sha256_hex(current.as_bytes()));
    // New content places the baseurl under the header.
    assert!(plan.files[0].content.contains("[updates]\nbaseurl=https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/"));
}

#[test]
fn switch_touches_only_the_target_section() {
    let dir = common::fixture_repo_dir();
    let outcome = build_action_plan(&RepoLocations::legacy(&dir), "fedora", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else { panic!("expected a plan") };
    let content = &plan.files[0].content;

    // Sibling section is byte-identical.
    let original = std::fs::read_to_string(&plan.files[0].path).unwrap();
    fn sibling(text: &str) -> &str {
        text.split_once("[fedora-debuginfo]").unwrap().1
    }
    assert_eq!(sibling(&original), sibling(content));

    // Signing configuration survives.
    assert!(content.contains("gpgcheck=1"));
    assert!(content.contains("gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-$releasever-$basearch"));
    assert!(content.contains("enabled=1"));
    // Exactly one baseurl line in the fedora section, no metalink left there.
    let fedora_block = content.split_once("[fedora-debuginfo]").unwrap().0.to_string();
    assert_eq!(fedora_block.matches("baseurl=").count(), 1);
    assert!(!fedora_block.contains("metalink="));
    assert!(!fedora_block.contains("#baseurl="));
}

#[test]
fn already_on_mirror_reports_no_change() {
    let dir = common::fixture_repo_dir();
    // First switch, then apply the planned content to disk, then plan again.
    let outcome = build_action_plan(&RepoLocations::legacy(&dir), "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else { panic!("expected a plan") };
    std::fs::write(&plan.files[0].path, &plan.files[0].content).unwrap();

    let outcome = build_action_plan(&RepoLocations::legacy(&dir), "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    match outcome {
        PlanOutcome::NoChange { reason } => assert!(reason.contains("already on the Yandex mirror")),
        PlanOutcome::Plan { .. } => panic!("expected NoChange"),
    }
}

#[test]
fn hyperdedic_rejects_rpmfusion_and_accepts_fedora() {
    let dir = common::fixture_repo_dir();
    let err = build_action_plan(&RepoLocations::legacy(&dir), "rpmfusion-free", &RepoAction::Switch { mirror: Mirror::Hyperdedic }).unwrap_err();
    assert!(matches!(err, Error::NotSwitchable { .. }), "got {err:?}");
    assert!(err.to_string().contains("does not carry"));

    let outcome = build_action_plan(&RepoLocations::legacy(&dir), "updates", &RepoAction::Switch { mirror: Mirror::Hyperdedic }).expect("plan");
    assert!(matches!(outcome, PlanOutcome::Plan { .. }));
}

#[test]
fn cisco_modes_build_expected_plans() {
    let dir = common::fixture_repo_dir();
    let locations = RepoLocations::legacy(&dir);

    // disable: enabled=0, URL lines stay.
    let outcome = build_action_plan(&locations, "fedora-cisco-openh264", &RepoAction::Cisco { mode: CiscoMode::Disable }).expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else { panic!("expected a plan") };
    assert_eq!(preview.set_enabled.as_deref(), Some("0"));
    assert_eq!(preview.new_baseurl, None);
    assert_eq!(plan.snapshot_tag, "cisco-disable-fedora-cisco-openh264");
    let content = &plan.files[0].content;
    assert!(content.contains("[fedora-cisco-openh264]\nenabled=0"));
    assert!(content.contains("metalink=https://mirrors.fedoraproject.org/metalink?repo=fedora-cisco-openh264-$releasever"));

    // codecs: codecs baseurl + enabled=1.
    let outcome = build_action_plan(&locations, "fedora-cisco-openh264", &RepoAction::Cisco { mode: CiscoMode::Codecs }).expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else { panic!("expected a plan") };
    assert_eq!(preview.new_baseurl.as_deref(), Some(CISCO_CODECS_BASEURL));
    let content = &plan.files[0].content;
    assert!(content.contains(&format!("[fedora-cisco-openh264]\nbaseurl={CISCO_CODECS_BASEURL}")));
    assert!(content.contains("enabled=1"));

    // keep: nothing to do.
    let outcome = build_action_plan(&locations, "fedora-cisco-openh264", &RepoAction::Cisco { mode: CiscoMode::Keep }).expect("plan");
    match outcome {
        PlanOutcome::NoChange { .. } => {}
        _ => panic!("expected NoChange for keep"),
    }
}

#[test]
fn non_switchable_and_unknown_repositories_are_rejected() {
    let dir = common::fixture_repo_dir();
    let err = build_action_plan(&RepoLocations::legacy(&dir), "google-chrome", &RepoAction::Switch { mirror: Mirror::Yandex }).unwrap_err();
    assert!(matches!(err, Error::NotSwitchable { .. }));
    let err = build_action_plan(&RepoLocations::legacy(&dir), "no-such-repo", &RepoAction::Switch { mirror: Mirror::Yandex }).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}

#[test]
fn restore_plan_points_at_the_snapshot() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let snap = fmm_core::snapshot::create_snapshot(&repo_dir, &snap_root, "test-tag").expect("snapshot");
    let snap_id = snap.file_name().unwrap().to_string_lossy().into_owned();

    let plan = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, &snap_id, false).expect("restore plan");
    assert_eq!(plan.action, PlanAction::Restore);
    assert_eq!(plan.snapshot_tag, "before-restore");
    assert_eq!(plan.snapshot.as_deref(), Some(snap.to_str().unwrap()));
    assert!(!plan.purge);
    assert!(plan.files.is_empty());

    // Unknown and hostile ids are rejected.
    let err = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, "no-such", false).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
    let err = fmm_core::snapshot::build_restore_plan(&repo_dir, &snap_root, "../..", false).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
}

#[test]
fn parsed_plan_file_round_trips_through_repo_file_parser() {
    // The planned content must itself parse and keep the section structure.
    let dir = common::fixture_repo_dir();
    let outcome = build_action_plan(&RepoLocations::legacy(&dir), "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else { panic!("expected a plan") };
    let f = RepoFile::from_text(Path::new(&plan.files[0].path).to_path_buf(), &plan.files[0].content);
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

    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else { panic!("expected a plan") };

    // The target is the shadow in the local dir, not the packaged file.
    let shadow = local.join("fedora-updates.repo");
    assert_eq!(plan.files[0].path, shadow.to_string_lossy());
    assert_eq!(preview.target_path, shadow.to_string_lossy());
    // The preview marks a created file and names the packaged source.
    assert!(preview.creates_file);
    let packaged_source = packaged.join("fedora-updates.repo").to_string_lossy().into_owned();
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
    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else { panic!("expected a plan") };
    let shadow = local.join("fedora-updates.repo");
    std::fs::write(&shadow, &plan.files[0].content).unwrap();

    // Re-picking the same mirror is a no-change, not a rewrite.
    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    assert!(matches!(outcome, PlanOutcome::NoChange { .. }), "re-picking the current mirror must be NoChange");

    // A different mirror rewrites the shadow in place, based on the packaged file.
    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Hyperdedic }).expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else { panic!("expected a plan") };
    assert_eq!(plan.files[0].path, shadow.to_string_lossy());
    // Rewritten in place, but the content still derives from the packaged file.
    assert!(!preview.creates_file);
    let packaged_source = packaged.join("fedora-updates.repo").to_string_lossy().into_owned();
    assert_eq!(preview.source_path, Some(packaged_source));
    let current = std::fs::read_to_string(&shadow).unwrap();
    assert_eq!(plan.files[0].before_sha256, fmm_core::plan::sha256_hex(current.as_bytes()));
    assert!(plan.files[0].content.contains("mirror.hyperdedic.ru/fedora/linux/updates/$releasever/Everything/$basearch/"));
    assert!(!plan.files[0].content.contains("mirror.yandex.ru"));
    // Unrelated packaged content flows through (countme line from the fixture).
    assert!(plan.files[0].content.contains("countme=1"));
}

#[test]
fn f45_nochange_holds_even_when_the_packaged_source_drifted() {
    let (packaged, local) = common::fixture_f45_dirs();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else { panic!("expected a plan") };
    let shadow = local.join("fedora-updates.repo");
    std::fs::write(&shadow, &plan.files[0].content).unwrap();

    // The packaged file changes (package update) after the shadow was created.
    let packaged_path = packaged.join("fedora-updates.repo");
    let updated = std::fs::read_to_string(&packaged_path).unwrap().replace("countme=1", "countme=1\nmetadata_expire=1h");
    std::fs::write(&packaged_path, updated).unwrap();

    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).expect("plan");
    match outcome {
        PlanOutcome::NoChange { reason } => assert!(reason.contains("already on the Yandex mirror")),
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
    let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Hyperdedic }).expect("plan");
    let PlanOutcome::Plan { preview, plan } = outcome else { panic!("expected a plan") };
    assert_eq!(plan.files[0].path, fork.to_string_lossy());
    assert_eq!(preview.target_path, fork.to_string_lossy());
    // A local file without a packaged sibling rewrites its own content only.
    assert!(!preview.creates_file);
    assert_eq!(preview.source_path, None);
    let current = std::fs::read_to_string(&fork).unwrap();
    assert_eq!(plan.files[0].before_sha256, fmm_core::plan::sha256_hex(current.as_bytes()));
    assert!(plan.files[0].content.contains("mirror.hyperdedic.ru"));
}

#[test]
fn f45_existing_shadow_file_without_the_section_is_not_overwritten() {
    let (packaged, local) = common::fixture_f45_dirs();
    // A file with the packaged name exists but does not define the section.
    std::fs::write(local.join("fedora-updates.repo"), "[something-else]\nname=Other\nenabled=1\n").unwrap();

    let locations = RepoLocations::f45_plus(&packaged, &local);
    let err = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: Mirror::Yandex }).unwrap_err();
    assert!(matches!(err, Error::NotSwitchable { .. }), "got {err:?}");
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn f45_cisco_modes_target_the_shadow_too() {
    let (packaged, local) = common::fixture_f45_dirs();
    let locations = RepoLocations::f45_plus(&packaged, &local);

    let outcome = build_action_plan(&locations, "fedora-cisco-openh264", &RepoAction::Cisco { mode: CiscoMode::Disable }).expect("plan");
    let PlanOutcome::Plan { plan, .. } = outcome else { panic!("expected a plan") };
    assert_eq!(plan.files[0].path, local.join("fedora-cisco-openh264.repo").to_string_lossy());
    assert_eq!(plan.files[0].before_sha256, "");
    assert!(plan.files[0].content.contains("[fedora-cisco-openh264]\nenabled=0"));
}
