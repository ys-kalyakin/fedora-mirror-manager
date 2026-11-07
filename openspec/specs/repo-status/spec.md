# repo-status Specification

## Purpose

Gives the user a read-only, always-available inventory of the dnf repositories configured on the system: what is in `/etc/yum.repos.d/*.repo`, whether each repository is enabled, and which source it currently points to. This view needs no privileges and is the foundation on which switching actions operate.

## Requirements

### Requirement: Repository inventory is loaded from the repo directory

The system SHALL read every `*.repo` file in the repository locations of the detected layout and present one entry per repository section with: repository id (section header), display name, enabled flag, current source classification, current URL (if any), and the file the section comes from. On the legacy layout the only location is `/etc/yum.repos.d`. On the F45+ layout the locations are `/usr/share/dnf5/repos.d` and `/etc/yum.repos.d`, merged: when a repository section id is defined in both locations, the definition from `/etc/yum.repos.d` is the effective one and the packaged definition SHALL NOT be listed separately.

#### Scenario: Inventory displayed on first load
- **GIVEN** the repository locations of the detected layout contain at least one `*.repo` file
- **WHEN** the repository list view loads
- **THEN** every section of every `*.repo` file is listed with its id, name, enabled flag, source, and URL

#### Scenario: Inventory merges both locations on F45+
- **GIVEN** the F45+ layout with `*.repo` files under both `/usr/share/dnf5/repos.d` and `/etc/yum.repos.d`
- **WHEN** the repository list view loads
- **THEN** sections from both locations are listed in one view

#### Scenario: Local definition takes precedence on F45+
- **GIVEN** the F45+ layout and a repository section id defined by a file under `/etc/yum.repos.d` and also by a packaged file under `/usr/share/dnf5/repos.d`
- **WHEN** the repository list view loads
- **THEN** the entry reflects the `/etc/yum.repos.d` definition and the packaged definition of that section id is not shown separately

#### Scenario: Empty repo directory
- **GIVEN** the repository locations of the detected layout exist but contain no `*.repo` files
- **WHEN** the repository list view loads
- **THEN** an explicit "no repositories found" state is shown instead of an empty table without explanation

#### Scenario: Repo directory missing
- **GIVEN** the repository locations required by the detected layout do not exist (on F45+ both directories are absent or neither contains `*.repo` files)
- **WHEN** the repository list view loads
- **THEN** an explicit error state explains that the repository directories were not found

### Requirement: Current source classification per repository

The system SHALL classify each repository's current source from its configured URL lines as one of: `yandex` (baseurl on `mirror.yandex.ru`), `hyperdedic` (baseurl on `mirror.hyperdedic.ru`), `metalink` (a `metalink` or `mirrorlist` line), `other` (a `baseurl` pointing elsewhere), or `none` (no URL line at all). The first URL line found SHALL be used when a section carries several.

#### Scenario: Yandex baseurl recognized
- **WHEN** a section's `baseurl` starts with `https://mirror.yandex.ru/`
- **THEN** the repository is shown with source `yandex`

#### Scenario: Metalink recognized
- **WHEN** a section defines a `metalink` or `mirrorlist` line and no preceding `baseurl`
- **THEN** the repository is shown with source `metalink`

#### Scenario: Foreign baseurl shown as other
- **WHEN** a section's `baseurl` points to a host other than the known mirrors
- **THEN** the repository is shown with source `other` and the URL is visible to the user

#### Scenario: No URL configured
- **WHEN** a section has neither `baseurl`, `metalink`, nor `mirrorlist`
- **THEN** the repository is shown with source `none`

### Requirement: Switchability classification per repository

The system SHALL mark each repository as: switchable (Fedora, updates, modular, updates-testing, RPM Fusion free/nonfree, and EPEL families, classified from the section id and file name), Cisco OpenH264 (special modes), or not switchable (source/debuginfo, playground, COPR, `redhat.repo` and other third-party repositories). The classification SHALL follow the rules validated by the reference implementation, and unknown repositories SHALL default to not switchable.

#### Scenario: Fedora repository is switchable
- **WHEN** the section id `fedora` or `updates` is classified
- **THEN** the repository is marked switchable

#### Scenario: Cisco OpenH264 repository flagged
- **WHEN** a section id or file name contains `openh264` or `cisco`
- **THEN** the repository is marked as a Cisco OpenH264 repository

#### Scenario: Third-party repository is not switchable
- **WHEN** a section belongs to a third-party repository (e.g. Google Chrome, VS Code, a COPR repository)
- **THEN** the repository is shown but marked not switchable

### Requirement: Manual refresh of the inventory

The system SHALL provide an explicit refresh action that re-reads the repository locations of the currently detected layout from disk and updates the view, and SHALL reflect the on-disk state after the refresh completes. Refresh SHALL re-run the layout detection, so that a layout change on disk (e.g. after an OS upgrade) is picked up.

#### Scenario: Refresh picks up external changes
- **GIVEN** a repository file was modified outside the application
- **WHEN** the user triggers refresh
- **THEN** the view shows the new on-disk state

#### Scenario: Refresh picks up a layout change
- **GIVEN** the inventory was loaded with the legacy layout and the repository files were subsequently relocated to the F45+ location
- **WHEN** the user triggers refresh
- **THEN** the view shows the F45+ layout and the relocated repositories

### Requirement: Parse errors are surfaced

The system SHALL surface read failures (unreadable file, permission error) as an explicit error indication that names the affected file, and SHALL still show the sections that were read successfully.

#### Scenario: Unreadable file among readable ones
- **GIVEN** one `*.repo` file exists but cannot be read
- **WHEN** the inventory is loaded
- **THEN** an error naming that file is shown alongside the entries parsed from the readable files

### Requirement: Repository layout is detected automatically

The system SHALL detect the repository layout of the host automatically before reading any repository state: the F45+ layout is active when the directory `/usr/share/dnf5/repos.d` contains at least one `*.repo` file; otherwise the legacy layout is active. Detection SHALL NOT rely on parsing the OS version. The detected layout SHALL be shown to the user in the UI.

#### Scenario: F45+ layout detected
- **GIVEN** `/usr/share/dnf5/repos.d` exists and contains at least one `*.repo` file
- **WHEN** the application starts or refreshes
- **THEN** the F45+ layout is used for all repository reads and writes and the UI indicates it

#### Scenario: Legacy layout detected
- **GIVEN** `/usr/share/dnf5/repos.d` does not exist or contains no `*.repo` files
- **WHEN** the application starts or refreshes
- **THEN** the legacy layout is used and the UI indicates it

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

### Requirement: Enabled state is shown as a color dot

In the repositories table the system SHALL show each entry's enabled state as a fixed-size schematic dot: green for enabled, red for disabled. The dot SHALL carry a text alternative naming the state (hover tooltip and accessible label), and the dot's rendered size and the width of its column SHALL NOT depend on which state is shown.

#### Scenario: Enabled repository shows a green dot
- **WHEN** the repositories table displays an entry whose enabled flag is true
- **THEN** the entry's enabled cell shows a green dot instead of the "enabled" text badge

#### Scenario: Disabled repository shows a red dot
- **WHEN** the repositories table displays an entry whose enabled flag is false
- **THEN** the entry's enabled cell shows a red dot instead of the "disabled" text badge

#### Scenario: Dot exposes its meaning as text
- **WHEN** the user hovers over (or an assistive technology focuses) a dot in the enabled column
- **THEN** the state is available as text ("enabled" or "disabled")

#### Scenario: Row layout does not depend on the state
- **WHEN** the table shows entries with mixed enabled and disabled states
- **THEN** all dots occupy the same cell space and no row's layout shifts compared to another

### Requirement: Badge labels stay on one line

In the repositories table, badge labels (source classification, entry origin) SHALL NOT break mid-word and SHALL NOT wrap onto a second line. The source column SHALL be wide enough to display its longest label (`HyperDedic`) without breaking.

#### Scenario: HyperDedic label is not broken
- **WHEN** the repositories table displays an entry with source `hyperdedic`
- **THEN** the source badge shows "HyperDedic" intact on a single line

#### Scenario: Origin label is not broken
- **WHEN** the repositories table on the F45+ layout displays an entry with origin `packaged`
- **THEN** the origin badge shows "packaged" intact on a single line

### Requirement: Table columns keep stable widths

The repositories table SHALL keep fixed column width shares independent of cell content. Content that does not fit its column SHALL wrap or truncate within its own cell and SHALL NOT shift neighboring columns' content, shift the row height unevenly, or overlap another column.

#### Scenario: Long URL wraps within its own cell
- **WHEN** the repositories table displays an entry with a URL longer than its column
- **THEN** the URL wraps within the URL column and the neighboring Action column keeps its position

#### Scenario: Long repository id or file name does not displace other columns
- **WHEN** the repositories table displays an entry whose id or file name is longer than its column
- **THEN** the content wraps within its own column and the Enabled, Source, URL, and Action columns keep their positions

#### Scenario: Action cell spans the full row height
- **WHEN** the repositories table displays a row whose action buttons stack vertically
- **THEN** the Action cell is a regular table cell spanning the row height and its bottom border aligns with the bottom borders of the other cells in that row

### Requirement: Inventory table columns are sortable

The repositories table SHALL let the user sort its rows by clicking the header of any data column: Repository, File, State, Source, and URL. The Action column SHALL NOT be sortable. Sorting SHALL operate on the already-loaded inventory without re-reading the repository locations, and rows with equal sort keys SHALL keep their current relative order.

#### Scenario: Repository header sorts by id ascending
- **WHEN** the user clicks the "Repository" column header once
- **THEN** the rows are ordered by repository id ascending, case-insensitively

#### Scenario: File header sorts by file name
- **WHEN** the user sorts by the "File" column
- **THEN** the rows are ordered by the file name of each entry, case-insensitively

#### Scenario: Second click reverses the sort direction
- **WHEN** the user clicks an already-sorted column header a second time
- **THEN** the rows are ordered by that column descending

#### Scenario: State header groups by enabled flag
- **WHEN** the user sorts by the "State" column
- **THEN** disabled entries and enabled entries form two groups, with disabled first in ascending order and enabled first in descending order

#### Scenario: Source header sorts by classification
- **WHEN** the user sorts by the "Source" column
- **THEN** the rows are ordered by the source classification of each entry

#### Scenario: Entries without a URL sort last in both directions
- **GIVEN** the inventory contains entries with a URL and entries without one
- **WHEN** the user sorts by the "URL" column, in either direction
- **THEN** the entries without a URL are listed after all entries with a URL

#### Scenario: Equal keys keep their relative order
- **GIVEN** two rows have the same value in the sorted column
- **WHEN** the sort is applied
- **THEN** those two rows keep their relative order from before the sort

#### Scenario: Action column is not sortable
- **WHEN** the user clicks the "Action" column header
- **THEN** the row order does not change and the header shows no sort indicator

#### Scenario: Default order without user sort
- **GIVEN** the user has not clicked any column header since the panel loaded
- **WHEN** the inventory is rendered
- **THEN** rows appear in the order the backend delivered (file order)

#### Scenario: Sort selection survives a refresh
- **GIVEN** the user has sorted the table by a column
- **WHEN** the inventory is reloaded (Refresh button or after an applied action)
- **THEN** the fresh entries are shown in the previously chosen sort order

#### Scenario: Headers expose the active sort state
- **WHEN** a column is sorted
- **THEN** its header shows the sort direction visually and via the `aria-sort` attribute, and sortable headers are operable from the keyboard

