# Spec Delta: snapshot-rollback

## MODIFIED Requirements

### Requirement: Automatic snapshot before every privileged write

Before any privileged write (mirror switch, Cisco mode change, restore) is performed, the system SHALL snapshot the entire `/etc/yum.repos.d` directory into a new directory under `/etc/yum.repos.d.bak` named with a UTC timestamp and a tag naming the action. The snapshot SHALL preserve file permissions and ownership. On the F45+ layout `/etc/yum.repos.d` is exactly the state the application can modify (third-party files plus its own overriding copies); packaged files under `/usr/share/dnf5/repos.d` SHALL never be snapshotted, modified, or restored.

#### Scenario: Snapshot created before a switch
- **WHEN** an authorized mirror switch is about to modify a repository file
- **THEN** a new snapshot directory appears under `/etc/yum.repos.d.bak` before the write happens

#### Scenario: F45+ snapshot covers only the writable location
- **GIVEN** the F45+ layout
- **WHEN** an authorized mirror switch creates its pre-write snapshot
- **THEN** the snapshot contains only `/etc/yum.repos.d` files and no files from `/usr/share/dnf5/repos.d`

#### Scenario: Snapshot failure aborts the write
- **GIVEN** the snapshot cannot be created (e.g. the backup root is not writable)
- **WHEN** the privileged write is attempted
- **THEN** the write is not performed and an error naming the cause is shown

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
