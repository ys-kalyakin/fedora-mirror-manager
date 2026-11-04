# snapshot-rollback Specification

## Purpose

Protects the user against bad writes: every privileged modification of `/etc/yum.repos.d` is preceded by an automatic snapshot, snapshots are visible in the UI, and the whole directory can be restored byte-for-byte from any snapshot — including the state that existed before the first mirror switch.

## Requirements

### Requirement: Automatic snapshot before every privileged write

Before any privileged write (mirror switch, Cisco mode change, restore) is performed, the system SHALL snapshot the entire `/etc/yum.repos.d` directory into a new directory under the user snapshot root — `$XDG_DATA_HOME/fmm/snapshots`, which defaults to `~/.local/share/fmm/snapshots` — named with a UTC timestamp and a tag naming the action. Snapshots SHALL be stored only under the user snapshot root and never under a system directory. The snapshot SHALL preserve file permissions and ownership, and the system SHALL hand ownership of the snapshot it creates to the invoking user, so the unprivileged application can list, preview and delete snapshots without authorization. On the F45+ layout `/etc/yum.repos.d` is exactly the state the application can modify (third-party files plus its own overriding copies); packaged files under `/usr/share/dnf5/repos.d` SHALL never be snapshotted, modified, or restored.

#### Scenario: Snapshot created before a switch
- **WHEN** an authorized mirror switch is about to modify a repository file
- **THEN** a new snapshot directory appears under the user snapshot root before the write happens

#### Scenario: Snapshots live only in the user directory
- **WHEN** any privileged write creates its pre-write snapshot
- **THEN** the snapshot directory is created under `$XDG_DATA_HOME/fmm/snapshots` (default `~/.local/share/fmm/snapshots`) and nothing is written under `/etc/yum.repos.d.bak`

#### Scenario: Snapshot ownership is handed to the user
- **GIVEN** the user snapshot root exists and is owned by the invoking user
- **WHEN** the privileged helper creates a snapshot under it
- **THEN** the snapshot directory and its contents end up owned by the invoking user, and the unprivileged application can list, preview and delete it without any authorization prompt

#### Scenario: F45+ snapshot covers only the writable location
- **GIVEN** the F45+ layout
- **WHEN** an authorized mirror switch creates its pre-write snapshot
- **THEN** the snapshot contains only `/etc/yum.repos.d` files and no files from `/usr/share/dnf5/repos.d`

#### Scenario: Snapshot failure aborts the write
- **GIVEN** the snapshot cannot be created (e.g. the snapshot root is not writable)
- **WHEN** the privileged write is attempted
- **THEN** the write is not performed and an error naming the cause is shown

### Requirement: Snapshot retention

The system SHALL keep the 10 most recent snapshots in the user snapshot root (`$XDG_DATA_HOME/fmm/snapshots`) and prune older ones after creating a new snapshot. Manually deleted snapshots are not counted against the retention window.

#### Scenario: Oldest snapshots are pruned
- **GIVEN** 10 snapshots already exist
- **WHEN** a new privileged write creates an 11th snapshot
- **THEN** the oldest snapshot directory is removed and 10 remain

#### Scenario: Manual deletion frees the retention window
- **GIVEN** 10 snapshots exist and the user deletes one
- **WHEN** a new privileged write creates a snapshot
- **THEN** nothing is pruned and 10 snapshots remain

### Requirement: Snapshot list in the UI

The system SHALL show the existing snapshots newest first, each with its timestamp and action tag, and SHALL show an explicit empty state when no snapshots exist. The list SHALL read the user snapshot root (`$XDG_DATA_HOME/fmm/snapshots`) directly from the unprivileged application. Refreshing the snapshot list SHALL re-read the snapshot directory from disk.

#### Scenario: Snapshot list shown
- **GIVEN** snapshots exist in the user snapshot root
- **WHEN** the user opens the snapshots panel
- **THEN** the snapshots are listed newest first with timestamp and tag

#### Scenario: Empty snapshot list
- **GIVEN** no snapshots exist
- **WHEN** the user opens the snapshots panel
- **THEN** an explicit "no snapshots yet" state is shown

### Requirement: Restore from a chosen snapshot

The system SHALL allow restoring `/etc/yum.repos.d` from a user-chosen snapshot after polkit authorization. Restore SHALL copy the snapshot contents preserving permissions and ownership; `*.repo` files that appeared after the snapshot SHALL be kept by default and reported, with an explicit option to delete them. On the F45+ layout restore SHALL NOT touch the packaged files under `/usr/share/dnf5/repos.d`: restoring to a snapshot taken before any switches removes the overriding copies created after that snapshot (with the delete option), returning the repositories to their packaged defaults.

#### Scenario: Restore returns the directory to the snapshot state
- **GIVEN** a snapshot taken before any switching
- **WHEN** the user restores that snapshot and grants authorization
- **THEN** every `*.repo` file matches the snapshot content byte-for-byte

#### Scenario: Files added after the snapshot are reported
- **GIVEN** a `*.repo` file was created after the chosen snapshot
- **WHEN** the user restores that snapshot without the delete option
- **THEN** the file remains on disk and the UI reports which files were extra

#### Scenario: Purge option removes later files
- **GIVEN** a `*.repo` file was created after the chosen snapshot
- **WHEN** the user restores with the "delete files added after snapshot" option enabled
- **THEN** the extra file is removed and the directory matches the snapshot exactly

#### Scenario: F45+ restore to pre-switch state removes overriding copies
- **GIVEN** the F45+ layout, a snapshot taken before any switches, and overriding copies created after it
- **WHEN** the user restores that snapshot with the delete option enabled
- **THEN** the overriding copies are removed from `/etc/yum.repos.d` and the packaged files under `/usr/share/dnf5/repos.d` remain unchanged

#### Scenario: Restore honors the cache refresh option
- **GIVEN** the refresh option is enabled
- **WHEN** a restore completes successfully
- **THEN** `dnf clean all` and `dnf makecache` run as root, and a `makecache` failure is reported as a warning without marking the restore failed

### Requirement: Deleting a single snapshot

The system SHALL allow the user to delete one specific snapshot from the snapshots panel after an explicit confirmation. Deletion SHALL remove only the chosen snapshot directory, SHALL require no polkit authorization because snapshots live in the user's own data directory, and SHALL refresh the list afterwards. A snapshot that no longer exists on disk SHALL surface a clear error instead of failing silently.

#### Scenario: Delete with confirmation
- **GIVEN** at least one snapshot exists
- **WHEN** the user clicks the delete control of a snapshot and confirms the dialog
- **THEN** that snapshot's directory is removed from the user snapshot root and the remaining snapshots are listed

#### Scenario: Cancel keeps the snapshot
- **WHEN** the user dismisses the delete confirmation
- **THEN** the snapshot remains on disk and stays listed

#### Scenario: Deleting an already-missing snapshot
- **GIVEN** the snapshot list is stale
- **WHEN** the user deletes a snapshot that no longer exists on disk
- **THEN** a clear error is shown and the refreshed list no longer contains it

### Requirement: One-click migration of legacy snapshots

When snapshots exist under the legacy root `/etc/yum.repos.d.bak`, the system SHALL offer a one-click migration that moves every legacy snapshot into the user snapshot root under a single polkit authorization, hands ownership of the moved snapshots to the invoking user, and removes the legacy directory when it is left empty. Snapshots still in the legacy root SHALL NOT appear in the snapshot list until they are migrated.

#### Scenario: Migration notice with legacy snapshots
- **GIVEN** `/etc/yum.repos.d.bak` contains at least one snapshot
- **WHEN** the user opens the snapshots panel
- **THEN** a notice offers to move the legacy snapshots into the user snapshot root

#### Scenario: Migration moves the snapshots out of the system directory
- **WHEN** the user accepts the migration and grants the single authorization
- **THEN** every legacy snapshot is moved into the user snapshot root owned by the invoking user, listed together with any snapshots created later, and the legacy directory is removed when it is empty

#### Scenario: Dismissed migration keeps the legacy state
- **WHEN** the user dismisses the migration notice or the authorization
- **THEN** the legacy snapshots remain untouched under `/etc/yum.repos.d.bak` and the notice is offered again

#### Scenario: No notice without legacy snapshots
- **GIVEN** `/etc/yum.repos.d.bak` does not exist or contains no snapshots
- **WHEN** the user opens the snapshots panel
- **THEN** no migration notice is shown
