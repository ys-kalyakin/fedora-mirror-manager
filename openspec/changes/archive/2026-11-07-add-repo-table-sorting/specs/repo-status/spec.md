# Spec Delta

## ADDED Requirements

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
