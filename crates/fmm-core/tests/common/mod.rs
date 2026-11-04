//! Shared helpers for the fmm-core integration tests.

// The helpers are compiled once per test target; not every target uses all of them.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Fresh temp directory with every fixture `*.repo` file copied in.
pub fn fixture_repo_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("fmm-core-test-{}-{}", std::process::id(), n));
    fs::create_dir_all(&dir).expect("create temp dir");
    copy_fixtures(&dir);
    dir
}

/// Empty fresh temp directory.
pub fn empty_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("fmm-core-test-{}-{}", std::process::id(), n));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Copies the fixture `*.repo` files into `dir`.
pub fn copy_fixtures(dir: &Path) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in fs::read_dir(&fixtures).expect("fixtures dir") {
        let entry = entry.expect("fixture entry");
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".repo") {
            fs::copy(entry.path(), dir.join(&name)).expect("copy fixture");
        }
    }
}

/// Fresh F45-style directory pair: `packaged` holds every fixture `*.repo`
/// file (mirroring `/usr/share/dnf5/repos.d`), `local` starts empty.
pub fn fixture_f45_dirs() -> (PathBuf, PathBuf) {
    let packaged = empty_dir();
    copy_fixtures(&packaged);
    let local = empty_dir();
    (packaged, local)
}
