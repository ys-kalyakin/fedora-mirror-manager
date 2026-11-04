# Design

## Context

The app is a Tauri 2 shell with a placeholder UI (`bootstrap-project-structure`, in flight). Domain logic lives in `fmm-core` (currently placeholder `Repo`/`Mirror` structs); `fmm-app` is a thin command adapter. The bootstrap design reserved room for polkit-driven editing of `/etc/yum.repos.d` — this change introduces that mechanism.

The reference implementation `mirror-switch.sh` (repo `/home/helloween/projects/mirror-switch`) validates the domain rules: section classification by id/file name, per-family baseurl templates for two fixed mirrors, section-scoped rewriting (drop `baseurl`/`metalink`/`mirrorlist`, insert new `baseurl` under the header), whole-directory snapshots with retention 10, and `gpgcheck` preservation. Reading `/etc/yum.repos.d` requires no privileges; writing requires root.

## Goals / Non-Goals

**Goals:**

- Port the reference script's domain logic (classifier, URL catalog, section rewrite, snapshot naming/pruning) into `fmm-core` as pure, unit-testable code.
- A plan-based privileged apply: the unprivileged app computes exact file contents, a small root helper applies them after polkit authorization.
- UI views: repository inventory with per-repo switching controls, snapshot panel with restore.
- Graceful degradation: the read-only view works even when the privileged helper is not installed.

**Non-Goals:**

- RPM/Flatpak packaging of the helper and policy (install steps documented in README; packaging is a later change).
- Mirror discovery, ranking, or speed measurement (a later change per the README roadmap).
- Configurable/extendable mirror catalogs — fixed Yandex + HyperDedic, as in the reference.
- Localization; UI strings stay in English.
- Batch/global switching — user chose per-repository granularity.

## Decisions

### D1. Three-part architecture: `fmm-core` (logic) → `fmm-app` (commands) → `fmm-apply` (root helper)

```
ui/ (TypeScript)                fmm-app (Tauri commands)          fmm-apply (root helper)
  list view  ──invoke──▶ list_repos() ──▶ fmm-core parse/classify
  switch     ──invoke──▶ preview_action() ──▶ fmm-core build plan
             ──invoke──▶ apply_plan() ──pkexec──▶ snapshot → verify → write → (dnf refresh)
  snapshots  ──invoke──▶ list_snapshots() / preview_restore() / apply_restore()
```

- `fmm-core` gains modules: `reposd` (line-preserving `.repo` parser + section-scoped rewriter), `classify` (port of the script's `repo_key` ordering: copr → cisco/openh264 → source/debuginfo/playground → rpmfusion → epel → fedora → skip), `catalog` (fixed mirror catalog: per-family baseurl templates with literal `$releasever`/`$basearch`; HyperDedic marked unavailable for RPM Fusion keys), `snapshot` (timestamped naming with collision suffix, prune-to-10, listing, restore preview with extra-file detection), `plan` (apply-plan model, plan serialization), and extended `error`.
- `fmm-app` commands: `list_repos`, `preview_action`, `apply_plan`, `list_snapshots`, `preview_restore`, `apply_restore`. Commands are thin: build/serialize plans via `fmm-core`, spawn the helper, parse its report.
- **Alternative rejected**: the helper re-parses and re-computes everything itself as root. Duplicates domain logic in a second place and makes testing require root; the plan approach keeps all logic unprivileged and testable.

### D2. Plan protocol with staleness detection

The plan is JSON on the helper's stdin; the report is JSON on its stdout.

```json
// plan in
{ "plan_version": 1, "action": "switch" | "restore",
  "snapshot_tag": "switch-updates-yandex",
  "refresh_cache": true,
  "purge": false,                       // restore only
  "snapshot": "/etc/yum.repos.d.bak/…", // restore only
  "files": [ { "path": "/etc/yum.repos.d/fedora.repo",
               "before_sha256": "…",    // hash of current on-disk content
               "content": "…full new file…" } ] }
// report out
{ "ok": true, "snapshot": "…", "changed": ["…"], "unchanged": ["…"],
  "extra_files": [],                    // restore: files kept that were not in the snapshot
  "cache": { "ran": true, "ok": true, "output_tail": "…" } }
```

- The helper aborts with `stale_plan` if a file's current sha256 does not match `before_sha256` (the file changed between preview and apply — TOCTOU guard). **Alternative rejected**: no check — a stale write could silently revert someone else's concurrent change.
- Writes are performed in place (open with truncate, write, fsync) like the script's `cat > file`, preserving inode, owner, mode, and SELinux context. **Alternative rejected**: write-temp-then-rename — creates a fresh inode and loses the SELinux label of the original, which matters on Fedora.
- On any per-file mismatch or error the helper stops before the first write and reports `ok: false` with a machine-readable `code` (`stale_plan`, `snapshot_failed`, `io`, `dnf_warning` is not an error) — no partial application.

### D3. Privileged execution: `pkexec` + polkit action policy

- New binary crate `crates/fmm-apply` (bin `fmm-apply`), installed to `/usr/libexec/fmm-apply`, owned by root. Invoked as `pkexec /usr/libexec/fmm-apply [--repo-dir DIR] [--snapshot-root DIR]`; paths default to `/etc/yum.repos.d` and `/etc/yum.repos.d.bak` (arguments exist for tests, which run the helper directly on temp dirs without root).
- Polkit policy file `org.fmm.desktop.apply.policy` (action `org.fmm.desktop.apply`, annotation `org.freedesktop.policykit.exec.path=/usr/libexec/fmm-apply`) installed to `/usr/share/polkit-1/actions/`. Defaults: `auth_admin_keep` for active sessions — repeated actions within the cache window do not re-prompt; **alternative rejected**: `yes` (unattended root writes) and plain `auth_admin` (prompt on every action).
- The app resolves the helper path from `FMM_APPLY_HELPER` (dev/test override), default `/usr/libexec/fmm-apply`. If the helper or policy is missing, switching/restore controls show a "privileged helper not installed" state with the two install commands from the README, and the read-only view keeps working.
- **Alternative rejected**: running the whole app under `pkexec`/sudo — root GUI process, far larger attack surface; and D-Bus system service — heavier plumbing than one-shot `pkexec` for a desktop tool.

### D4. Classifier and URL catalog ported verbatim from the reference

The classification order and the per-family URL templates are copied from `mirror-switch.sh` (`repo_key`, `baseurl_for`) into `fmm-core` data tables, including the dot-to-space normalization of id/file names, the HyperDedic/RPM-Fusion exclusion, and the cisco template `https://codecs.fedoraproject.org/openh264/$releasever/$basearch/`. Unit tests assert the full key→URL table against the values documented in the reference README, so a divergence from the validated mapping is a test failure. Unknown sections classify as not switchable — safe default.

### D5. `.repo` parsing is line-preserving and section-scoped

The parser keeps the file as raw lines (UTF-8 required; a non-UTF-8 file is a per-file parse error). It maps section headers to their line ranges and extracts per section: `enabled`, and the first of `baseurl`/`metalink`/`mirrorlist` (uncommented). The rewriter operates on one section: remove `baseurl`/`metalink`/`mirrorlist` lines (commented or not), insert the new `baseurl=` (and `enabled=` for cisco) directly under the header, leave every other byte untouched. Files in `SKIP_FILES` (`redhat.repo`) are displayed but never rewritten.

### D6. Frontend stays framework-free with small view modules

`ui/src` gains `repos.ts` (table view, per-row mirror/Cisco controls, preview dialog, error states), `snapshots.ts` (list, restore dialog with purge and refresh options), and `apply.ts` (plan → `apply_plan` invoke, report rendering). `shell.ts` keeps bootstrapping and hosts the refresh action. This follows bootstrap D3; the Tauri seam does not change.

### D7. Error surface

`fmm-core::error::Error` gains variants for IO (with path), parse failures (file + reason), stale plan, snapshot failure, and helper failure (code + message). `fmm-app` maps them to a serializable `{ code, message }` command error; the UI renders code-specific states (denied → "not authorized", stale_plan → "repositories changed on disk, refresh and retry", missing helper → install hint).

### D8. In-app helper setup (no pre-installation required)

The app offers one-click setup for the privileged components: the polkit policy is embedded into `fmm-app` at build time (`include_str!` of `packaging/polkit/…`) and written to a fresh temp file; the helper source is resolved from `FMM_APPLY_HELPER`, then next to the running executable (dev layout `target/{debug,release}/fmm-apply`), then the installed path (binary step is skipped — idempotent). Both files are installed by invoking `pkexec /usr/bin/install …` with direct arguments. `/usr/bin/install` is a root-owned system binary, so pkexec needs no pre-installed project component, and passing arguments without a shell keeps the call injection-free. **Alternative rejected:** a single `pkexec /bin/sh -c "install … && install …"` — one prompt instead of two, but the authorization dialog then shows a bare shell invocation instead of the concrete install command; the app states up front that two prompts appear.

## Risks / Trade-offs

- [Bad write can break package management] → unconditional pre-write snapshot, staleness hash check, preview-before-confirm, and tests asserting `gpgcheck`/sibling sections are untouched; worst case is a documented one-click restore.
- [pkexec refuses non-root-owned binaries] → README dev step installs the built helper as root (`sudo install`); the app's missing-helper state explains this instead of failing cryptically.
- [No live progress during `dnf makecache`] → the UI shows an in-progress state until the helper reports; `output_tail` of the cache step is included in the report. Accepted for this change.
- [Snapshot directory unreadable to a non-root user on some systems] → snapshot list degrades to the explicit error state with a hint; restore/switch still work (the helper reads it as root).
- [Polkit dialog behavior cannot be automated headlessly] → manual verification tasks in tasks.md; automated coverage covers parser, classifier, plan build, and helper logic on temp dirs.

## Migration Plan

Additive change; no data migration. Rollback is `git revert`. The helper and policy are optional at runtime — without them the app is a functional read-only viewer.

## Open Questions

None blocking. Polkit defaults (`auth_admin_keep`), helper install path, and protocol version are decided above and revisitable during implementation without changing the specs.
