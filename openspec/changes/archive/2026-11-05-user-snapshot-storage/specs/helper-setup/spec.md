# Spec Delta

## ADDED Requirements

### Requirement: Update offer when the installed helper is outdated

The app SHALL offer the one-click setup as an update whenever the installed helper is outdated: it reports an older plan-protocol version than the application, or reports none at all. The update banner SHALL offer one click into the same setup flow and SHALL describe a single authorization, like the missing-helper banner. A current helper SHALL show no setup banner.

#### Scenario: Outdated helper shows the update banner
- **GIVEN** the installed helper reports an older protocol version than the application
- **WHEN** the repositories or snapshots panel is shown
- **THEN** a banner offers to update the helper with one click and its hint describes a single authorization

#### Scenario: Helper too old to report a version counts as outdated
- **GIVEN** the installed helper does not report a protocol version
- **WHEN** the panels are shown
- **THEN** the update banner is offered

#### Scenario: Current helper shows no banner
- **GIVEN** the installed helper reports the same protocol version as the application
- **WHEN** the panels are shown
- **THEN** no setup banner is shown

#### Scenario: Version probe grants no privileges
- **WHEN** the application detects the installed helper's version
- **THEN** it runs the helper directly without any polkit authorization and nothing is modified
