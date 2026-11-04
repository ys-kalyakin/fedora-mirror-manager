# Spec Delta

## Purpose

Lets the user pick the app's color theme — light or dark — with one click from the app header, instead of the theme being dictated solely by the GNOME system color scheme.

## ADDED Requirements

### Requirement: Header theme toggle
The app header SHALL contain a theme toggle button that switches the entire UI between the light and dark themes. Activating the button SHALL apply the other theme immediately, without restarting the app or reloading the page.

#### Scenario: Toggle from light to dark
- **WHEN** the UI is showing the light theme and the user activates the theme toggle button
- **THEN** the entire UI is immediately rendered in the dark theme

#### Scenario: Toggle back to light
- **WHEN** the UI is showing the dark theme and the user activates the theme toggle button
- **THEN** the entire UI is immediately rendered in the light theme

### Requirement: Default theme follows the system color scheme
While the user has made no explicit theme choice, the app SHALL follow the system color scheme: dark when the system prefers dark, light otherwise. While no explicit choice is stored, the app SHALL re-theme live when the system color scheme changes, without a restart.

#### Scenario: First launch follows system
- **GIVEN** no theme choice has been stored yet
- **WHEN** the app launches
- **THEN** the UI is rendered in the theme matching the system color scheme

#### Scenario: System scheme change re-themes live
- **GIVEN** no theme choice has been stored yet
- **WHEN** the system color scheme changes (for example, the GNOME style preference is flipped)
- **THEN** the running app re-themes to match without a restart

### Requirement: Explicit theme choice persists and overrides the system
When the user activates the theme toggle, the chosen theme SHALL be stored locally in the app profile and applied immediately. On every subsequent launch, the stored choice SHALL be applied instead of the system scheme, and while a stored choice exists, system color scheme changes SHALL NOT re-theme the app.

#### Scenario: Choice survives restart
- **WHEN** the user switches to a theme, closes the app, and launches it again
- **THEN** the app opens in the theme that was chosen before closing

#### Scenario: Stored choice overrides system change
- **GIVEN** the user has explicitly chosen a theme
- **WHEN** the system color scheme changes
- **THEN** the app keeps the stored theme and does not re-theme

### Requirement: Both themes cover the full UI
The light and dark themes SHALL each style every UI surface: the header, panels and their headers, the repository table and status dots, source/origin badges, warnings and errors, dialogs, the helper banner, and the footer status bar. Activating either theme SHALL NOT leave any surface rendered with colors belonging to the other theme.

#### Scenario: Full dark theme rendering
- **WHEN** the dark theme is active and the repositories panel, a snapshot dialog, and an error message are visible
- **THEN** each of them is rendered with dark-theme colors — dark backgrounds with light text — and no surface keeps light-theme colors

#### Scenario: Full light theme rendering
- **WHEN** the light theme is active and the repositories panel, a snapshot dialog, and an error message are visible
- **THEN** each of them is rendered with light-theme colors — light backgrounds with dark text — and no surface keeps dark-theme colors
