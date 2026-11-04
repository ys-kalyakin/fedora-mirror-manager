# Spec Delta: mirror-switch

## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Changes are previewed before authorization

The system SHALL show the planned changes for the selected repository (the target file, the new `baseurl` and removed lines) before any privileged operation is requested, and SHALL NOT write anything until the user confirms the preview. On the F45+ layout the preview SHALL show the file in `/etc/yum.repos.d` that will be created or rewritten, not the packaged file.

#### Scenario: Preview shown before apply
- **WHEN** the user selects a mirror for a repository and opens the confirmation
- **THEN** the planned new `baseurl` (and any `enabled=` change) is displayed before authorization is requested

#### Scenario: Preview names the effective target on F45+
- **GIVEN** the F45+ layout and a repository with no overriding file in `/etc/yum.repos.d`
- **WHEN** the user opens the confirmation for a switch
- **THEN** the preview shows the overriding file path in `/etc/yum.repos.d` that the switch will create

#### Scenario: Cancel in preview performs nothing
- **WHEN** the user cancels the confirmation
- **THEN** no privileged action is requested and no file is modified
