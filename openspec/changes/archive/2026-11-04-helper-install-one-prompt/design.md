# Design

## Context

`install_helper_with` (`src-tauri/src/commands.rs`) currently runs one `pkexec /usr/bin/install` per file: the helper binary, then the polkit policy (staged to a temp file by `write_policy_temp`). Each pkexec run triggers its own polkit authorization against the generic `org.freedesktop.policykit.exec` action (`auth_admin`, no keep), hence two password prompts. The app's own policy action (`org.fmm.desktop.apply`, `auth_admin_keep`) only applies to `/usr/libexec/fmm-apply` — it cannot cover the install steps, and polkit offers no way to reuse one authorization across two pkexec processes. Constraint: pkexec refuses to execute binaries not owned by root, which is why the flow stages installs through root-owned `/usr/bin/install` today.

## Goals / Non-Goals

- Goals: exactly one polkit prompt per setup run; preserve idempotent reinstall, temp-policy cleanup, `denied`/`helper_missing` error codes, `InstallReport` shape.
- Non-Goals: no change to the apply/restore helper invocation path; no packaging (RPM/spec) changes; no edits to the polkit policy; no polkit agent or action changes.

## Decisions

### D1: Merge both installs into one `pkexec /bin/sh -c '…'` invocation

The privileged part becomes a single command:

```
pkexec /bin/sh -c '/usr/bin/install -D -m 0755 -o root -g root '<helper_src>' '<INSTALLED_HELPER>' && /usr/bin/install -D -m 0644 -o root -g root '<policy_src>' '<POLICY_INSTALLED>'
```

(with every interpolated path passed through a shell-quoting helper). `&&` makes the install all-or-nothing per authorization.

Alternatives rejected:
- **Helper self-install mode (`pkexec <helper_src> --install …`)** — impossible: pkexec refuses user-owned binaries.
- **A second polkit action annotating a system binary** (`/usr/bin/install`, `/bin/sh`) — would re-bind that action for every application on the system; unacceptable.
- **`pkexec /usr/bin/xargs -0` fed the argument pairs on stdin** — stays shell-free but is obscure, fragile around exact argument counts, and needs stdin plumbing; harder to review than a quoted one-liner.
- **Building a tarball and extracting as root** — needs a new dependency and has the same quoting-free appeal but worse auditability for a two-file copy.

The original "no shell, no quoting" choice served two separate simple invocations; merging to one prompt forces a minimal shell wrapper. All interpolated values are paths the app itself resolves or generates (env override, exe dir, self-named temp file, fixed destinations) — no user-typed input reaches the script.

### D2: Strict quoting helper with its own unit tests

A small `shell_quote` (single-quote wrapper with `'\''` escaping) applied to every interpolated path; unit-tested against spaces and embedded single quotes. This is the injection boundary.

### D3: Generalize `run_pkexec` to `OsString` arguments

The script is a `String`, not a `PathBuf`; change the argument slice to `OsString` (or `Vec<String>`) and adapt existing call sites/tests. Failure mapping (non-zero exit / spawn failure → `denied`, stderr as detail) stays unchanged.

### D4: Script builder replaces the step list

`install_steps`/`install_step` are replaced by a builder that returns the shell script plus the logical destination list for `InstallReport` (helper dest only when the source is not already the installed path, plus the policy dest always). The report keeps its current meaning: destinations of the installs the run performed.

## Risks / Trade-offs

- [Quoting bug in the shell one-liner] → dedicated quoting helper, unit tests with spaces/quotes, and only app-controlled values interpolated.
- [Polkit dialog shows a generic interpreter name instead of `/usr/bin/install`] → cosmetic; the dialog still requires the admin password and names root ownership. Accepted.
- [Behavior change: previously the helper could be installed while the policy step was denied; now a dismissal installs nothing] → intended atomicity improvement; the error path and `denied` code are unchanged.
- [`/bin/sh` path assumption] → Fedora guarantees `/bin/sh` (usrmerge); pkexec requires an absolute path, which `/bin/sh` satisfies.

## Migration Plan

No data or config migration. The change is internal to the install invocation plus copy updates (`ui/src/apply.ts`, `README.md`). Rollback is a plain git revert. A previously half-installed state (helper present, policy missing) remains recoverable by running setup again, as before.

## Open Questions

None.
