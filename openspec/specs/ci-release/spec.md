# ci-release Specification

## Purpose

Defines the automated verification and distribution contract: every push and
pull request is checked by CI, and pushing a version tag publishes a GitHub
Release with ready-to-run Linux x86_64 artifacts (AppImage with the bundled
privileged helper, RPM, and checksums) that a Fedora user can download and
use without build tools.

## Requirements

### Requirement: CI verifies every push and pull request
Every push to the main branch and every pull request SHALL trigger a CI run that executes the project's quality gates from the README: workspace Rust tests, workspace clippy with warnings denied, and the UI production build (`tsc && vite build`). The run SHALL be green only when all gates pass, and the run that corresponds to the head commit of a pull request SHALL report the result on that pull request.

#### Scenario: Green run on a healthy commit
- **WHEN** a commit on which tests, clippy, and the UI build all pass is pushed or opened as a pull request
- **THEN** the CI run for that commit finishes green with all gates executed

#### Scenario: A failing gate turns the run red
- **WHEN** any single gate fails in a push or pull request run (a Rust test fails, or clippy emits a warning, or the UI build reports a type or bundling error)
- **THEN** the CI run for that commit finishes red and names the failed gate

### Requirement: A version tag publishes a runnable GitHub Release
Pushing a tag named `v<version>` (for example `v0.1.0`) SHALL trigger a release run that builds the app in release mode for Linux x86_64 and publishes a GitHub Release for that tag containing: the AppImage (primary ready-to-run artifact), the RPM package, and a `SHA256SUMS` file covering both. Release notes SHALL be generated automatically from the commits since the previous release. Tags that do not start with `v` SHALL NOT trigger a release run.

#### Scenario: Tag push produces a full release
- **WHEN** the maintainer pushes tag `v0.1.0` to the GitHub repository
- **THEN** a release run builds, and the release page for `v0.1.0` offers an `.AppImage`, an `.rpm`, and `SHA256SUMS` whose lines match the uploaded assets, with auto-generated notes

#### Scenario: Checksums match the assets
- **WHEN** a user verifies the downloaded AppImage and RPM against the release's `SHA256SUMS`
- **THEN** every checksum matches its asset

#### Scenario: Non-version tag does not release
- **WHEN** a tag that does not start with `v` (for example `test-build`) is pushed
- **THEN** no release run starts and no GitHub Release is created

### Requirement: The released AppImage runs out of the box
The released AppImage SHALL launch the application on a supported Linux x86_64 system (Fedora with WebKitGTK installed) without any local build tools, and its one-click helper setup SHALL work from the bundled sources: the helper binary and the polkit policy needed by the existing install flow SHALL be available inside the released image so that authorizing the setup installs `/usr/libexec/fmm-apply` and the policy exactly as the helper-setup capability specifies.

#### Scenario: Downloaded AppImage launches standalone
- **WHEN** a user downloads the AppImage from a release, makes it executable, and runs it on Fedora x86_64 without a Rust or Node toolchain installed
- **THEN** the application window opens showing the repositories view

#### Scenario: One-click helper install works from the released AppImage
- **WHEN** the user clicks **Install helper** in the released AppImage and approves the single authorization prompt
- **THEN** the helper and the polkit policy are installed to their system paths and the setup reports success, as in the helper-setup capability

### Requirement: The released RPM installs on Fedora
The released RPM SHALL be installable on a supported Fedora x86_64 system with the package manager, SHALL put the application on the system menu/PATH, and SHALL launch the same application; privileged setup from the installed app SHALL follow the same helper-setup flow.

#### Scenario: RPM installs and launches
- **WHEN** a user on Fedora x86_64 installs the release's RPM with `dnf install ./<file>.rpm` and starts Fedora Mirror Manager from the menu or terminal
- **THEN** the application launches with the repositories view

### Requirement: Pipelines reproduce from a clean checkout
Every CI and release run SHALL succeed on a fresh GitHub-hosted runner from a clean checkout using only committed inputs: the committed lockfiles (Cargo.lock, ui/package-lock.json), the committed Rust toolchain pin, and declared system dependencies — with no reliance on state from previous runs. Caching MAY speed runs up but SHALL NOT be required for correctness.

#### Scenario: Cold-cache run succeeds
- **WHEN** a CI run and a release run execute with empty caches on a clean checkout
- **THEN** both complete successfully using only committed inputs and declared system packages
