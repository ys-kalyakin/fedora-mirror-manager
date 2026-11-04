# Fedora Mirror Manager

Desktop utility for managing Fedora dnf repository mirrors: discover available
mirrors, compare them, and switch repositories between `metalink` and `baseurl`
sources directly from the UI.

Status: **early feature stage**. The UI lists the repositories from
`/etc/yum.repos.d`, switches them between the Yandex and HyperDedic mirrors,
and rolls back via snapshots. Mirror discovery and comparison are planned as
follow-up changes — see [`openspec/`](openspec/) for specs and the change log.

Stack: Rust workspace + [Tauri 2](https://v2.tauri.app/) (Vite + TypeScript UI).

## Prerequisites

- Rust stable via [rustup](https://rustup.rs)
- Node.js ≥ 20 and npm
- Tauri 2 system dependencies (Fedora):

  ```sh
  sudo dnf install webkit2gtk4.1-devel gtk3-devel glib2-devel dbus-devel librsvg2-devel libsoup3-devel
  ```

## Development

From the repository root:

```sh
npm install --prefix ui   # first time only — installs frontend dependencies
npm run tauri dev
```

## Release build

```sh
npm run tauri build
```

## Tests and lints

```sh
cargo test
cargo clippy --workspace -- -D warnings
cd ui && npm run build
```

## Privileged helper installation

Reading `/etc/yum.repos.d` needs no privileges, but switching mirrors and
restoring snapshots does. The app performs privileged work through the
`fmm-apply` helper binary invoked with `pkexec` (polkit). Until the helper is
installed, the app works as a read-only viewer and shows a banner explaining
what is missing.

### Option A — in-app setup (recommended)

Build the workspace and launch the app; the repositories panel shows a
banner with an **Install helper** button. Clicking it installs both the
helper and the polkit policy via two polkit authorization prompts — no manual
`sudo` commands needed. The action is idempotent: click it again after
rebuilding the helper to reinstall the fresh binary.

```sh
cargo build -p fmm-apply   # the app picks up target/debug/fmm-apply next to its own binary
npm run tauri dev          # then click "Install helper" in the banner
```

### Option B — manual installation

From the repository root:

```sh
cargo build -p fmm-apply
sudo install -D -m 0755 -o root -g root target/debug/fmm-apply /usr/libexec/fmm-apply
sudo install -D -m 0644 -o root -g root packaging/polkit/org.fmm.desktop.apply.policy /usr/share/polkit-1/actions/org.fmm.desktop.apply.policy
```

For a release build, use `cargo build --release -p fmm-apply` and install
`target/release/fmm-apply` instead. Re-install after every rebuild of the
helper — `pkexec` refuses to run binaries that are not owned by root.

### Helper path override

Override the helper path for development and tests:

```sh
FMM_APPLY_HELPER=/path/to/fmm-apply npm run tauri dev
```

## Project structure

```
package.json       root npm entry: `npm run tauri …` (the Tauri CLI must run from the root)
ui/                frontend (Vite + TypeScript, no framework yet)
src-tauri/         crate "fmm-app": Tauri shell, commands, window setup
crates/fmm-core/   crate "fmm-core": domain models, .repo parsing, plans, snapshots
crates/fmm-apply/  crate "fmm-apply": privileged root helper (pkexec target)
packaging/polkit/  polkit action policy for the helper
openspec/          OpenSpec planning home: specs, changes, archive
```

Domain logic lives in `fmm-core` and stays independent of the UI framework;
`src-tauri` is a thin adapter (commands and state) between the frontend and
the domain crate. `fmm-apply` receives a computed plan as JSON on stdin,
snapshots `/etc/yum.repos.d` before every write, verifies the plan against
the on-disk state, and reports back as JSON on stdout.

## Troubleshooting

| Symptom | What to do |
|---|---|
| "The privileged helper is not installed" | Click **Install helper** in the banner (or in the error dialog), or use *Privileged helper installation* in this README |
| "Not authorized — the request was dismissed" | The polkit dialog was cancelled; nothing was changed, retry |
| "The repositories changed on disk since the preview" | Another tool modified `/etc/yum.repos.d` — press Refresh and plan again |
| Cache refresh failed after an apply | The changes are in place; check the reported `dnf` output tail |

Domain logic lives in `fmm-core` and stays independent of the UI framework;
`src-tauri` is a thin adapter (commands and state) between the frontend and
the domain crate.
