//! Mirror model (placeholder).
//!
//! Future work: mirror discovery from metalink data, ranking and selection.

/// A single Fedora package mirror (placeholder shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mirror {
    /// Base URL of the mirror, e.g. `https://mirror.example.com/fedora/`.
    pub url: String,
    /// Country the mirror reports, if known.
    pub country: Option<String>,
}
