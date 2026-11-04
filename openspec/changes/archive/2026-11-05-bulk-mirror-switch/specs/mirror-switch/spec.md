# Spec Delta

## Purpose

Extends mirror switching with per-mirror bulk actions: one click above the repository list plans and applies a switch to a chosen mirror for every repository that can be switched to it, under one combined preview, one authorization, and one snapshot.

## ADDED Requirements

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
