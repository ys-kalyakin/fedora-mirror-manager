# Tasks

## 1. fmm-core: repository inventory

- [x] 1.1 Add error variants (`io` with path, `parse` with file + reason) to `fmm-core::error` and implement `reposd.rs`: line-preserving parser producing sections (id, line range) with per-section `enabled` and the first uncommented `baseurl`/`metalink`/`mirrorlist`; add fixture `.repo` files; verify `cargo test -p fmm-core` passes for a fixture set (fedora/rpmfusion/epel files) and a non-UTF-8 file yields a per-file parse error
- [x] 1.2 Implement source classification (`yandex` / `hyperdedic` / `metalink` / `other` / `none` from the first URL line) with unit tests; verify `cargo test -p fmm-core` covers each source kind including a foreign `baseurl` host
- [x] 1.3 Implement `inventory(repo_dir)` returning entries (id, name, file, enabled, source, url, switchability placeholder, `redhat.repo` marked never-rewritten); verify `cargo test -p fmm-core` covers a populated fixture dir, an empty dir ("no repositories" result), and a missing dir (error)

## 2. fmm-core: classifier and mirror catalog

- [x] 2.1 Port the reference classifier into `classify.rs` preserving its precedence (copr → openh264/cisco → source/srpm/debuginfo/playground → rpmfusion → epel → fedora → skip, with dot-to-space normalization); verify a table-driven `cargo test -p fmm-core` covers fedora/updates/updates-testing/modular, RPM Fusion free+nonfree × release/updates/testing, epel/epel-testing/epel-modular, `fedora-cisco-openh264`, a COPR id, `google-chrome`, and `redhat.repo`
- [x] 2.2 Implement `catalog.rs` with per-key baseurl templates for yandex and hyperdedic (literal `$releasever`/`$basearch`) and the cisco codecs URL; verify a table-driven test asserts every key→URL pair equals the values documented in the reference README and that HyperDedic is unavailable for RPM Fusion keys
- [x] 2.3 Fold classification into inventory entries (`switchable` / `cisco` / `not-switchable`); verify `cargo test -p fmm-core` shows a switchable `updates`, cisco `fedora-cisco-openh264`, and not-switchable `google-chrome` from the fixture dir

## 3. fmm-core: switch plans

- [x] 3.1 Implement the section-scoped rewriter: remove `baseurl`/`metalink`/`mirrorlist` lines (including commented) in the target section, insert the new `baseurl=` directly under the header; verify tests assert sibling sections are byte-identical and `gpgcheck`/`gpgkey`/`enabled` lines are untouched
- [x] 3.2 Implement cisco plan modes (`disable` → `enabled=0`, `codecs` → codecs baseurl + `enabled=1`, `keep` → no-op); verify `cargo test -p fmm-core` covers both applied modes and the keep no-op
- [x] 3.3 Implement the plan model and builder (file path, `before_sha256` of current content, full new content, preview data: new baseurl + removed lines) with plan JSON serialization and an `already-on-mirror` no-change result; verify serialization round-trip and idempotence tests pass

## 4. fmm-core: snapshots and restore plans

- [x] 4.1 Implement snapshot naming (UTC timestamp + action tag + collision suffix), listing (parse snapshot dir names newest-first with tag), and prune-to-10 selection; verify tests on temp dirs cover naming, ordering, and the prune set
- [x] 4.2 Implement restore preview (extra `*.repo` files in the repo dir vs the snapshot, purge list) and restore plan build; verify tests cover extra-files detection and both purge modes

## 5. fmm-apply: privileged helper crate

- [x] 5.1 Scaffold `crates/fmm-apply` (bin) in the workspace: reads the plan JSON from stdin, accepts `--repo-dir`/`--snapshot-root` (defaults `/etc/yum.repos.d`, `/etc/yum.repos.d.bak`), writes the report JSON to stdout; verify `cargo check -p fmm-apply` and `cargo clippy -p fmm-apply -- -D warnings` pass
- [x] 5.2 Implement the switch action: create snapshot preserving permissions → verify each file's sha256 against `before_sha256` (abort with `stale_plan` before any write) → in-place truncate+write+fsync → prune to 10; verify integration tests on temp dirs cover a successful switch, a no-change skip, a stale-plan abort leaving files untouched, and a snapshot-failure abort
- [x] 5.3 Implement the restore action: copy snapshot contents preserving permissions, keep or delete (purge) extra `*.repo` files, report `extra_files`; verify integration tests cover byte-for-byte restore with purge on and off
- [x] 5.4 Implement the optional cache refresh (`dnf clean all` + `dnf makecache`, `--dnf-bin` override for tests, output tail in report, `makecache` failure → warning not failure); verify integration tests with a fake dnf stub cover success, disabled, and failing-makecache cases

## 6. fmm-app: commands and error mapping

- [x] 6.1 Add commands `list_repos`, `list_snapshots`, `preview_action`, `preview_restore` over `fmm-core`, plus a serializable `{ code, message }` command error; verify `cargo test -p fmm-app` (or `cargo check` where commands are thin) and clippy pass
- [x] 6.2 Add `apply_plan`/`apply_restore`: resolve helper path (`FMM_APPLY_HELPER` override, default `/usr/libexec/fmm-apply`), detect missing helper as code `helper_missing`, spawn `pkexec` with the plan on stdin, parse the report; verify tests with a stub helper script via the env override cover success, report-error, and missing-helper paths without polkit
- [x] 6.3 Register all commands in `src-tauri/src/lib.rs`; verify `cargo check -p fmm-app` and `cargo clippy --workspace -- -D warnings` pass

## 7. UI: repository view and switching

- [x] 7.1 Implement `repos.ts`: inventory table (id, file, enabled, source badge, URL, switchability), refresh action, loading/error/empty/missing-dir states; verify `npm run build` and `tsc --noEmit` pass in `ui/`
- [x] 7.2 Implement switching controls: mirror choice per row (HyperDedic disabled for RPM Fusion with a hint), cisco mode controls, already-on-mirror indication, preview dialog (new baseurl, removed lines, cache-refresh checkbox default on) calling `apply_plan`, and report/error states (`denied`, `stale_plan`, `helper_missing`); verify `npm run build` and `tsc --noEmit` pass

## 8. UI: snapshots panel and restore

- [x] 8.1 Implement `snapshots.ts` and shared `apply.ts`: snapshot list newest-first with tag/timestamp, empty state, restore dialog (snapshot choice, purge option, refresh option) calling `apply_restore`, report/error rendering; wire both panels into `shell.ts` replacing the mirror-list placeholder; verify `npm run build` and `tsc --noEmit` pass

## 9. Helper installation and documentation

- [x] 9.1 Author `packaging/polkit/org.fmm.desktop.apply.policy` (action `org.fmm.desktop.apply`, annotation `org.freedesktop.policykit.exec.path=/usr/libexec/fmm-apply`, defaults `auth_admin_keep` for active sessions); verify the file parses as XML (`xmllint --noout`)
- [ ] 9.2 Document in README: helper + policy install commands (`sudo install` of the built `fmm-apply` and the policy), `FMM_APPLY_HELPER` dev override, missing-helper degradation, and a troubleshooting row for denied/stale-plan states; verify each documented command runs as written on this machine

## 10. Integration verification

- [x] 10.1 Run `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, and `cd ui && npm run build`; verify all pass
- [ ] 10.2 Manual dev-run check with the helper installed: the inventory shows this system's real repositories with correct sources; previewing a switch shows the planned baseurl; switching one repository prompts polkit and succeeds; a new snapshot appears; restoring it via the UI returns the directory byte-for-byte (verify with `diff -r` against a fresh snapshot); closing the window exits the process

## 11. In-app privileged helper setup

- [x] 11.1 Add `helper_status` and `install_helper` commands (embedded polkit policy, helper source resolution, two `pkexec /usr/bin/install` steps, temp-file cleanup, injectable pkexec for tests); verify `cargo test -p fmm-app` covers resolution, step building, and a stubbed install flow, and `cargo clippy --workspace -- -D warnings` passes
- [x] 11.2 Add the UI setup flow: helper-missing banner with an "Install helper" button in the repositories and snapshots panels, and an "Install helper" action inside the `helper_missing` error dialog; verify `tsc --noEmit` and `npm run build` pass
- [x] 11.3 Update README: in-app setup as the primary path, manual commands as fallback; verify the documented text matches the implemented flow

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
- Verify the archived result.
