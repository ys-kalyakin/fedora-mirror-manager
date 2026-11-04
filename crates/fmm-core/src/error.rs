//! Shared error type for domain operations.

use std::path::PathBuf;

/// Domain error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A requested entity was not found.
    #[error("not found: {0}")]
    NotFound(String),
    /// An I/O operation on `path` failed.
    #[error("io error for {}: {}", path.display(), source)]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A `.repo` file could not be parsed.
    #[error("cannot parse {}: {}", file.display(), reason)]
    Parse { file: PathBuf, reason: String },
    /// The user asked to switch a repository that cannot be switched.
    #[error("cannot switch {}: {}", id, reason)]
    NotSwitchable { id: String, reason: String },
    /// A plan no longer matches the on-disk state it was built from.
    #[error("stale plan: {0}")]
    StalePlan(String),
    /// A snapshot operation failed.
    #[error("snapshot error: {0}")]
    Snapshot(String),
    /// The privileged helper reported a failure.
    #[error("helper error ({}): {}", code, message)]
    Helper { code: String, message: String },
}

/// Convenient result alias for domain operations.
pub type Result<T> = std::result::Result<T, Error>;
