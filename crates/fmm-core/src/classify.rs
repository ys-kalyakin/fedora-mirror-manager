//! Classification of repository sections, ported from the reference
//! implementation (`mirror-switch.sh`, `repo_key`). The precedence and the
//! dot-to-space normalization follow the script exactly: unknown sections
//! default to [`RepoKey::Skip`], which is never rewritten.

use serde::Serialize;

/// Domain key of a repository section, mirroring the reference script's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepoKey {
    Fedora,
    FedoraModular,
    Updates,
    UpdatesModular,
    UpdatesTesting,
    UpdatesTestingModular,
    RfFree,
    RfFreeUpdates,
    RfFreeTesting,
    RfNonfree,
    RfNonfreeUpdates,
    RfNonfreeTesting,
    Epel,
    EpelTesting,
    EpelModular,
    /// Cisco OpenH264 repository with dedicated modes.
    Cisco,
    /// Unknown or intentionally untouched repository.
    Skip,
}

/// How (and whether) the user may act on a repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Switchability {
    /// Can be switched between the mirrors.
    Switchable,
    /// Cisco OpenH264 repository with dedicated modes.
    Cisco,
    /// Display only; never rewritten.
    NotSwitchable,
}

/// Classifies a repository section by its id and file name.
pub fn classify(id: &str, file_name: &str) -> RepoKey {
    let t = format!("{id} {file_name}").to_lowercase().replace('.', " ");

    // COPR repositories must be checked before the `*fedora*` pattern:
    // `copr.fedorainfracloud.org` contains a fedora substring.
    if t.contains("copr") {
        return RepoKey::Skip;
    }

    // Cisco OpenH264 (Fedora and EPEL) is handled separately.
    if t.contains("openh264") || t.contains("cisco") {
        return RepoKey::Cisco;
    }

    // Source / debuginfo / playground repositories are left as they are.
    if t.contains("source")
        || t.contains("srpm")
        || t.contains("debuginfo")
        || t.contains("playground")
    {
        return RepoKey::Skip;
    }

    if t.contains("rpmfusion") || t.contains("rpm-fusion") {
        let nonfree = t.contains("nonfree");
        if t.contains("testing") {
            return if nonfree {
                RepoKey::RfNonfreeTesting
            } else {
                RepoKey::RfFreeTesting
            };
        }
        if t.contains("updates") {
            return if nonfree {
                RepoKey::RfNonfreeUpdates
            } else {
                RepoKey::RfFreeUpdates
            };
        }
        return if nonfree {
            RepoKey::RfNonfree
        } else {
            RepoKey::RfFree
        };
    }

    if t.contains("epel") {
        if t.contains("testing") {
            return RepoKey::EpelTesting;
        }
        if t.contains("modular") {
            return RepoKey::EpelModular;
        }
        return RepoKey::Epel;
    }

    if t.contains("fedora") {
        let upd = t.contains("updates");
        let tst = t.contains("testing");
        let modular = t.contains("modular");
        if tst && modular {
            return RepoKey::UpdatesTestingModular;
        }
        if tst {
            return RepoKey::UpdatesTesting;
        }
        if upd && modular {
            return RepoKey::UpdatesModular;
        }
        if modular {
            return RepoKey::FedoraModular;
        }
        if upd {
            return RepoKey::Updates;
        }
        return RepoKey::Fedora;
    }

    RepoKey::Skip
}

/// Switchability implied by a repository key.
pub fn switchability(key: RepoKey) -> Switchability {
    match key {
        RepoKey::Cisco => Switchability::Cisco,
        RepoKey::Skip => Switchability::NotSwitchable,
        _ => Switchability::Switchable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_fedora_family() {
        assert_eq!(classify("fedora", "fedora.repo"), RepoKey::Fedora);
        assert_eq!(
            classify("fedora-modular", "fedora-modular.repo"),
            RepoKey::FedoraModular
        );
        assert_eq!(classify("updates", "fedora-updates.repo"), RepoKey::Updates);
        assert_eq!(
            classify("updates-modular", "fedora-updates-modular.repo"),
            RepoKey::UpdatesModular
        );
        assert_eq!(
            classify("updates-testing", "fedora-updates-testing.repo"),
            RepoKey::UpdatesTesting
        );
        assert_eq!(
            classify(
                "updates-testing-modular",
                "fedora-updates-testing-modular.repo"
            ),
            RepoKey::UpdatesTestingModular
        );
    }

    #[test]
    fn classifies_rpmfusion_family() {
        assert_eq!(
            classify("rpmfusion-free", "rpmfusion-free.repo"),
            RepoKey::RfFree
        );
        assert_eq!(
            classify("rpmfusion-free-updates", "rpmfusion-free-updates.repo"),
            RepoKey::RfFreeUpdates
        );
        assert_eq!(
            classify(
                "rpmfusion-free-updates-testing",
                "rpmfusion-free-updates-testing.repo"
            ),
            RepoKey::RfFreeTesting
        );
        assert_eq!(
            classify("rpmfusion-nonfree", "rpmfusion-nonfree.repo"),
            RepoKey::RfNonfree
        );
        assert_eq!(
            classify(
                "rpmfusion-nonfree-updates",
                "rpmfusion-nonfree-updates.repo"
            ),
            RepoKey::RfNonfreeUpdates
        );
        assert_eq!(
            classify(
                "rpmfusion-nonfree-updates-testing",
                "rpmfusion-nonfree-updates-testing.repo"
            ),
            RepoKey::RfNonfreeTesting
        );
    }

    #[test]
    fn classifies_epel_family() {
        assert_eq!(classify("epel", "epel.repo"), RepoKey::Epel);
        assert_eq!(
            classify("epel-testing", "epel-testing.repo"),
            RepoKey::EpelTesting
        );
        assert_eq!(
            classify("epel-modular", "epel-modular.repo"),
            RepoKey::EpelModular
        );
    }

    #[test]
    fn classifies_cisco_and_skips_specials() {
        assert_eq!(
            classify("fedora-cisco-openh264", "fedora-cisco-openh264.repo"),
            RepoKey::Cisco
        );
        assert_eq!(
            classify("epel-cisco-openh264", "epel-cisco-openh264.repo"),
            RepoKey::Cisco
        );
        // A COPR id contains a fedora substring but must stay untouched.
        assert_eq!(
            classify("user-app", "_copr:copr.fedorainfracloud.org:user-app.repo"),
            RepoKey::Skip
        );
        assert_eq!(classify("fedora-source", "fedora.repo"), RepoKey::Skip);
        assert_eq!(
            classify("updates-debuginfo", "fedora-updates.repo"),
            RepoKey::Skip
        );
        assert_eq!(
            classify("google-chrome", "google-chrome.repo"),
            RepoKey::Skip
        );
        assert_eq!(classify("rhel-9-baseos", "redhat.repo"), RepoKey::Skip);
        // Dots are normalized to spaces: "example.org" must not read as a key.
        assert_eq!(classify("mirrors.example.org", "third.repo"), RepoKey::Skip);
    }

    #[test]
    fn switchability_follows_key() {
        assert_eq!(switchability(RepoKey::Fedora), Switchability::Switchable);
        assert_eq!(
            switchability(RepoKey::EpelTesting),
            Switchability::Switchable
        );
        assert_eq!(switchability(RepoKey::Cisco), Switchability::Cisco);
        assert_eq!(switchability(RepoKey::Skip), Switchability::NotSwitchable);
    }
}
