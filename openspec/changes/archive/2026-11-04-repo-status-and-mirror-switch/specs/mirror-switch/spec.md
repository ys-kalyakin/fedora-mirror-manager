# Spec Delta

## Purpose

Lets the user switch individual dnf repositories between the fixed set of Russian mirrors (Yandex, HyperDedic) and manage Cisco OpenH264 repositories, replacing `metalink`/`mirrorlist` sources with literal mirror `baseurl`s. Changes are previewed before anything is written, applied through a polkit-authorized privileged helper, and optionally followed by a package-cache refresh.

## ADDED Requirements

### Requirement: Per-repository mirror choice

The system SHALL offer each switchable repository a choice between the mirrors `mirror.yandex.ru` and `mirror.hyperdedic.ru`. The resulting `baseurl` SHALL use literal `$releasever` and `$basearch` variables and follow the URL mapping validated by the reference implementation for the repository's family (release, updates, updates-testing, modular variants, RPM Fusion free/nonfree, EPEL).

#### Scenario: Switch updates to Yandex
- **WHEN** the user applies the Yandex mirror to the `updates` repository
- **THEN** the planned `baseurl` is `https://mirror.yandex.ru/fedora/linux/updates/$releasever/Everything/$basearch/`

#### Scenario: Mirror availability differs per repository family
- **WHEN** the user views switching options for an RPM Fusion repository while HyperDedic is offered
- **THEN** HyperDedic is presented as unavailable for that repository with an explanation (it does not mirror RPM Fusion)

#### Scenario: Choosing the mirror the repository already uses
- **WHEN** the user picks a mirror whose `baseurl` the repository section already has
- **THEN** the application reports that no change is needed and performs no write

### Requirement: Switching rewrites only the target section

Applying a mirror switch SHALL replace the section's source with the mirror `baseurl` placed directly under the section header and SHALL remove existing `baseurl`, `metalink`, and `mirrorlist` lines (including commented-out ones) from that section only. Other sections in the same file SHALL remain untouched.

#### Scenario: Sibling sections are untouched
- **GIVEN** a `.repo` file contains the sections `fedora` and `updates`
- **WHEN** the user switches `fedora` to Yandex
- **THEN** only the `fedora` section's URL lines change and the `updates` section is byte-identical

#### Scenario: Metalink lines are removed on switch
- **GIVEN** a section with `metalink=` and commented `# baseurl=` lines
- **WHEN** the user switches it to a mirror
- **THEN** the section contains exactly one new `baseurl=` line under the header and no `metalink=` or commented `baseurl=` lines

### Requirement: Signing configuration is never modified

The system SHALL NOT modify `gpgcheck`, `gpgkey`, `enabled` (except the Cisco disable mode), or any other unrelated line of a switched section.

#### Scenario: gpgcheck survives a switch
- **GIVEN** a section with `gpgcheck=1` and a `gpgkey=` line
- **WHEN** the user switches it to a mirror
- **THEN** `gpgcheck` and `gpgkey` lines are unchanged in the written file

### Requirement: Cisco OpenH264 modes

For Cisco OpenH264 repositories the system SHALL offer exactly three modes: `disable` (writes `enabled=0`), `codecs` (writes `baseurl=https://codecs.fedoraproject.org/openh264/$releasever/$basearch/` and `enabled=1`), and `keep` (no change).

#### Scenario: Cisco repository disabled
- **WHEN** the user applies mode `disable` to `fedora-cisco-openh264`
- **THEN** the section contains `enabled=0` and its URL lines are otherwise unchanged

#### Scenario: Cisco repository moved to codecs mirror
- **WHEN** the user applies mode `codecs` to a Cisco OpenH264 repository
- **THEN** the section points at `https://codecs.fedoraproject.org/openh264/$releasever/$basearch/` and is enabled

### Requirement: Changes are previewed before authorization

The system SHALL show the planned changes for the selected repository (new `baseurl` and removed lines) before any privileged operation is requested, and SHALL NOT write anything until the user confirms the preview.

#### Scenario: Preview shown before apply
- **WHEN** the user selects a mirror for a repository and opens the confirmation
- **THEN** the planned new `baseurl` (and any `enabled=` change) is displayed before authorization is requested

#### Scenario: Cancel in preview performs nothing
- **WHEN** the user cancels the confirmation
- **THEN** no privileged action is requested and no file is modified

### Requirement: Privileged application via polkit

The system SHALL perform confirmed writes as root through a helper authorized by polkit. On success the repository entry reflects the new source; on denial or failure the files on disk SHALL remain unchanged and the UI SHALL show an error.

#### Scenario: Successful authorized switch
- **WHEN** the user confirms the switch and grants authorization in the polkit dialog
- **THEN** the section's file on disk is updated and the repository list shows the new source after refresh

#### Scenario: Authorization denied
- **WHEN** the user cancels the polkit dialog
- **THEN** no file is modified and an explicit "not authorized" indication is shown

#### Scenario: Helper failure leaves files intact
- **WHEN** the privileged helper reports a failure during the write
- **THEN** the UI shows an error with the reported cause and the repository keeps its previous source

### Requirement: In-app privileged helper setup

The system SHALL offer an in-app setup action that installs the privileged helper binary and its polkit policy by requesting authorization through polkit, without any pre-installation step, and SHALL make switching and restore available once the setup succeeds. While the helper is missing, the UI SHALL show an explicit indication with this setup action available.

#### Scenario: Setup installs the helper from the app
- **WHEN** the user triggers the in-app setup and grants the authorization prompts
- **THEN** the helper binary is installed as root and the polkit policy is registered
- **AND** switching and restore become available without restarting the application

#### Scenario: Setup denied keeps the app read-only
- **WHEN** the user cancels the authorization during setup
- **THEN** nothing is installed and the app remains a read-only viewer with the explicit missing-helper indication

### Requirement: Optional package cache refresh

The system SHALL offer a "refresh package cache" option, enabled by default, that runs `dnf clean all` followed by `dnf makecache` after a successful privileged write. A `makecache` failure SHALL be reported as a warning and SHALL NOT mark the switch as failed.

#### Scenario: Cache refresh after switch
- **GIVEN** the refresh option is enabled
- **WHEN** a switch completes successfully
- **THEN** `dnf clean all` and `dnf makecache` are run as root and their progress/outcome is shown

#### Scenario: No cache refresh when disabled
- **GIVEN** the refresh option is disabled
- **WHEN** a switch completes successfully
- **THEN** no dnf command is run

#### Scenario: Makecache failure is a warning
- **GIVEN** the refresh option is enabled
- **WHEN** `dnf makecache` exits with an error after a successful file write
- **THEN** the switch is reported as applied with a warning, not as failed
