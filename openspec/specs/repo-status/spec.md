# repo-status Specification

## Purpose

Gives the user a read-only, always-available inventory of the dnf repositories configured on the system: what is in `/etc/yum.repos.d/*.repo`, whether each repository is enabled, and which source it currently points to. This view needs no privileges and is the foundation on which switching actions operate.

## Requirements

### Requirement: Repository inventory is loaded from the repo directory

The system SHALL read every `*.repo` file in the repository locations of the detected layout and present one entry per repository section with: repository id (section header), display name, enabled flag, current source classification, current URL (if any), and the file the section comes from. On the legacy layout the only location is `/etc/yum.repos.d`. On the F45+ layout the locations are `/usr/share/dnf5/repos.d` and `/etc/yum.repos.d`, merged: when a repository section id is defined in both locations, the definition from `/etc/yum.repos.d` is the effective one and the packaged definition SHALL NOT be listed separately.

#### Scenario: Inventory displayed on first load
- **GIVEN** the repository locations of the detected layout contain at least one `*.repo` file
- **WHEN** the repository list view loads
- **THEN** every section of every `*.repo` file is listed with its id, name, enabled flag, source, and URL

#### Scenario: Inventory merges both locations on F45+
- **GIVEN** the F45+ layout with `*.repo` files under both `/usr/share/dnf5/repos.d` and `/etc/yum.repos.d`
- **WHEN** the repository list view loads
- **THEN** sections from both locations are listed in one view

#### Scenario: Local definition takes precedence on F45+
- **GIVEN** the F45+ layout and a repository section id defined by a file under `/etc/yum.repos.d` and also by a packaged file under `/usr/share/dnf5/repos.d`
- **WHEN** the repository list view loads
- **THEN** the entry reflects the `/etc/yum.repos.d` definition and the packaged definition of that section id is not shown separately

#### Scenario: Empty repo directory
- **GIVEN** the repository locations of the detected layout exist but contain no `*.repo` files
- **WHEN** the repository list view loads
- **THEN** an explicit "no repositories found" state is shown instead of an empty table without explanation

#### Scenario: Repo directory missing
- **GIVEN** the repository locations required by the detected layout do not exist (on F45+ both directories are absent or neither contains `*.repo` files)
- **WHEN** the repository list view loads
- **THEN** an explicit error state explains that the repository directories were not found

### Requirement: Current source classification per repository

The system SHALL classify each repository's current source from its configured URL lines as one of: `yandex` (baseurl on `mirror.yandex.ru`), `hyperdedic` (baseurl on `mirror.hyperdedic.ru`), `metalink` (a `metalink` or `mirrorlist` line), `other` (a `baseurl` pointing elsewhere), or `none` (no URL line at all). The first URL line found SHALL be used when a section carries several.

#### Scenario: Yandex baseurl recognized
- **WHEN** a section's `baseurl` starts with `https://mirror.yandex.ru/`
- **THEN** the repository is shown with source `yandex`

#### Scenario: Metalink recognized
- **WHEN** a section defines a `metalink` or `mirrorlist` line and no preceding `baseurl`
- **THEN** the repository is shown with source `metalink`

#### Scenario: Foreign baseurl shown as other
- **WHEN** a section's `baseurl` points to a host other than the known mirrors
- **THEN** the repository is shown with source `other` and the URL is visible to the user

#### Scenario: No URL configured
- **WHEN** a section has neither `baseurl`, `metalink`, nor `mirrorlist`
- **THEN** the repository is shown with source `none`

### Requirement: Switchability classification per repository

The system SHALL mark each repository as: switchable (Fedora, updates, modular, updates-testing, RPM Fusion free/nonfree, and EPEL families, classified from the section id and file name), Cisco OpenH264 (special modes), or not switchable (source/debuginfo, playground, COPR, `redhat.repo` and other third-party repositories). The classification SHALL follow the rules validated by the reference implementation, and unknown repositories SHALL default to not switchable.

#### Scenario: Fedora repository is switchable
- **WHEN** the section id `fedora` or `updates` is classified
- **THEN** the repository is marked switchable

#### Scenario: Cisco OpenH264 repository flagged
- **WHEN** a section id or file name contains `openh264` or `cisco`
- **THEN** the repository is marked as a Cisco OpenH264 repository

#### Scenario: Third-party repository is not switchable
- **WHEN** a section belongs to a third-party repository (e.g. Google Chrome, VS Code, a COPR repository)
- **THEN** the repository is shown but marked not switchable

### Requirement: Manual refresh of the inventory

The system SHALL provide an explicit refresh action that re-reads the repository locations of the currently detected layout from disk and updates the view, and SHALL reflect the on-disk state after the refresh completes. Refresh SHALL re-run the layout detection, so that a layout change on disk (e.g. after an OS upgrade) is picked up.

#### Scenario: Refresh picks up external changes
- **GIVEN** a repository file was modified outside the application
- **WHEN** the user triggers refresh
- **THEN** the view shows the new on-disk state

#### Scenario: Refresh picks up a layout change
- **GIVEN** the inventory was loaded with the legacy layout and the repository files were subsequently relocated to the F45+ location
- **WHEN** the user triggers refresh
- **THEN** the view shows the F45+ layout and the relocated repositories

### Requirement: Parse errors are surfaced

The system SHALL surface read failures (unreadable file, permission error) as an explicit error indication that names the affected file, and SHALL still show the sections that were read successfully.

#### Scenario: Unreadable file among readable ones
- **GIVEN** one `*.repo` file exists but cannot be read
- **WHEN** the inventory is loaded
- **THEN** an error naming that file is shown alongside the entries parsed from the readable files

### Requirement: Repository layout is detected automatically

The system SHALL detect the repository layout of the host automatically before reading any repository state: the F45+ layout is active when the directory `/usr/share/dnf5/repos.d` contains at least one `*.repo` file; otherwise the legacy layout is active. Detection SHALL NOT rely on parsing the OS version. The detected layout SHALL be shown to the user in the UI.

#### Scenario: F45+ layout detected
- **GIVEN** `/usr/share/dnf5/repos.d` exists and contains at least one `*.repo` file
- **WHEN** the application starts or refreshes
- **THEN** the F45+ layout is used for all repository reads and writes and the UI indicates it

#### Scenario: Legacy layout detected
- **GIVEN** `/usr/share/dnf5/repos.d` does not exist or contains no `*.repo` files
- **WHEN** the application starts or refreshes
- **THEN** the legacy layout is used and the UI indicates it

### Requirement: Entry origin is shown on the F45+ layout

On the F45+ layout the system SHALL show for each inventory entry where it effectively comes from: `packaged` (defined by a file under `/usr/share/dnf5/repos.d` and not overridden) or `local` (defined by a file under `/etc/yum.repos.d`, including shadow copies written by the application). On the legacy layout all entries SHALL be shown as `local`. For every entry, regardless of layout, the full absolute path of the effective `.repo` file SHALL be visible to the user, not only the file's base name.

#### Scenario: Packaged repository marked
- **GIVEN** the F45+ layout and a repository section defined only under `/usr/share/dnf5/repos.d`
- **WHEN** the inventory is displayed
- **THEN** the entry is marked as coming from the packaged location

#### Scenario: Overriding file marked
- **GIVEN** the F45+ layout and a repository section defined by a file under `/etc/yum.repos.d` that also exists packaged
- **WHEN** the inventory is displayed
- **THEN** the entry is marked as coming from the local location

#### Scenario: Full effective path is visible
- **GIVEN** the F45+ layout and an entry marked as packaged
- **WHEN** the inventory is displayed
- **THEN** the full path of the packaged `.repo` file under `/usr/share/dnf5/repos.d` is available to the user for that entry
