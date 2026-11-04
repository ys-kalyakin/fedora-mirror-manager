//! Inventory behavior over the fixture repo directory (tasks 1.1-1.3).

mod common;

use fmm_core::classify::Switchability;
use fmm_core::error::Error;
use fmm_core::reposd::{Layout, Origin, RepoLocations, Source, detect_layout_in, inventory};
use std::fs;

#[test]
fn inventories_the_fixture_directory() {
    let dir = common::fixture_repo_dir();
    let inv = inventory(&RepoLocations::legacy(&dir)).expect("inventory");

    let by_id = |id: &str| {
        inv.entries
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing {id}"))
    };

    let updates = by_id("updates");
    assert_eq!(updates.file, "fedora-updates.repo");
    assert_eq!(updates.name, "Fedora $releasever - $basearch - Updates");
    assert!(updates.enabled);
    assert_eq!(updates.source, Source::Metalink);
    assert_eq!(updates.kind, fmm_core::classify::RepoKey::Updates);
    assert_eq!(updates.switchability, Switchability::Switchable);
    // Legacy: every entry is local, and the path points into the directory.
    assert_eq!(updates.origin, Origin::Local);
    assert_eq!(
        updates.path,
        dir.join("fedora-updates.repo").to_string_lossy()
    );

    let fedora = by_id("fedora");
    assert_eq!(fedora.source, Source::Metalink);
    assert!(
        fedora
            .url
            .as_deref()
            .unwrap()
            .starts_with("https://mirrors.fedoraproject.org/metalink")
    );

    let yandex = by_id("fedora-yandex-example");
    assert_eq!(yandex.source, Source::Yandex);

    let chrome = by_id("google-chrome");
    assert_eq!(chrome.source, Source::Other);
    assert_eq!(chrome.switchability, Switchability::NotSwitchable);
    assert_eq!(
        chrome.url.as_deref(),
        Some("https://dl.google.com/linux/chrome/rpm/stable/x86_64")
    );

    let cisco = by_id("fedora-cisco-openh264");
    assert_eq!(cisco.switchability, Switchability::Cisco);

    // Source/debuginfo sections are display-only.
    assert_eq!(
        by_id("fedora-debuginfo").switchability,
        Switchability::NotSwitchable
    );
    // Subscription-managed file stays display-only.
    assert_eq!(
        by_id("rhel-9-for-x86_64-baseos-rpms").switchability,
        Switchability::NotSwitchable
    );

    // 11 fixture sections in total.
    assert_eq!(inv.entries.len(), 11);
    assert!(inv.errors.is_empty());
    assert_eq!(inv.layout, Layout::Legacy);
}

#[test]
fn empty_directory_is_a_valid_empty_inventory() {
    let dir = common::empty_dir();
    let inv = inventory(&RepoLocations::legacy(&dir)).expect("inventory");
    assert!(inv.entries.is_empty());
    assert!(inv.errors.is_empty());
}

#[test]
fn missing_directory_is_an_error() {
    let err = inventory(&RepoLocations::legacy(
        std::env::temp_dir().join("fmm-core-no-such-dir-xyz"),
    ))
    .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
    assert!(err.to_string().contains("repository directory"));
}

#[test]
fn unreadable_file_is_reported_alongside_parsed_entries() {
    let dir = common::fixture_repo_dir();
    // Invalid UTF-8 content makes exactly one file unparseable.
    fs::write(dir.join("broken.repo"), [0x5b, 0xff, 0x5d, 0x0a]).unwrap();
    let inv = inventory(&RepoLocations::legacy(&dir)).expect("inventory with per-file error");
    assert_eq!(inv.errors.len(), 1);
    assert_eq!(inv.errors[0].file, "broken.repo");
    assert!(inv.errors[0].message.contains("UTF-8"));
    // The readable files are still there.
    assert!(inv.entries.iter().any(|e| e.id == "updates"));
}

#[test]
fn layout_detection_follows_the_packaged_directory() {
    // Missing or empty packaged dir → legacy; one .repo file → F45+.
    assert_eq!(detect_layout_in(&common::empty_dir()), Layout::Legacy);
    assert_eq!(
        detect_layout_in(&std::env::temp_dir().join("fmm-core-no-such-packaged-dir")),
        Layout::Legacy
    );
    let (packaged, _local) = common::fixture_f45_dirs();
    assert_eq!(detect_layout_in(&packaged), Layout::F45Plus);
}

#[test]
fn f45_inventory_merges_both_locations() {
    let (packaged, local) = common::fixture_f45_dirs();
    // A third-party file only exists locally.
    fs::write(
        local.join("third-party.repo"),
        "[acme]\nname=Acme\nbaseurl=https://acme.example/repo\nenabled=1\n",
    )
    .unwrap();

    let locations = RepoLocations::f45_plus(&packaged, &local);
    let inv = inventory(&locations).expect("inventory");
    assert_eq!(inv.layout, Layout::F45Plus);

    let updates = inv
        .entries
        .iter()
        .find(|e| e.id == "updates")
        .expect("packaged entry");
    assert_eq!(updates.origin, Origin::Packaged);
    assert_eq!(
        updates.path,
        packaged.join("fedora-updates.repo").to_string_lossy()
    );

    let acme = inv
        .entries
        .iter()
        .find(|e| e.id == "acme")
        .expect("local entry");
    assert_eq!(acme.origin, Origin::Local);
    assert_eq!(acme.switchability, Switchability::NotSwitchable);
}

#[test]
fn f45_local_definition_overrides_the_packaged_section() {
    let (packaged, local) = common::fixture_f45_dirs();
    // A local file redefines the `updates` section from a differently named file.
    fs::write(
        local.join("my-fork.repo"),
        "[updates]\nname=Local fork\nbaseurl=https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/\nenabled=1\n",
    )
    .unwrap();

    let inv = inventory(&RepoLocations::f45_plus(&packaged, &local)).expect("inventory");
    let updates: Vec<_> = inv.entries.iter().filter(|e| e.id == "updates").collect();
    assert_eq!(
        updates.len(),
        1,
        "the packaged duplicate must not be listed separately"
    );
    assert_eq!(updates[0].origin, Origin::Local);
    assert_eq!(updates[0].file, "my-fork.repo");
    assert_eq!(updates[0].source, Source::Yandex);
    // The packaged sections of other files are untouched.
    assert!(
        inv.entries
            .iter()
            .any(|e| e.id == "fedora" && e.origin == Origin::Packaged)
    );
}

#[test]
fn f45_missing_local_dir_still_reads_the_packaged_one() {
    let (packaged, local) = common::fixture_f45_dirs();
    fs::remove_dir(&local).unwrap();
    let inv =
        inventory(&RepoLocations::f45_plus(&packaged, &local)).expect("packaged entries survive");
    assert!(!inv.entries.is_empty());
    assert!(inv.entries.iter().all(|e| e.origin == Origin::Packaged));
}

#[test]
fn f45_missing_all_directories_is_an_error() {
    let base = std::env::temp_dir().join("fmm-core-f45-missing");
    let locations = RepoLocations::f45_plus(base.join("packaged"), base.join("local"));
    let err = inventory(&locations).unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
    // Both locations are named in the error.
    let message = err.to_string();
    assert!(
        message.contains("packaged") && message.contains("local"),
        "message: {message}"
    );
}
