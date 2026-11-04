# Spec Delta

## ADDED Requirements

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
