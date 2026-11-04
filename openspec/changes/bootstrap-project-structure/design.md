# Design

## Context

Greenfield repository: only `openspec/` (planning home) and `.zcode/` exist. No Rust toolchain is installed on the machine yet; Node.js 22 + npm are present. The user has chosen **Tauri 2** for the UI stack and **polkit-driven editing of `/etc/yum.repos.d/*.repo`** as the eventual switching mechanism — that mechanism is *not* part of this change, but the structure must leave room for it.

## Goals / Non-Goals

**Goals:**

- A Cargo workspace whose domain logic is independent of the UI framework.
- A Tauri 2 shell that compiles and launches, satisfying the `app-shell` spec.
- A frontend scaffold with a clear place for future mirror-list UI.
- Project hygiene from day one: pinned toolchain, formatting/linting config, README, git history.

**Non-Goals:**

- No mirror discovery, ranking, or switching logic (later changes).
- No polkit policy/agent implementation (later change; structure only reserves a place for it).
- No packaging (RPM/Flatpak) or CI pipelines (later changes).
- No choice of frontend framework beyond a minimal Vite + TypeScript scaffold.

## Decisions

### D1. Cargo workspace with two members

```
fedora-mirror-manager/
├── Cargo.toml                  # [workspace] members, shared deps, profile
├── package.json                # root npm entry: `npm run tauri dev` (CLI needs root cwd)
├── rust-toolchain.toml         # channel = "stable"
├── rustfmt.toml
├── .gitignore
├── .editorconfig
├── README.md
├── ui/                         # frontend (Vite + TypeScript)
│   ├── package.json
│   ├── index.html
│   ├── tsconfig.json
│   ├── vite.config.ts
│   └── src/
│       ├── main.ts
│       ├── style.css
│       └── shell.ts            # shell bootstrap + status handshake
├── src-tauri/                  # crate "fmm-app" — Tauri shell
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/                  # Tauri default icon set
│   └── src/
│       ├── main.rs             # thin entry point
│       ├── lib.rs              # run() — builder, state, command registration
│       └── commands.rs         # status command (app version)
└── crates/
    └── fmm-core/               # crate "fmm-core" — domain library
        ├── Cargo.toml
        └── src/
            └── lib.rs          # placeholder modules: repo, mirror, error
```

- `fmm-core` holds all future domain logic (repo-file parsing, mirror model, polkit orchestration) as a plain library — unit-testable without any UI or windowing. **Alternative rejected:** a single crate with everything inside `src-tauri` — couples domain code to Tauri and makes headless testing of repo logic harder.
- `src-tauri` (crate name `fmm-app`) follows the Tauri convention so `tauri dev`/`tauri build` work unmodified; it stays a thin adapter: commands, state, error mapping.
- Workspace `Cargo.toml` centralizes dependency versions (`workspace.dependencies`) and sets a shared release profile.

### D2. Tauri 2 (user-selected)

Web-frontend UI backed by a Rust core; small binaries and mature tooling. Compared in AskUserQuestion against GTK4+Relm4, Iced, and egui. Consequence: Tauri 2 system dependencies on Fedora (`webkit2gtk4.1-devel`, `gtk3-devel`, `glib2-devel`, `dbus-devel` and friends) must be installed during apply before the first build.

### D3. Frontend: Vite + TypeScript, no framework yet

The shell needs one page and a status line; a framework adds nothing yet. Keeping `ui/` framework-free means introducing Svelte/React/Vue later is a contained change to `ui/` alone — the Tauri seam (`invoke` commands) does not change. **Alternative rejected:** scaffolding Svelte now — extra moving parts for a placeholder screen; deliberately deferred.

### D4. Naming and identifiers

- Product name: **Fedora Mirror Manager** (window title, README H1).
- Crates: `fmm-core`, `fmm-app`; workspace `fedora-mirror-manager`.
- Tauri bundle identifier: `org.fmm.desktop` (placeholder, revisited at packaging time; must not contain dashes).

### D5. Backend status handshake

One Tauri command `app_status()` returning `{ version }` (from `CARGO_PKG_VERSION`) proves the frontend→backend seam end to end, matching the `Backend status handshake` requirement. `fmm-core` exposes `pub fn version()` and stub modules `repo`, `mirror`, `error` so later changes extend an existing shape instead of inventing one.

### D6. Rust edition and toolchain

`rust-toolchain.toml` pins `stable`; crates use **edition 2024**. `rustfmt.toml` keeps defaults plus `hard_tabs = false`; clippy runs with default lints (no suppressions).

## Risks / Trade-offs

- [Rust toolchain absent; Tauri's Fedora system deps are numerous] → tasks include explicit environment-setup steps (rustup, one `dnf install` command) verified before the first build; README records them for future machines.
- [GUI launch cannot be asserted in headless automation] → acceptance for launch scenarios is a manual/dev-run check; automated coverage in this change is `cargo test` (fmm-core), `cargo clippy`, and the frontend build, with the window check performed interactively.
- [`ui/` without a framework may feel sparse once the mirror-list UI lands] → accepted; the Tauri command seam is framework-independent, so adopting a framework later touches only `ui/`.
- [Tauri identifier `org.fmm.desktop` may change at packaging time] → single constant in `tauri.conf.json`; no code depends on it yet.

## Migration Plan

Not applicable (greenfield). Rollback is `git` — the scaffold lands as an initial commit; later changes build on top.

## Open Questions

None blocking. Frontend framework choice, packaging target (RPM vs Flatpak), and the exact polkit mechanism are deliberately deferred to their own changes.
