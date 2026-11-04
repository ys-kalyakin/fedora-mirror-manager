//! Snapshot creation, listing, pruning, deletion, migration and restore
//! preview over temp dirs (tasks 4.1-4.2, user-snapshot-storage 1.3-1.4).

mod common;

use fmm_core::snapshot::{
    build_restore_plan, create_snapshot, delete_snapshot, extra_repo_files, list_snapshots,
    migrate_snapshots, plan_prune, restore_preview,
};
use std::fs;
use std::path::Path;

#[test]
fn snapshot_captures_the_directory_and_is_listed_newest_first() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();

    let snap =
        create_snapshot(&repo_dir, &snap_root, "before-switch-updates-yandex").expect("snapshot");
    // Contents copied with names intact.
    assert!(snap.join("fedora.repo").is_file());
    assert!(snap.join("google-chrome.repo").is_file());
    // Name carries the UTC timestamp and the tag.
    let id = snap.file_name().unwrap().to_string_lossy().into_owned();
    assert!(id.ends_with("-before-switch-updates-yandex"), "id={id}");
    assert!(id.starts_with("20"));

    let listed = list_snapshots(&snap_root).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].tag, "before-switch-updates-yandex");
    assert!(listed[0].timestamp.starts_with("20"));
}

#[test]
fn missing_repo_dir_is_a_snapshot_error() {
    let snap_root = common::empty_dir();
    let err = create_snapshot(Path::new("/no/such/dir"), &snap_root, "x").unwrap_err();
    assert!(matches!(err, fmm_core::error::Error::Snapshot(_)));
}

#[test]
fn retention_keeps_only_the_newest_ten() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    for i in 0..12 {
        create_snapshot(&repo_dir, &snap_root, &format!("iter-{i:02}")).expect("snapshot");
    }
    let listed = list_snapshots(&snap_root).expect("list");
    assert_eq!(listed.len(), 10);
    // Newest first: the most recent tag is at the top, the two oldest are gone.
    assert_eq!(listed[0].tag, "iter-11");
    let names: Vec<&str> = listed.iter().map(|s| s.tag.as_str()).collect();
    assert!(!names.contains(&"iter-00"));
    assert!(!names.contains(&"iter-01"));
    assert_eq!(plan_prune(&snap_root, 10).expect("prune").len(), 0);
}

#[test]
fn collision_suffix_keeps_two_snapshots_in_one_second() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let a = create_snapshot(&repo_dir, &snap_root, "dup").expect("first");
    let b = create_snapshot(&repo_dir, &snap_root, "dup").expect("second");
    assert_ne!(a, b);
    assert_eq!(list_snapshots(&snap_root).expect("list").len(), 2);
}

#[test]
fn extra_repo_files_are_detected_for_the_restore_dialog() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let snap = create_snapshot(&repo_dir, &snap_root, "clean").expect("snapshot");
    assert!(
        extra_repo_files(&repo_dir, &snap)
            .expect("extras")
            .is_empty()
    );

    fs::write(repo_dir.join("added-later.repo"), "[added]\nname=x\n").unwrap();
    let extras = extra_repo_files(&repo_dir, &snap).expect("extras");
    assert_eq!(extras, vec!["added-later.repo".to_string()]);

    // Restore preview goes through validation and reports the same.
    let id = snap.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        restore_preview(&repo_dir, &snap_root, &id).expect("preview"),
        extras
    );
}

#[test]
fn non_repo_files_are_ignored_in_extras() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let snap = create_snapshot(&repo_dir, &snap_root, "x").expect("snapshot");
    fs::write(repo_dir.join("README.txt"), "not a repo file").unwrap();
    assert!(
        extra_repo_files(&repo_dir, &snap)
            .expect("extras")
            .is_empty()
    );
}

#[test]
fn delete_removes_only_the_chosen_snapshot() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let a = create_snapshot(&repo_dir, &snap_root, "keep-me").expect("first");
    let b = create_snapshot(&repo_dir, &snap_root, "drop-me").expect("second");

    let id = b.file_name().unwrap().to_string_lossy().into_owned();
    delete_snapshot(&snap_root, &id).expect("delete");

    // The chosen directory is gone, the other one is untouched.
    assert!(!b.is_dir());
    assert!(a.is_dir());
    let listed = list_snapshots(&snap_root).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].id,
        a.file_name().unwrap().to_string_lossy().into_owned()
    );
}

#[test]
fn delete_rejects_invalid_and_missing_ids_without_touching_the_rest() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let snap = create_snapshot(&repo_dir, &snap_root, "safe").expect("snapshot");
    let id = snap.file_name().unwrap().to_string_lossy().into_owned();

    for bad in ["", "a/b", "..", "no-such-snapshot"] {
        let err = delete_snapshot(&snap_root, bad).unwrap_err();
        assert!(
            matches!(err, fmm_core::error::Error::NotFound(_)),
            "id={bad:?}"
        );
    }
    // The valid snapshot survived every rejected attempt.
    assert!(snap.is_dir());
    assert_eq!(list_snapshots(&snap_root).expect("list").len(), 1);
    assert!(delete_snapshot(&snap_root, &id).is_ok());
}

#[test]
fn restore_plan_carries_the_snapshot_root_and_source() {
    let snap_root = common::empty_dir();
    let snap = create_snapshot(&common::fixture_repo_dir(), &snap_root, "rp").expect("snapshot");
    let id = snap.file_name().unwrap().to_string_lossy().into_owned();

    let plan =
        build_restore_plan(Path::new("/etc/yum.repos.d"), &snap_root, &id, false).expect("plan");
    assert_eq!(plan.plan_version, fmm_core::plan::PLAN_VERSION);
    assert_eq!(plan.snapshot_root, snap_root.to_string_lossy().into_owned());
    assert_eq!(
        plan.snapshot.as_deref(),
        Some(snap.to_string_lossy().as_ref())
    );
    assert_eq!(plan.snapshot_tag, "before-restore");
}

#[test]
fn migration_moves_snapshots_and_removes_an_emptied_legacy_root() {
    let legacy = common::empty_dir();
    let snap_root = common::empty_dir();
    fs::create_dir(legacy.join("20261101T000000Z-switch-fedora-yandex")).unwrap();
    fs::write(
        legacy.join("20261101T000000Z-switch-fedora-yandex/fedora.repo"),
        "[fedora]\n",
    )
    .unwrap();
    fs::create_dir(legacy.join("20261102T000000Z-before-restore")).unwrap();

    let report = migrate_snapshots(&legacy, &snap_root);
    assert!(report.ok, "report: {report:?}");
    assert_eq!(report.moved.len(), 2);
    assert!(report.failed.is_empty());
    // Both snapshots live in the user root now, with their contents.
    assert!(
        snap_root
            .join("20261101T000000Z-switch-fedora-yandex/fedora.repo")
            .is_file()
    );
    assert!(snap_root.join("20261102T000000Z-before-restore").is_dir());
    // Nothing was left behind: the legacy root is gone.
    assert!(!legacy.exists());
}

#[test]
fn migration_keeps_a_non_empty_legacy_root_and_reports_collisions() {
    let legacy = common::empty_dir();
    let snap_root = common::empty_dir();
    fs::create_dir(legacy.join("20261101T000000Z-switch-fedora-yandex")).unwrap();
    fs::write(legacy.join("notes.txt"), "not a snapshot").unwrap();
    // A snapshot with the same name already exists in the user root.
    fs::create_dir(snap_root.join("20261101T000000Z-switch-fedora-yandex")).unwrap();

    let report = migrate_snapshots(&legacy, &snap_root);
    assert!(!report.ok);
    assert!(report.moved.is_empty());
    assert_eq!(
        report.failed,
        vec!["20261101T000000Z-switch-fedora-yandex".to_string()]
    );
    // The collision was not overwritten and the stray file kept the root alive.
    assert!(legacy.join("notes.txt").is_file());
    assert!(
        legacy
            .join("20261101T000000Z-switch-fedora-yandex")
            .is_dir()
    );
}

#[test]
fn migration_without_a_legacy_root_is_a_no_op() {
    let report = migrate_snapshots(Path::new("/no/such/dir"), &common::empty_dir());
    assert!(report.ok);
    assert!(report.moved.is_empty());
    assert!(report.failed.is_empty());
}
