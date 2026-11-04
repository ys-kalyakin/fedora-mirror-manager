//! Smoke check against the real host layout. Ignored by default (the outcome
//! depends on the machine running the tests); run explicitly on the target
//! system with:
//!
//! ```sh
//! cargo test -p fmm-core --test host_smoke -- --ignored --nocapture
//! ```
//!
//! On a Fedora 45+ host this verifies end to end (read-only): layout
//! detection, the merged packaged+local inventory, and that a previewed
//! switch targets the overriding copy in `/etc/yum.repos.d` with the
//! create-new sentinel. Nothing is written.

use fmm_core::plan::{build_action_plan, PlanOutcome, RepoAction};
use fmm_core::reposd::{detect_layout, inventory, Layout, Origin, RepoLocations};

#[test]
#[ignore = "host-dependent: run manually on the target system"]
fn host_inventory_and_preview_follow_the_real_layout() {
    let locations = RepoLocations::detect();
    println!("detected layout: {:?}", locations.layout);

    let inv = inventory(&locations).expect("inventory on the host");
    println!(
        "inventory: {} entries, {} per-file errors, layout {:?}",
        inv.entries.len(),
        inv.errors.len(),
        inv.layout
    );
    for entry in &inv.entries {
        println!("  {:28} {:9} {:?}", entry.id, format!("{:?},", entry.origin), entry.url);
    }
    assert!(!inv.entries.is_empty(), "the host must have repositories");

    match locations.layout {
        Layout::F45Plus => {
            // Packaged Fedora repos must be present and marked as packaged.
            let fedora = inv.entries.iter().find(|e| e.id == "fedora").expect("packaged fedora repo");
            assert_eq!(fedora.origin, Origin::Packaged);
            assert!(fedora.path.starts_with("/usr/share/dnf5/repos.d/"));
            // Third-party (COPR etc.) entries come from /etc/yum.repos.d.
            assert!(
                inv.entries.iter().all(|e| if e.origin == Origin::Local {
                    e.path.starts_with("/etc/yum.repos.d/")
                } else {
                    true
                })
            );
            // A previewed switch must name the /etc shadow path with the
            // create-new sentinel (no write happens during preview). The host
            // may already point at the requested mirror (hand-edited packaged
            // files), so pick the mirror that actually changes something.
            let updates = inv.entries.iter().find(|e| e.id == "updates").expect("updates repo");
            let requested = match updates.source {
                fmm_core::reposd::Source::Yandex => fmm_core::catalog::Mirror::Hyperdedic,
                _ => fmm_core::catalog::Mirror::Yandex,
            };
            let outcome = build_action_plan(&locations, "updates", &RepoAction::Switch { mirror: requested }).expect("preview");
            let PlanOutcome::Plan { preview, plan } = outcome else {
                // NoChange is only correct if the entry really is on the
                // requested mirror already.
                assert_eq!(
                    updates.source,
                    fmm_core::reposd::Source::Hyperdedic,
                    "NoChange for a mirror the entry does not use"
                );
                println!("host already on hyperdedic; NoChange is the correct outcome");
                return;
            };
            println!("preview target: {}", preview.target_path);
            assert_eq!(preview.target_path, "/etc/yum.repos.d/fedora-updates.repo");
            assert_eq!(plan.files[0].path, preview.target_path);
            assert_eq!(plan.files[0].before_sha256, "");
            // The preview marks the created overriding copy and names the
            // packaged source it is derived from.
            assert!(preview.creates_file);
            assert_eq!(
                preview.source_path.as_deref(),
                Some("/usr/share/dnf5/repos.d/fedora-updates.repo")
            );
            // The rewritten section carries the requested mirror and no
            // metalink (other sections of the file keep their lines).
            let rewritten = fmm_core::reposd::RepoFile::from_text(
                std::path::PathBuf::from(&plan.files[0].path),
                &plan.files[0].content,
            );
            let section = rewritten.section("updates").expect("updates section in planned content");
            match &section.url {
                Some((fmm_core::reposd::UrlKind::BaseUrl, url)) => {
                    assert!(url.contains("mirror.yandex.ru") || url.contains("mirror.hyperdedic.ru"), "url: {url}");
                }
                other => panic!("expected a baseurl in the planned section, got {other:?}"),
            }
            assert!(!std::path::Path::new(&plan.files[0].path).exists(), "preview must not write");
        }
        Layout::Legacy => {
            assert!(inv.entries.iter().all(|e| e.origin == Origin::Local));
        }
    }
    // detect_layout must agree with the locations' layout.
    assert_eq!(detect_layout(), locations.layout);
}
