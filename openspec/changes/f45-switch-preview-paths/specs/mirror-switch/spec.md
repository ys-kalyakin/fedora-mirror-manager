# Spec Delta

## MODIFIED Requirements

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
