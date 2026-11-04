# mirror-switch Specification

## Purpose

Lets the user switch individual dnf repositories between the fixed set of Russian mirrors (Yandex, HyperDedic) and manage Cisco OpenH264 repositories, replacing `metalink`/`mirrorlist` sources with literal mirror `baseurl`s. Changes are previewed before anything is written, applied through a polkit-authorized privileged helper, and optionally followed by a package-cache refresh.

## Requirements

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

The system SHALL show the planned changes for the selected repository (the target file, the new `baseurl` and removed lines) before any privileged operation is requested, and SHALL NOT write anything until the user confirms the preview. On the F45+ layout the preview SHALL show the file in `/etc/yum.repos.d` that will be created or rewritten, not the packaged file, and SHALL additionally name the packaged source file under `/usr/share/dnf5/repos.d` from which the new content is derived. The preview SHALL distinguish a target file that will be created from one that will be rewritten in place. On the legacy layout the preview SHALL show the file to be rewritten without the created/rewritten distinction and without a packaged source.

#### Scenario: Preview shown before apply
- **WHEN** the user selects a mirror for a repository and opens the confirmation
- **THEN** the planned new `baseurl` (and any `enabled=` change) is displayed before authorization is requested

#### Scenario: Preview names the effective target on F45+
- **GIVEN** the F45+ layout and a repository with no overriding file in `/etc/yum.repos.d`
- **WHEN** the user opens the confirmation for a switch
- **THEN** the preview shows the overriding file path in `/etc/yum.repos.d` that the switch will create, explicitly marked as a file that will be created

#### Scenario: Preview names the packaged source on F45+
- **GIVEN** the F45+ layout and a repository whose section is defined by a packaged file under `/usr/share/dnf5/repos.d`
- **WHEN** the user opens the confirmation for a switch
- **THEN** the preview names that packaged file as the source the new content is derived from, alongside the target path in `/etc/yum.repos.d`

#### Scenario: Preview marks a rewritten overriding copy
- **GIVEN** the F45+ layout and an overriding file already present in `/etc/yum.repos.d` for the target repository
- **WHEN** the user opens the confirmation for a switch
- **THEN** the preview shows the target path in `/etc/yum.repos.d` marked as a file that will be rewritten in place

#### Scenario: Legacy preview keeps the single-path form
- **GIVEN** the legacy layout
- **WHEN** the user opens the confirmation for a switch
- **THEN** the preview shows the path of the file to be rewritten with no created/rewritten marking and no packaged source

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

### Requirement: Switch never writes the packaged repository location

On the F45+ layout the system SHALL NOT modify anything under `/usr/share/dnf5/repos.d`. A switch SHALL write its result as a complete copy of the current packaged `.repo` file — with only the target section rewritten — into `/etc/yum.repos.d` under the same file name, where it overrides the packaged file entirely.

#### Scenario: First switch on F45+ creates an overriding copy
- **GIVEN** the F45+ layout, no file with the packaged file's name under `/etc/yum.repos.d`, and a packaged file with the target section
- **WHEN** the user switches the target repository to a mirror and authorizes
- **THEN** a new file with the same name is created in `/etc/yum.repos.d` whose target section carries the mirror `baseurl` and no `metalink`/`mirrorlist` lines
- **AND** the packaged file under `/usr/share/dnf5/repos.d` is unchanged

#### Scenario: Overriding copy overrides the packaged file entirely
- **GIVEN** the F45+ layout and a completed switch that created an overriding file in `/etc/yum.repos.d`
- **WHEN** dnf resolves the repository configuration
- **THEN** the repository section as defined by the overriding file is in effect (including the absence of the packaged `metalink` line)

### Requirement: Creating an overriding copy is guarded against races

When a switch needs to create an overriding file that does not yet exist, the plan SHALL require the file to be absent at apply time; the privileged write SHALL be rejected as stale if the file appeared between preview and apply.

#### Scenario: File appeared between preview and apply
- **GIVEN** a plan that creates an overriding file and a file of the same name that appeared after the preview
- **WHEN** the plan is applied
- **THEN** the helper reports a stale plan and no file is written

### Requirement: Existing overriding copy is rebuilt from the packaged file

On the F45+ layout, when an overriding file already exists in `/etc/yum.repos.d` for the target repository, a switch SHALL rewrite that file in place, rebuilding its content from the current packaged file with only the target section changed, so that unrelated package-update changes are not lost.

#### Scenario: Existing overriding copy is rewritten in place
- **GIVEN** the F45+ layout and an overriding file already present in `/etc/yum.repos.d` for the target repository
- **WHEN** the user switches the target repository to a different mirror
- **THEN** the overriding file is rewritten in place, based on the current content of the packaged file with only the target section changed

### Requirement: Bulk mirror actions in the repositories panel header

The system SHALL offer, in the repositories panel header alongside the refresh action, one bulk action per mirror (`Yandex`, `HyperDedic`) that plans in a single operation the switch of every repository eligible for that mirror. Each bulk action SHALL cover only that mirror's repositories; the per-repository switch controls in the table SHALL remain available and unchanged.

#### Scenario: One button per mirror in the panel header
- **WHEN** the repositories table is displayed
- **THEN** a bulk action for Yandex and a bulk action for HyperDedic are shown in the panel header next to the refresh action

#### Scenario: Bulk action plans without privileges
- **WHEN** the user triggers a bulk action for a mirror
- **THEN** the batch is planned without any authorization request and a combined preview of the planned changes is shown before anything is written

### Requirement: Bulk switch covers every repository the mirror can serve

A bulk action for a mirror SHALL include exactly the repositories that are switchable, whose family the mirror carries (a mirror `baseurl` exists for it), and whose current source is not already that mirror. Every switchable but excluded repository SHALL be reported with a reason: already on the mirror, or not carried by the mirror. A bulk switch SHALL rewrite only URL lines: the `enabled` flag of every included repository SHALL remain unchanged.

#### Scenario: Metalink repositories are switched
- **GIVEN** switchable repositories whose current source is `metalink`
- **WHEN** the user runs the Yandex bulk action
- **THEN** those repositories are included in the batch and their sections receive the Yandex `baseurl` with no `metalink`/`mirrorlist` lines left

#### Scenario: HyperDedic batch skips RPM Fusion with a reason
- **GIVEN** RPM Fusion repositories and the HyperDedic bulk action
- **WHEN** the preview is shown
- **THEN** the RPM Fusion repositories are reported as skipped because HyperDedic does not mirror them, and no plan entry references them

#### Scenario: Repositories already on the mirror are skipped without writes
- **GIVEN** switchable repositories whose current source already is the chosen mirror
- **WHEN** the user runs that mirror's bulk action
- **THEN** those repositories are reported as skipped (already on the mirror) and no file is written for them

#### Scenario: Not-switchable and Cisco repositories are untouched
- **GIVEN** third-party repositories and Cisco OpenH264 repositories
- **WHEN** the user runs any bulk action
- **THEN** no plan entry references them

#### Scenario: Disabled repository keeps its enabled state
- **GIVEN** a disabled repository classified switchable and carried by the chosen mirror
- **WHEN** the bulk action for that mirror is applied
- **THEN** the repository's URL lines point at the mirror and its `enabled` value is unchanged

### Requirement: Repositories sharing a file are merged into one rewrite

When several included repositories are defined by the same `.repo` file, the plan SHALL contain one rewrite of that file covering all included sections and SHALL NOT contain overlapping rewrites of the same path. On the F45+ layout, several included sections of one packaged file SHALL produce a single overriding copy in `/etc/yum.repos.d` built from the current packaged file. If any target file changed between preview and apply, the whole batch SHALL be rejected as stale and no file written.

#### Scenario: Two sections of one file become one plan entry
- **GIVEN** a `.repo` file defining two switchable sections, both included in the bulk action
- **WHEN** the plan is built
- **THEN** the plan contains exactly one rewrite of that file whose content carries both sections' new `baseurl`s and keeps every other section byte-identical

#### Scenario: One overriding copy per packaged file
- **GIVEN** the F45+ layout and a packaged file defining two switchable sections both included in the bulk action, with no overriding copy yet
- **WHEN** the plan is built
- **THEN** the plan creates exactly one file in `/etc/yum.repos.d` under the packaged file's name, derived from the current packaged file with both sections rewritten, and the packaged file is unchanged

#### Scenario: A file changed since the preview aborts the whole batch
- **GIVEN** a confirmed bulk plan and one of its target files modified outside the application after the preview
- **WHEN** the plan is applied
- **THEN** the helper reports a stale plan and no file of the batch is written

### Requirement: Combined preview before authorization

The system SHALL show the planned bulk changes in a single confirmation dialog before any privileged operation: every affected file with its target path marked created or rewritten and its packaged source (on the F45+ layout), each rewritten section with its new `baseurl`, and the skipped repositories with reasons. The dialog SHALL offer the package-cache refresh option, SHALL NOT write anything unless the user confirms, and cancel SHALL leave the system untouched.

#### Scenario: Preview lists every affected file and section
- **WHEN** the user opens the confirmation for a bulk action
- **THEN** each file the batch will write is listed with its created/rewritten marking, and each rewritten section is listed with its new `baseurl`

#### Scenario: Skipped repositories are visible before applying
- **GIVEN** a bulk action where some repositories are skipped (already on the mirror, not carried)
- **WHEN** the confirmation is shown
- **THEN** the skipped repositories are listed with their reasons alongside the planned changes

#### Scenario: Cancel in the bulk preview performs nothing
- **WHEN** the user cancels the bulk confirmation
- **THEN** no privileged action is requested and no file is modified

### Requirement: One authorization and one snapshot per bulk apply

Applying a confirmed bulk plan SHALL perform exactly one polkit authorization, create exactly one pre-write snapshot tagged for the bulk switch, and then write all files of the batch. On denial or failure before the first write, the files on disk SHALL remain unchanged and the UI SHALL show an error. The resulting report SHALL show the snapshot and the files changed by the batch, and a failed cache refresh SHALL be reported as a warning, not a failure.

#### Scenario: One password prompt covers the whole batch
- **WHEN** the user confirms a bulk plan that touches several files and grants the authorization
- **THEN** a single polkit prompt appears and, after it, all files of the batch are written and one snapshot is saved

#### Scenario: Authorization denied leaves everything unchanged
- **WHEN** the user dismisses the polkit prompt for a bulk apply
- **THEN** no file is modified and an explicit "not authorized" indication is shown

#### Scenario: Makecache failure after a bulk apply is a warning
- **GIVEN** the refresh option is enabled
- **WHEN** `dnf makecache` fails after the batch was written
- **THEN** the batch is reported as applied with a warning, not as failed

### Requirement: Bulk action with nothing to switch

When no repository is eligible for a bulk action (every switchable repository is already on that mirror or not carried by it), the system SHALL report that there is nothing to switch, without requesting authorization and without writing anything.

#### Scenario: Everything already on the mirror
- **GIVEN** every switchable repository the chosen mirror carries already uses it
- **WHEN** the user runs that mirror's bulk action
- **THEN** an explicit "nothing to switch" indication is shown and no authorization is requested
