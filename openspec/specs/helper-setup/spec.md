# helper-setup Specification

## Purpose

Lets the user install the privileged helper and its polkit policy from the app in one click, with a single password request, so switching mirrors and restoring snapshots work without manual terminal steps.

## Requirements

### Requirement: Setup asks for the password once
The helper setup flow SHALL perform all its privileged file operations under exactly one polkit authorization per setup run. The flow SHALL NOT require the user to enter the admin password more than once, regardless of how many files it installs.

#### Scenario: Fresh install with one prompt
- **WHEN** the user starts setup while neither the helper binary nor the policy is installed and approves the authorization prompt
- **THEN** exactly one password prompt appears and, after it, both the helper binary and the policy are installed

#### Scenario: Prompt dismissed
- **WHEN** the user dismisses the authorization prompt during setup
- **THEN** the setup fails with the `denied` error and reports no destinations as installed

### Requirement: Setup installs the helper and the policy
The setup SHALL install the resolved helper binary to `/usr/libexec/fmm-apply` (mode 0755, owner root:root) and the embedded polkit policy to `/usr/share/polkit-1/actions/org.fmm.desktop.apply.policy` (mode 0644, owner root:root). On success it SHALL report the list of installed destination paths. If the resolved helper source is already the installed path, the setup SHALL still install (refresh) the policy.

#### Scenario: Successful setup report
- **WHEN** the user approves the prompt during a fresh setup
- **THEN** the completion dialog lists `/usr/libexec/fmm-apply` and `/usr/share/polkit-1/actions/org.fmm.desktop.apply.policy` as installed

#### Scenario: Setup is repeatable
- **WHEN** the user runs setup again after it already succeeded (for example after rebuilding the helper)
- **THEN** the setup succeeds again with a single authorization prompt and reports the installed destinations

### Requirement: Setup resolves the helper source or fails clearly
The setup SHALL resolve the helper binary source from the app-provided override, then the app executable's directory, then the installed path. When no helper binary can be resolved, the setup SHALL fail with the `helper_missing` error and a message naming the build step, without asking for authorization.

#### Scenario: Helper binary not built
- **WHEN** the user starts setup and no helper binary exists in any of the resolved locations
- **THEN** the setup fails with `helper_missing` and shows the build instruction, with no password prompt

### Requirement: Setup tolerates special characters in paths
The setup SHALL correctly install when the helper source path or its parent directories contain spaces, quotes, or other shell-special characters; such paths SHALL NOT break the privileged invocation or allow command injection.

#### Scenario: Helper source path contains spaces
- **WHEN** the helper binary to install lives under a directory whose name contains a space, and the user approves the prompt
- **THEN** the setup succeeds and the helper is installed to `/usr/libexec/fmm-apply`

### Requirement: Setup UI describes a single authorization
The setup UI (banner, error-dialog entry, and progress dialog) SHALL describe the authorization as a single password request and SHALL NOT state that multiple prompts may appear.

#### Scenario: Banner hint wording
- **WHEN** the helper is missing and the setup banner is shown
- **THEN** its hint text says installing asks for authorization once and does not mention two prompts

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
