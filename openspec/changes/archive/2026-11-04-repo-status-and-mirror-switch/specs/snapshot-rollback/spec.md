# Spec Delta

## Purpose

Protects the user against bad writes: every privileged modification of `/etc/yum.repos.d` is preceded by an automatic snapshot, snapshots are visible in the UI, and the whole directory can be restored byte-for-byte from any snapshot — including the state that existed before the first mirror switch.

## ADDED Requirements

### Requirement: Automatic snapshot before every privileged write

Before any privileged write (mirror switch, Cisco mode change, restore) is performed, the system SHALL snapshot the entire `/etc/yum.repos.d` directory into a new directory under `/etc/yum.repos.d.bak` named with a UTC timestamp and a tag naming the action. The snapshot SHALL preserve file permissions and ownership.

#### Scenario: Snapshot created before a switch
- **WHEN** an authorized mirror switch is about to modify a repository file
- **THEN** a new snapshot directory appears under `/etc/yum.repos.d.bak` before the write happens

#### Scenario: Snapshot failure aborts the write
- **GIVEN** the snapshot cannot be created (e.g. the backup root is not writable)
- **WHEN** the privileged write is attempted
- **THEN** the write is not performed and an error naming the cause is shown

### Requirement: Snapshot retention

The system SHALL keep the 10 most recent snapshots in `/etc/yum.repos.d.bak` and prune older ones after creating a new snapshot.

#### Scenario: Oldest snapshots are pruned
- **GIVEN** 10 snapshots already exist
- **WHEN** a new privileged write creates an 11th snapshot
- **THEN** the oldest snapshot directory is removed and 10 remain

### Requirement: Snapshot list in the UI

The system SHALL show the existing snapshots newest first, each with its timestamp and action tag, and SHALL show an explicit empty state when no snapshots exist. Refreshing the snapshot list SHALL re-read the snapshot directory from disk.

#### Scenario: Snapshot list shown
- **GIVEN** snapshots exist in `/etc/yum.repos.d.bak`
- **WHEN** the user opens the snapshots panel
- **THEN** the snapshots are listed newest first with timestamp and tag

#### Scenario: Empty snapshot list
- **GIVEN** no snapshots exist
- **WHEN** the user opens the snapshots panel
- **THEN** an explicit "no snapshots yet" state is shown

### Requirement: Restore from a chosen snapshot

The system SHALL allow restoring `/etc/yum.repos.d` from a user-chosen snapshot after polkit authorization. Restore SHALL copy the snapshot contents preserving permissions and ownership; `*.repo` files that appeared after the snapshot SHALL be kept by default and reported, with an explicit option to delete them.

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

#### Scenario: Restore honors the cache refresh option
- **GIVEN** the refresh option is enabled
- **WHEN** a restore completes successfully
- **THEN** `dnf clean all` and `dnf makecache` run as root, and a `makecache` failure is reported as a warning without marking the restore failed
