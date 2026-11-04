# Spec Delta

## Purpose

Establishes the desktop application foundation: a windowed shell that launches, presents a placeholder interface, and exposes the seam between the frontend and the Rust backend on which all mirror-management features build.

## ADDED Requirements

### Requirement: Application launches into a main window

The system SHALL build and launch as a desktop application that opens a single main window presenting the application shell.

#### Scenario: Successful launch in development mode
- **WHEN** the application is started from the development toolchain
- **THEN** a main window opens without errors
- **AND** the window shows the application title "Fedora Mirror Manager"

#### Scenario: Shell layout is present
- **WHEN** the main window is open
- **THEN** the interface shows a header area with the application name
- **AND** a main content area with a placeholder for the mirror list
- **AND** a status area placeholder

### Requirement: Clean shutdown

The application SHALL terminate its process when the user closes the main window.

#### Scenario: Window close ends the application
- **WHEN** the user closes the main window
- **THEN** the application process exits and no background process remains

### Requirement: Backend status handshake

The shell SHALL request application status from the Rust backend when the interface loads and display the returned status in the status area.

#### Scenario: Status retrieved on load
- **WHEN** the shell finishes loading
- **THEN** it requests status from the backend
- **AND** displays the backend-reported application version in the status area

#### Scenario: Backend status failure
- **WHEN** the status request to the backend fails
- **THEN** the status area shows an explicit error indication instead of a stale or empty status
