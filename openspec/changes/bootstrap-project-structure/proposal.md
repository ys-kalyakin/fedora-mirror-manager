# Proposal

## Why

The repository currently contains only the OpenSpec planning home — no code, no build system. Before any mirror-management feature can be delivered, the project needs a foundation: a Rust workspace with a Tauri 2 desktop shell and a module layout that future changes (mirror discovery, mirror switching, privileged repo editing) can build on without rework.

## What Changes

- Create a **Cargo workspace** with two members:
  - `crates/fmm-core` — domain library (repository model, config parsing, future mirror logic); scaffolded with a minimal public API placeholder.
  - `src-tauri` — Tauri 2 application crate (window, commands, app state); named `fmm-app`.
- Scaffold a **frontend** in `ui/` (Vite + TypeScript, no heavy framework yet) wired to the Tauri shell.
- Deliver an **app shell**: the application builds and launches, showing a main window with a placeholder layout (title bar content, mirror-list placeholder area, status placeholder).
- Add **project hygiene**: `rust-toolchain.toml` (stable), `.gitignore`, `.editorconfig`, `README.md`, rustfmt/clippy configuration, and git repository initialization.
- No application features beyond the shell: mirror discovery, switching, and polkit integration are explicitly out of scope and reserved for later changes.

## Capabilities

### New Capabilities

- `app-shell`: The application launches as a Tauri 2 desktop window presenting the basic UI shell (application title, placeholder mirror-list area, placeholder status area) and exits cleanly when the window is closed. Covers startup, window presentation, and the structural seams (commands layer in `fmm-app`, domain layer in `fmm-core`, UI layer in `ui/`) that all later capabilities extend.

### Modified Capabilities

- (none — greenfield project)

## Impact

- **New code**: workspace root manifest, `crates/fmm-core`, `src-tauri` (`fmm-app`), `ui/` frontend, project hygiene files.
- **Environment prerequisites** (to be set up during apply): Rust stable via rustup (not currently installed), Node.js ≥ 20 + npm (present), Tauri 2 system dependencies on Fedora (e.g. `webkit2gtk4.1-devel`, `gtk3-devel` and related packages via `dnf`).
- **No existing code, APIs, or specs affected** — the project is empty apart from `openspec/` and `.zcode/`.
- Assumption: artifacts are written in English; the UI language and any localization are deferred to later changes.
