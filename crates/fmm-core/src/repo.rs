//! dnf repository configuration model (placeholder).
//!
//! Future work: parsing and writing `/etc/yum.repos.d/*.repo` entries,
//! switching between `metalink` and `baseurl` sources.

/// A single dnf repository definition (placeholder shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// Repository id, e.g. `fedora`.
    pub id: String,
    /// Human-readable name from the `[id]` section header.
    pub name: String,
}

impl Repo {
    /// Creates a placeholder repository definition.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_placeholder_repo() {
        let repo = Repo::new("fedora", "Fedora");
        assert_eq!(repo.id, "fedora");
        assert_eq!(repo.name, "Fedora");
    }
}
