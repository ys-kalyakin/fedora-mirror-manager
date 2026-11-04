//! Snapshot creation, listing, pruning and restore preview over temp dirs
//! (tasks 4.1-4.2).

mod common;

use fmm_core::snapshot::{create_snapshot, extra_repo_files, list_snapshots, plan_prune, restore_preview};
use std::fs;
use std::path::Path;

#[test]
fn snapshot_captures_the_directory_and_is_listed_newest_first() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();

    let snap = create_snapshot(&repo_dir, &snap_root, "before-switch-updates-yandex").expect("snapshot");
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
    assert!(extra_repo_files(&repo_dir, &snap).expect("extras").is_empty());

    fs::write(repo_dir.join("added-later.repo"), "[added]\nname=x\n").unwrap();
    let extras = extra_repo_files(&repo_dir, &snap).expect("extras");
    assert_eq!(extras, vec!["added-later.repo".to_string()]);

    // Restore preview goes through validation and reports the same.
    let id = snap.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(restore_preview(&repo_dir, &snap_root, &id).expect("preview"), extras);
}

#[test]
fn non_repo_files_are_ignored_in_extras() {
    let repo_dir = common::fixture_repo_dir();
    let snap_root = common::empty_dir();
    let snap = create_snapshot(&repo_dir, &snap_root, "x").expect("snapshot");
    fs::write(repo_dir.join("README.txt"), "not a repo file").unwrap();
    assert!(extra_repo_files(&repo_dir, &snap).expect("extras").is_empty());
}
