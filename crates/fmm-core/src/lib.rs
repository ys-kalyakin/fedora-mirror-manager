//! Domain library for Fedora Mirror Manager.
//!
//! Holds repository and mirror models plus mirror-management logic
//! (`.repo` parsing, classification, mirror catalog, plans, snapshots),
//! independent of any UI framework.

pub mod bulk;
pub mod catalog;
pub mod classify;
pub mod error;
pub mod mirror;
pub mod plan;
pub mod repo;
pub mod reposd;
pub mod snapshot;

/// Returns the crate version reported to the application shell.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty_and_numeric() {
        let v = version();
        assert!(!v.is_empty());
        assert!(
            v.split('.')
                .all(|part| part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty())
        );
    }
}
