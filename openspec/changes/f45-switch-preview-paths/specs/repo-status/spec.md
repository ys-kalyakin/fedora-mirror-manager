# Spec Delta

## MODIFIED Requirements

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
