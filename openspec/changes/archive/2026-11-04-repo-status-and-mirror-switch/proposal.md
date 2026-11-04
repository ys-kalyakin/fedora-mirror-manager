# Proposal

## Why

The application is currently an empty shell: it shows nothing about the system's dnf repositories and cannot change them. The reference implementation (`mirror-switch.sh`, a separate bash script) already proves the domain logic — classify sections of `/etc/yum.repos.d/*.repo`, identify their current source, and switch them between Russian mirrors — but requires a terminal and manual sudo. Bringing this into the desktop UI is the first real feature of the product.

## What Changes

- **Repository status view**: parse `/etc/yum.repos.d/*.repo` and display every repository section with its id, name, enabled flag, current source (`yandex` / `hyperdedic` / `metalink` / none), and URL, with manual refresh. Read-only; requires no privileges.
- **Per-repository mirror switching**: each manageable repository row (Fedora, updates, RPM Fusion free/nonfree, EPEL families) gets a mirror choice — `mirror.yandex.ru` or `mirror.hyperdedic.ru` — applied by writing a literal `baseurl` and removing `metalink`/`mirrorlist`/duplicate `baseurl` lines for that section only. Cisco OpenH264 repositories get a dedicated choice: `disable` / `codecs.fedoraproject.org` / `keep`. Third-party, source, debuginfo, playground and COPR repositories are shown but marked as not switchable.
- **Snapshots and rollback**: before every privileged write, the whole `/etc/yum.repos.d` is snapshotted to `/etc/yum.repos.d.bak` (10 most recent kept, mirroring the reference script). The UI lists snapshots and can restore `/etc/yum.repos.d` from a chosen snapshot — including the original pre-switch state.
- **Privileged execution via polkit**: writes (switch, snapshot, restore, optional `dnf clean all` + `dnf makecache`) run through a small root helper invoked with `pkexec`; the app itself stays unprivileged and previews the planned changes before asking for authorization.
- **Optional cache refresh**: a checkbox on switching/restore actions controls whether `dnf clean all` and `dnf makecache` run afterwards (default: on).

User decisions recorded during proposal: switch + restore in scope (full parity with the reference script); snapshot list exposed in the UI; Cisco mode and cache refresh exposed in the UI; switching granularity is per repository. Assumptions: the mirror catalog is fixed (Yandex, HyperDedic) as in the reference script; UI strings stay in English (localization is a later change).

## Capabilities

### New Capabilities

- `repo-status`: Read-only inventory of dnf repositories — parsing and classifying `/etc/yum.repos.d/*.repo`, reporting each section's enabled flag and current source (mirror vs metalink vs none), and refreshing the view.
- `mirror-switch`: Switching individual repositories between the fixed set of mirrors (Yandex, HyperDedic) and Cisco OpenH264 modes, with preview of planned changes, polkit-authorized privileged application, and optional dnf cache refresh.
- `snapshot-rollback`: Automatic snapshots of `/etc/yum.repos.d` before privileged writes, listing them in the UI, and restoring the directory byte-for-byte from a chosen snapshot.

### Modified Capabilities

- (none — only `app-shell` exists, defined by the in-flight `bootstrap-project-structure` change; no requirement of it changes, though the mirror-list placeholder area of the shell is where the status view lands)

## Impact

- **`crates/fmm-core`** (major): new domain modules — `.repo` file parser (section-preserving read and per-section rewrite), repository classifier, fixed mirror catalog, snapshot manager, apply-plan model and validation; extended `error.rs`.
- **`src-tauri` (`fmm-app`)**: new Tauri commands (repository list, snapshot list, apply plan) and mapping of domain results to the frontend.
- **New `crates/fmm-apply`** helper binary: root-side executor invoked via `pkexec` (action plan in, JSON report out); polkit action policy file; packaging/install notes in the README.
- **`ui/`**: repository table view with per-row actions, mirror-choice controls, Cisco mode controls, snapshot panel with restore, cache-refresh checkbox, loading/error states.
- **Dependencies**: no new Rust crates required (`pkexec` is external); frontend unchanged framework-wise.
- **Risks**: wrong writes to `/etc/yum.repos.d` can break package management — mitigated by preview-before-apply and unconditional pre-write snapshots; polkit dialogs cannot be exercised in headless tests — accepted as manual verification.
