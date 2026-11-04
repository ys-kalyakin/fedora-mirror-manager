# Spec Delta

## Purpose

Gives the user a read-only, always-available inventory of the dnf repositories configured on the system: what is in `/etc/yum.repos.d/*.repo`, whether each repository is enabled, and which source it currently points to. This view needs no privileges and is the foundation on which switching actions operate.

## ADDED Requirements

### Requirement: Repository inventory is loaded from the repo directory

The system SHALL read every `*.repo` file in `/etc/yum.repos.d` and present one entry per repository section with: repository id (section header), display name, enabled flag, current source classification, current URL (if any), and the file the section comes from.

#### Scenario: Inventory displayed on first load
- **GIVEN** `/etc/yum.repos.d` contains at least one `*.repo` file
- **WHEN** the repository list view loads
- **THEN** every section of every `*.repo` file is listed with its id, name, enabled flag, source, and URL

#### Scenario: Empty repo directory
- **GIVEN** `/etc/yum.repos.d` exists but contains no `*.repo` files
- **WHEN** the repository list view loads
- **THEN** an explicit "no repositories found" state is shown instead of an empty table without explanation

#### Scenario: Repo directory missing
- **GIVEN** `/etc/yum.repos.d` does not exist
- **WHEN** the repository list view loads
- **THEN** an explicit error state explains that the repository directory was not found

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

The system SHALL provide an explicit refresh action that re-reads `/etc/yum.repos.d` from disk and updates the view, and SHALL reflect the on-disk state after the refresh completes.

#### Scenario: Refresh picks up external changes
- **GIVEN** a repository file was modified outside the application
- **WHEN** the user triggers refresh
- **THEN** the view shows the new on-disk state

### Requirement: Parse errors are surfaced

The system SHALL surface read failures (unreadable file, permission error) as an explicit error indication that names the affected file, and SHALL still show the sections that were read successfully.

#### Scenario: Unreadable file among readable ones
- **GIVEN** one `*.repo` file exists but cannot be read
- **WHEN** the inventory is loaded
- **THEN** an error naming that file is shown alongside the entries parsed from the readable files
