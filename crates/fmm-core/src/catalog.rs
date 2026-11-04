//! Fixed mirror catalog: per-family `baseurl` templates for the two mirrors
//! supported by the reference implementation. All URLs use literal
//! `$releasever` and `$basearch` — dnf substitutes them.

use serde::{Deserialize, Serialize};

use crate::classify::RepoKey;
use crate::reposd::Source;

/// The mirrors known to the application, in offer order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mirror {
    Yandex,
    Hyperdedic,
}

impl Mirror {
    /// All mirrors, in the order they are offered in the UI.
    pub const ALL: [Mirror; 2] = [Mirror::Yandex, Mirror::Hyperdedic];

    /// Root path of the mirror tree; family paths are appended per key.
    pub fn root(self) -> &'static str {
        match self {
            Mirror::Yandex => "https://mirror.yandex.ru/fedora",
            Mirror::Hyperdedic => "https://mirror.hyperdedic.ru/fedora",
        }
    }

    /// Human-readable name for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Mirror::Yandex => "Yandex",
            Mirror::Hyperdedic => "HyperDedic",
        }
    }

    /// Lowercase slug used in snapshot tags (`switch-updates-yandex`).
    pub fn slug(self) -> &'static str {
        match self {
            Mirror::Yandex => "yandex",
            Mirror::Hyperdedic => "hyperdedic",
        }
    }

    /// Source classification a repository carries while pointing at this mirror.
    pub fn source(self) -> Source {
        match self {
            Mirror::Yandex => Source::Yandex,
            Mirror::Hyperdedic => Source::Hyperdedic,
        }
    }
}

/// `baseurl` of the official OpenH264 codecs mirror (cisco repositories).
pub const CISCO_CODECS_BASEURL: &str =
    "https://codecs.fedoraproject.org/openh264/$releasever/$basearch/";

/// Returns the `baseurl` for `mirror` and `key`, or `None` when the mirror
/// does not carry that family (HyperDedic mirrors only Fedora and EPEL).
pub fn baseurl(mirror: Mirror, key: RepoKey) -> Option<String> {
    let p = mirror.root();
    let url = match key {
        RepoKey::Fedora => format!("{p}/linux/releases/$releasever/Everything/$basearch/os/"),
        RepoKey::FedoraModular => format!("{p}/linux/releases/$releasever/Modular/$basearch/os/"),
        RepoKey::Updates => format!("{p}/linux/updates/$releasever/Everything/$basearch/"),
        RepoKey::UpdatesModular => format!("{p}/linux/updates/$releasever/Modular/$basearch/"),
        RepoKey::UpdatesTesting => {
            format!("{p}/linux/updates/testing/$releasever/Everything/$basearch/")
        }
        RepoKey::UpdatesTestingModular => {
            format!("{p}/linux/updates/testing/$releasever/Modular/$basearch/")
        }
        RepoKey::RfFree => {
            format!("{p}/rpmfusion/free/fedora/releases/$releasever/Everything/$basearch/os/")
        }
        RepoKey::RfFreeUpdates => {
            format!("{p}/rpmfusion/free/fedora/updates/$releasever/$basearch/")
        }
        RepoKey::RfFreeTesting => {
            format!("{p}/rpmfusion/free/fedora/updates/testing/$releasever/$basearch/")
        }
        RepoKey::RfNonfree => {
            format!("{p}/rpmfusion/nonfree/fedora/releases/$releasever/Everything/$basearch/os/")
        }
        RepoKey::RfNonfreeUpdates => {
            format!("{p}/rpmfusion/nonfree/fedora/updates/$releasever/$basearch/")
        }
        RepoKey::RfNonfreeTesting => {
            format!("{p}/rpmfusion/nonfree/fedora/updates/testing/$releasever/$basearch/")
        }
        RepoKey::Epel => format!("{p}/epel/$releasever/Everything/$basearch/"),
        RepoKey::EpelTesting => format!("{p}/epel/testing/$releasever/Everything/$basearch/"),
        RepoKey::EpelModular => format!("{p}/epel/$releasever/Modular/$basearch/"),
        RepoKey::Cisco | RepoKey::Skip => return None,
    };
    if mirror == Mirror::Hyperdedic && is_rpmfusion(key) {
        return None;
    }
    Some(url)
}

/// True for RPM Fusion families (HyperDedic mirrors only Fedora and EPEL).
pub fn is_rpmfusion(key: RepoKey) -> bool {
    matches!(
        key,
        RepoKey::RfFree
            | RepoKey::RfFreeUpdates
            | RepoKey::RfFreeTesting
            | RepoKey::RfNonfree
            | RepoKey::RfNonfreeUpdates
            | RepoKey::RfNonfreeTesting
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The full key→URL mapping must equal the values validated by the
    /// reference implementation (mirror-switch.sh, `baseurl_for`).
    #[test]
    fn yandex_mapping_matches_reference() {
        let y = Mirror::Yandex.root();
        let cases = [
            (
                RepoKey::Fedora,
                format!("{y}/linux/releases/$releasever/Everything/$basearch/os/"),
            ),
            (
                RepoKey::FedoraModular,
                format!("{y}/linux/releases/$releasever/Modular/$basearch/os/"),
            ),
            (
                RepoKey::Updates,
                format!("{y}/linux/updates/$releasever/Everything/$basearch/"),
            ),
            (
                RepoKey::UpdatesModular,
                format!("{y}/linux/updates/$releasever/Modular/$basearch/"),
            ),
            (
                RepoKey::UpdatesTesting,
                format!("{y}/linux/updates/testing/$releasever/Everything/$basearch/"),
            ),
            (
                RepoKey::UpdatesTestingModular,
                format!("{y}/linux/updates/testing/$releasever/Modular/$basearch/"),
            ),
            (
                RepoKey::RfFree,
                format!("{y}/rpmfusion/free/fedora/releases/$releasever/Everything/$basearch/os/"),
            ),
            (
                RepoKey::RfFreeUpdates,
                format!("{y}/rpmfusion/free/fedora/updates/$releasever/$basearch/"),
            ),
            (
                RepoKey::RfFreeTesting,
                format!("{y}/rpmfusion/free/fedora/updates/testing/$releasever/$basearch/"),
            ),
            (
                RepoKey::RfNonfree,
                format!(
                    "{y}/rpmfusion/nonfree/fedora/releases/$releasever/Everything/$basearch/os/"
                ),
            ),
            (
                RepoKey::RfNonfreeUpdates,
                format!("{y}/rpmfusion/nonfree/fedora/updates/$releasever/$basearch/"),
            ),
            (
                RepoKey::RfNonfreeTesting,
                format!("{y}/rpmfusion/nonfree/fedora/updates/testing/$releasever/$basearch/"),
            ),
            (
                RepoKey::Epel,
                format!("{y}/epel/$releasever/Everything/$basearch/"),
            ),
            (
                RepoKey::EpelTesting,
                format!("{y}/epel/testing/$releasever/Everything/$basearch/"),
            ),
            (
                RepoKey::EpelModular,
                format!("{y}/epel/$releasever/Modular/$basearch/"),
            ),
        ];
        for (key, url) in cases {
            assert_eq!(
                baseurl(Mirror::Yandex, key).as_deref(),
                Some(url.as_str()),
                "key {key:?}"
            );
        }
    }

    #[test]
    fn hyperdedic_serves_fedora_and_epel_only() {
        let h = Mirror::Hyperdedic.root();
        assert_eq!(
            baseurl(Mirror::Hyperdedic, RepoKey::Fedora).as_deref(),
            Some(format!("{h}/linux/releases/$releasever/Everything/$basearch/os/").as_str())
        );
        assert_eq!(
            baseurl(Mirror::Hyperdedic, RepoKey::Epel).as_deref(),
            Some(format!("{h}/epel/$releasever/Everything/$basearch/").as_str())
        );
        // RPM Fusion is not mirrored on HyperDedic.
        for key in [
            RepoKey::RfFree,
            RepoKey::RfFreeUpdates,
            RepoKey::RfNonfree,
            RepoKey::RfNonfreeTesting,
        ] {
            assert_eq!(baseurl(Mirror::Hyperdedic, key), None, "key {key:?}");
        }
        // Cisco and skip keys have no mirror URL at all.
        for key in [RepoKey::Cisco, RepoKey::Skip] {
            for mirror in Mirror::ALL {
                assert_eq!(baseurl(mirror, key), None);
            }
        }
    }

    #[test]
    fn cisco_codecs_url_matches_reference() {
        assert_eq!(
            CISCO_CODECS_BASEURL,
            "https://codecs.fedoraproject.org/openh264/$releasever/$basearch/"
        );
    }
}
