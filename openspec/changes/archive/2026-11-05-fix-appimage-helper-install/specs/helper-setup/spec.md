# Spec Delta

## ADDED Requirements

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
