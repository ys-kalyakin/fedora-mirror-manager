//! Parsing of dnf `.repo` files from the repository locations of the detected
//! layout: `/etc/yum.repos.d` alone (legacy), or `/etc/yum.repos.d` plus the
//! packaged `/usr/share/dnf5/repos.d` (Fedora 45+).
//!
//! The parser is line-preserving: raw lines are kept so that a rewrite can
//! touch only the target section and leave every other byte untouched
//! (mirroring the reference implementation's rewriter).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::classify::{classify, switchability, RepoKey, Switchability};
use crate::error::{Error, Result};

/// Default repository directory: the only location on the legacy layout, the
/// user/third-party location on the F45+ layout.
pub const DEFAULT_REPO_DIR: &str = "/etc/yum.repos.d";
/// Packaged repository directory on the F45+ layout (Fedora change
/// "Relocate RPM Repo Configs to /usr").
pub const PACKAGED_REPO_DIR: &str = "/usr/share/dnf5/repos.d";
/// Default snapshot root.
pub const DEFAULT_SNAPSHOT_ROOT: &str = "/etc/yum.repos.d.bak";
/// Files that are never rewritten (managed by the subscription tooling).
pub const SKIP_FILES: [&str; 1] = ["redhat.repo"];

/// Repository layout of the host, detected from the filesystem (never from
/// the OS version).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// Fedora ≤ 44: packaged and user repositories share `/etc/yum.repos.d`.
    Legacy,
    /// Fedora 45+: packaged repositories live in `/usr/share/dnf5/repos.d`
    /// and `/etc/yum.repos.d` keeps user and third-party files.
    F45Plus,
}

/// Repository locations of one layout.
///
/// Reading follows dnf5 semantics (its default `reposdir` list is
/// `[/etc/yum.repos.d, /etc/distro.repos.d, /usr/share/dnf5/repos.d]`, and the
/// first directory that defines a section id owns the whole section): when the
/// same section id is defined in both directories, the local definition wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoLocations {
    /// The layout these locations were resolved for.
    pub layout: Layout,
    /// The directory the application writes to (`/etc/yum.repos.d`).
    pub local_dir: PathBuf,
    /// The packaged directory on the F45+ layout (`/usr/share/dnf5/repos.d`).
    pub packaged_dir: Option<PathBuf>,
}

impl RepoLocations {
    /// Locations of the legacy layout with `dir` as the only repo directory.
    pub fn legacy(dir: impl Into<PathBuf>) -> Self {
        Self { layout: Layout::Legacy, local_dir: dir.into(), packaged_dir: None }
    }

    /// Locations of the F45+ layout.
    pub fn f45_plus(packaged_dir: impl Into<PathBuf>, local_dir: impl Into<PathBuf>) -> Self {
        Self { layout: Layout::F45Plus, local_dir: local_dir.into(), packaged_dir: Some(packaged_dir.into()) }
    }

    /// Detects the layout of the running host from the filesystem.
    pub fn detect() -> Self {
        match detect_layout() {
            Layout::F45Plus => Self::f45_plus(PACKAGED_REPO_DIR, DEFAULT_REPO_DIR),
            Layout::Legacy => Self::legacy(DEFAULT_REPO_DIR),
        }
    }

    /// Directories to read in dnf5 precedence order (first wins per section id).
    pub fn read_dirs(&self) -> Vec<&Path> {
        match &self.packaged_dir {
            Some(packaged) => vec![self.local_dir.as_path(), packaged.as_path()],
            None => vec![self.local_dir.as_path()],
        }
    }

    /// True when `dir` is the packaged location of the F45+ layout.
    pub fn is_packaged(&self, dir: &Path) -> bool {
        self.packaged_dir.as_deref() == Some(dir)
    }
}

/// Detects the layout from the filesystem: F45+ iff the packaged directory
/// contains at least one `*.repo` file.
pub fn detect_layout() -> Layout {
    detect_layout_in(Path::new(PACKAGED_REPO_DIR))
}

/// [`detect_layout`] over an explicit directory (testable).
pub fn detect_layout_in(packaged_dir: &Path) -> Layout {
    if list_repo_files(packaged_dir).is_ok_and(|files| !files.is_empty()) {
        Layout::F45Plus
    } else {
        Layout::Legacy
    }
}

/// Kind of the URL line that defines where a repository fetches metadata from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlKind {
    /// `baseurl=`
    BaseUrl,
    /// `metalink=`
    Metalink,
    /// `mirrorlist=`
    MirrorList,
}

/// Source classification of a repository section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Yandex,
    Hyperdedic,
    Metalink,
    Other,
    None,
}

/// A `[section]` of a `.repo` file with the values the domain cares about.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section id from the `[id]` header.
    pub id: String,
    /// Index of the header line in the file's line list.
    pub header_line: usize,
    /// Exclusive end: index of the next header line or the line count.
    pub end_line: usize,
    /// `enabled=` value of the section (`dnf` defaults to enabled when absent).
    pub enabled: Option<String>,
    /// First uncommented URL line in file order.
    pub url: Option<(UrlKind, String)>,
}

/// A parsed `.repo` file.
#[derive(Debug, Clone)]
pub struct RepoFile {
    /// Absolute path of the file.
    pub path: PathBuf,
    lines: Vec<String>,
    sections: Vec<Section>,
}

impl RepoFile {
    /// Reads and parses the `.repo` file at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
        let text = String::from_utf8(bytes).map_err(|_| Error::Parse {
            file: path.to_path_buf(),
            reason: "file is not valid UTF-8".to_string(),
        })?;
        Ok(Self::from_text(path.to_path_buf(), &text))
    }

    /// Parses the file content of `path` from raw text.
    pub fn from_text(path: PathBuf, text: &str) -> Self {
        let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let sections = parse_sections(&lines);
        Self { path, lines, sections }
    }

    /// The raw lines of the file (without newline characters).
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The parsed sections, in file order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Finds a section by its id.
    pub fn section(&self, id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Joins the lines back into file content (newline-terminated, as stored).
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }

    /// Replaces the URL lines of `section_id` with a new `baseurl` placed
    /// directly under the header and/or sets `enabled`. Commented URL lines
    /// are removed as well; every other line is untouched, matching the
    /// reference rewriter.
    pub fn rewrite_section(&mut self, section_id: &str, baseurl: Option<&str>, set_enabled: Option<&str>) {
        let Some(sec) = self.section(section_id) else { return };
        let (header_line, end_line) = (sec.header_line, sec.end_line);
        let mut out: Vec<String> = Vec::with_capacity(self.lines.len() + 2);
        for (i, line) in self.lines.iter().enumerate() {
            if i == header_line {
                out.push(line.clone());
                if let Some(b) = baseurl {
                    out.push(format!("baseurl={b}"));
                }
                if let Some(e) = set_enabled {
                    out.push(format!("enabled={e}"));
                }
                continue;
            }
            if i > header_line && i < end_line {
                if baseurl.is_some() && is_url_line(line) {
                    continue;
                }
                if set_enabled.is_some() && is_enabled_line(line) {
                    continue;
                }
            }
            out.push(line.clone());
        }
        self.lines = out;
        // Header indexes shifted: re-derive the section ranges.
        self.sections = parse_sections(&self.lines);
    }
}

/// True for `^[[:space:]]*#?[[:space:]]*(baseurl|metalink|mirrorlist)[[:space:]]*=`.
pub fn is_url_line(line: &str) -> bool {
    line_matches_any_key(line, &["baseurl", "metalink", "mirrorlist"])
}

/// True for `^[[:space:]]*#?[[:space:]]*enabled[[:space:]]*=`.
pub fn is_enabled_line(line: &str) -> bool {
    line_matches_any_key(line, &["enabled"])
}

fn line_matches_any_key(line: &str, keys: &[&str]) -> bool {
    let t = line.trim_start();
    let t = t.strip_prefix('#').unwrap_or(t).trim_start();
    keys.iter().any(|key| t.strip_prefix(key).is_some_and(|rest| rest.trim_start().starts_with('=')))
}

/// Section id for a `[id]` header line.
fn section_id(line: &str) -> Option<&str> {
    let t = line.trim();
    if t.starts_with('[') && t.ends_with(']') && t.len() >= 2 {
        Some(t[1..t.len() - 1].trim())
    } else {
        None
    }
}

/// Parses an uncommented `key = value` line (key lowercased).
fn key_value(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with(';') {
        return None;
    }
    let eq = t.find('=')?;
    let key = t[..eq].trim().to_ascii_lowercase();
    if key.is_empty() {
        return None;
    }
    Some((key, t[eq + 1..].trim().to_string()))
}

fn parse_sections(lines: &[String]) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(id) = section_id(line) {
            sections.push(Section {
                id: id.to_string(),
                header_line: i,
                end_line: lines.len(),
                enabled: None,
                url: None,
            });
            continue;
        }
        if let Some((key, value)) = key_value(line).filter(|_| sections.last_mut().is_some()) {
            // Unwrap is safe: the filter above verified a current section exists.
            let current = sections.last_mut().unwrap();
            match key.as_str() {
                "enabled" => {
                    current.enabled.get_or_insert(value);
                }
                "baseurl" | "metalink" | "mirrorlist" if current.url.is_none() => {
                    let kind = match key.as_str() {
                        "baseurl" => UrlKind::BaseUrl,
                        "metalink" => UrlKind::Metalink,
                        _ => UrlKind::MirrorList,
                    };
                    current.url = Some((kind, value));
                }
                _ => {}
            }
        }
    }
    // Close the ranges: each section ends where the next header begins.
    for n in 0..sections.len() {
        let end = sections.get(n + 1).map_or(lines.len(), |next| next.header_line);
        sections[n].end_line = end;
    }
    sections
}

/// Classifies a section's current source from its first URL line. A
/// `metalink`/`mirrorlist` line always reports [`Source::Metalink`]; a
/// `baseurl` is matched against the known mirror hosts.
pub fn classify_source(url: Option<&(UrlKind, String)>) -> Source {
    match url {
        None => Source::None,
        Some((UrlKind::BaseUrl, value)) => {
            if value.contains("mirror.yandex.ru") {
                Source::Yandex
            } else if value.contains("mirror.hyperdedic.ru") {
                Source::Hyperdedic
            } else if value.is_empty() {
                Source::None
            } else {
                Source::Other
            }
        }
        Some(_) => Source::Metalink,
    }
}

/// One successfully parsed repository section, as shown in the UI.
#[derive(Debug, Clone, Serialize)]
pub struct RepoEntry {
    /// Section id, e.g. `updates`.
    pub id: String,
    /// `name=` value, falling back to the id.
    pub name: String,
    /// Base name of the `.repo` file the section comes from.
    pub file: String,
    /// Absolute path of the `.repo` file the section comes from.
    pub path: String,
    /// Where the effective definition comes from.
    pub origin: Origin,
    /// `dnf` semantics: absent or non-zero `enabled` means enabled.
    pub enabled: bool,
    /// Current source classification.
    pub source: Source,
    /// First URL line value, if any.
    pub url: Option<String>,
    /// Domain key of the section.
    pub kind: RepoKey,
    /// Whether (and how) the user may act on this repository.
    pub switchability: Switchability,
}

/// Where the effective definition of a section comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// Defined by a file under the packaged location (`/usr/share/dnf5/repos.d`).
    Packaged,
    /// Defined by a file under `/etc/yum.repos.d` (always on the legacy layout).
    Local,
}

/// A per-file failure reported alongside successfully parsed entries.
#[derive(Debug, Clone, Serialize)]
pub struct FileError {
    /// File the failure relates to (base name).
    pub file: String,
    /// Failure description.
    pub message: String,
}

/// Inventory of the repository locations: entries plus per-file errors.
#[derive(Debug, Clone, Serialize)]
pub struct Inventory {
    /// The layout the inventory was read for.
    pub layout: Layout,
    /// Parsed sections of all readable `*.repo` files, grouped by file.
    pub entries: Vec<RepoEntry>,
    /// Files that could not be read or parsed.
    pub errors: Vec<FileError>,
}

/// Reads every `*.repo` file in the locations of `locations` and builds the
/// merged inventory.
///
/// Directories are read in dnf5 precedence order (local first, packaged
/// second): when a section id is defined in both, the local definition wins
/// and the packaged one is not listed. Duplicate ids within one directory are
/// all listed, so plan building can reject the ambiguity. A directory that
/// exists but has no `*.repo` files contributes nothing; when every directory
/// is missing the inventory is an error, matching the legacy behavior for a
/// missing `/etc/yum.repos.d`.
pub fn inventory(locations: &RepoLocations) -> Result<Inventory> {
    let dirs = locations.read_dirs();
    if dirs.iter().all(|dir| !dir.is_dir()) {
        let names = dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ");
        return Err(Error::NotFound(format!(
            "repository director{} {}",
            if dirs.len() == 1 { "y" } else { "ies" },
            names
        )));
    }
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    // Section ids defined by earlier directories: their packaged duplicates
    // are skipped (the local definition is the effective one). Duplicates
    // within one directory stay listed so plan building can reject them.
    let mut claimed: HashSet<String> = HashSet::new();
    for dir in dirs {
        let mut dir_ids: HashSet<String> = HashSet::new();
        for path in list_repo_files(dir)? {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            match RepoFile::load(&path) {
                Ok(repo_file) => {
                    for section in repo_file.sections() {
                        if claimed.contains(&section.id) {
                            continue;
                        }
                        dir_ids.insert(section.id.clone());
                        entries.push(RepoEntry {
                            id: section.id.clone(),
                            name: name_of(&repo_file, section),
                            file: file_name.clone(),
                            path: path.to_string_lossy().into_owned(),
                            origin: if locations.is_packaged(dir) { Origin::Packaged } else { Origin::Local },
                            enabled: is_enabled(section),
                            source: classify_source(section.url.as_ref()),
                            url: section.url.as_ref().map(|(_, v)| v.clone()),
                            kind: classify(&section.id, &file_name),
                            switchability: switchability(classify(&section.id, &file_name)),
                        });
                    }
                }
                Err(err) => errors.push(FileError { file: file_name, message: err.to_string() }),
            }
        }
        claimed.extend(dir_ids);
    }
    Ok(Inventory { layout: locations.layout, entries, errors })
}

/// Sorted `*.repo` file paths in `dir`. A missing directory contributes no
/// files; an existing but unreadable directory is an error.
fn list_repo_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let read = fs::read_dir(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
    for entry in read {
        let entry = entry.map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "repo") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn name_of(repo_file: &RepoFile, section: &crate::reposd::Section) -> String {
    repo_file
        .lines()
        .get(section.header_line + 1..section.end_line)
        .and_then(|lines| {
            lines.iter().find_map(|line| {
                key_value(line).and_then(|(k, v)| (k == "name").then_some(v))
            })
        })
        .unwrap_or_else(|| section.id.clone())
}

fn is_enabled(section: &crate::reposd::Section) -> bool {
    match section.enabled.as_deref() {
        // dnf: any value other than 0 (or garbage) enables the repository.
        None => true,
        Some(v) => v != "0",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[fedora]\n\
name=Fedora $releasever - $basearch\n\
#baseurl=http://download.example/pub/fedora/linux/releases/$releasever/Everything/$basearch/os/\n\
metalink=https://mirrors.fedoraproject.org/metalink?repo=fedora-$releasever&arch=$basearch\n\
enabled=1\n\
gpgcheck=1\n\
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-$releasever-$basearch\n\
\n\
[updates]\n\
name=Fedora $releasever - $basearch - Updates\n\
metalink=https://mirrors.fedoraproject.org/metalink?repo=updates-released-f$releasever&arch=$basearch\n\
enabled=0\n";

    #[test]
    fn parses_sections_and_values() {
        let f = RepoFile::from_text(PathBuf::from("fedora.repo"), SAMPLE);
        assert_eq!(f.sections().len(), 2);
        let fedora = f.section("fedora").unwrap();
        assert_eq!(fedora.enabled.as_deref(), Some("1"));
        assert_eq!(
            fedora.url.as_ref().unwrap(),
            &(UrlKind::Metalink, "https://mirrors.fedoraproject.org/metalink?repo=fedora-$releasever&arch=$basearch".to_string())
        );
        assert_eq!(f.section("updates").unwrap().enabled.as_deref(), Some("0"));
        // Line-preserving round-trip.
        assert_eq!(f.render(), SAMPLE);
    }

    #[test]
    fn classifies_sources() {
        let f = RepoFile::from_text(PathBuf::from("x.repo"), SAMPLE);
        assert_eq!(classify_source(f.section("fedora").unwrap().url.as_ref()), Source::Metalink);

        let yandex = "[u]\nbaseurl=https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/\n";
        let f = RepoFile::from_text(PathBuf::from("x.repo"), yandex);
        assert_eq!(classify_source(f.section("u").unwrap().url.as_ref()), Source::Yandex);

        let hyper = "[u]\nbaseurl=https://mirror.hyperdedic.ru/fedora/epel/$releasever/Everything/$basearch/\n";
        let f = RepoFile::from_text(PathBuf::from("x.repo"), hyper);
        assert_eq!(classify_source(f.section("u").unwrap().url.as_ref()), Source::Hyperdedic);

        let foreign = "[u]\nbaseurl=https://dl.google.com/linux/chrome/rpm/stable/x86_64\n";
        let f = RepoFile::from_text(PathBuf::from("x.repo"), foreign);
        assert_eq!(classify_source(f.section("u").unwrap().url.as_ref()), Source::Other);

        let none = "[u]\nname=No URL\n";
        let f = RepoFile::from_text(PathBuf::from("x.repo"), none);
        assert_eq!(classify_source(f.section("u").unwrap().url.as_ref()), Source::None);
    }

    #[test]
    fn rewrite_replaces_url_lines_and_keeps_the_rest() {
        let mut f = RepoFile::from_text(PathBuf::from("fedora.repo"), SAMPLE);
        f.rewrite_section("fedora", Some("https://mirror.yandex.ru/fedora/linux/releases/$releasever/Everything/$basearch/os/"), None);
        let out = f.render();
        // The fedora section (everything before [updates]) carries the new
        // baseurl and none of its old URL lines.
        let fedora_block = out.split_once("[updates]").unwrap().0;
        assert!(fedora_block.contains("baseurl=https://mirror.yandex.ru/fedora/linux/releases/$releasever/Everything/$basearch/os/"));
        assert!(!fedora_block.contains("metalink="));
        assert!(!fedora_block.contains("#baseurl="));
        // gpgcheck, gpgkey, enabled and the updates section are untouched.
        assert!(out.contains("gpgcheck=1"));
        assert!(out.contains("gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-fedora-$releasever-$basearch"));
        assert!(out.contains("enabled=1\n"));
        assert!(out.contains("[updates]"));
        assert!(out.contains("repo=updates-released-f$releasever"));
    }

    #[test]
    fn rewrite_only_toggles_enabled_when_asked() {
        let mut f = RepoFile::from_text(PathBuf::from("c.repo"), "[c]\nname=x\nenabled=1\nbaseurl=https://example.org/repo/\n");
        // Cisco disable: enabled=0, URL lines stay.
        f.rewrite_section("c", None, Some("0"));
        let out = f.render();
        assert!(out.contains("enabled=0"));
        assert!(out.contains("baseurl=https://example.org/repo/"));
    }

    #[test]
    fn matches_commented_url_and_enabled_lines() {
        assert!(is_url_line("  #  baseurl=http://x/"));
        assert!(is_url_line("#metalink = https://x"));
        assert!(is_url_line("mirrorlist=https://x"));
        assert!(!is_url_line("# gpgcheck=1"));
        assert!(!is_url_line("baseurl_alt=x"));
        assert!(is_enabled_line(" enabled =0"));
        assert!(is_enabled_line("#enabled=1"));
        assert!(!is_enabled_line("enabled_flag=1"));
    }

    #[test]
    fn empty_value_counts_as_no_source() {
        let f = RepoFile::from_text(PathBuf::from("x.repo"), "[u]\nbaseurl=\nenabled=1\n");
        assert_eq!(classify_source(f.section("u").unwrap().url.as_ref()), Source::None);
    }
}
