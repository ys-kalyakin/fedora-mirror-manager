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

### Requirement: Setup stages sources the privileged process can read
Before its single polkit invocation, the setup SHALL stage the resolved helper binary into a fresh temporary location readable by the privileged process, and SHALL build the privileged install script from the staged copies (and the already-staged policy) instead of the application bundle location. Staging SHALL require no additional authorization, so the whole setup remains exactly one polkit authorization.

#### Scenario: Install from an AppImage mount that root cannot read
- **GIVEN** the application runs from an AppImage whose FUSE mount is not readable by the privileged process (the default: the mount is owner-only)
- **WHEN** the user starts the setup and approves the single authorization prompt
- **THEN** the helper and the polkit policy are installed to their system paths, and the privileged step did not read any file from the mount

#### Scenario: Staging needs no extra authorization
- **WHEN** the user runs the setup from any application layout (AppImage, RPM install, development build)
- **THEN** exactly one password prompt appears and, after it, both files are installed

#### Scenario: Already-installed helper is not staged
- **GIVEN** the resolved helper source is already the installed path
- **WHEN** the user runs the setup to refresh the policy
- **THEN** only the policy is staged and reinstalled, as before

### Requirement: Setup cleans up its staging
The setup SHALL remove its staged copies when the setup run finishes, whether it succeeded or failed.

#### Scenario: Staged copies are cleaned up
- **WHEN** the setup run finishes, successfully or with an error
- **THEN** no staged copy of the helper or the policy remains in the temporary location

### Requirement: Setup reports the real failure when the user authorized
The setup SHALL distinguish an authorization that was not obtained from a privileged step that ran and failed. When the authorization is not obtained, the setup SHALL fail with the `denied` code as before. When the user authorized and the privileged step failed, the setup SHALL fail with an `install_failed` code whose message carries the underlying error text. The setup UI SHALL show that underlying message for such failures instead of the generic "not authorized" text.

#### Scenario: Privileged step fails after a correct password
- **GIVEN** the user approved the authorization prompt
- **WHEN** the privileged install step exits with an error
- **THEN** the setup fails with `install_failed` and the error dialog shows the underlying error text, not the "not authorized" text

#### Scenario: Dismissed prompt is still denied
- **WHEN** the user dismisses the authorization prompt
- **THEN** the setup fails with the `denied` error and the UI reports that nothing was changed

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
