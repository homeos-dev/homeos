# homeos — Developer Spec

This document captures homeos's _internal_ spec — data structures, execution model, output contracts, and invariants that a developer (or AI agent implementing a task) needs to know but a user does not. End users read `README.md`; product intent and the tasks ledger live in `PRD.md`; coding style and Tech Stack live in `CONVENTIONS.md`; CLI command output specifics live in `COMMAND_OUTPUT.md` alongside this file in `SPEC/`.

## Data model

### homeos.yml

```yaml
packages:
  neovim:
    script_aliases: { update: install }
    enabled: false
    depends_on: [dnf-copr-mise]
    plugin: dnf
    params: { name: neovim }
plugins:
  dnf:
    url: https://github.com/hainet50b/homeos-plugin-dnf
```

- `packages.<name>.script_aliases` redirects one action to another's script (e.g., `update` runs `install.sh`)
- `enabled` defaults to `true` when omitted
- `archived` defaults to `false` when omitted and is omitted from serialization when `false`; `true` marks a package no machine should have installed (see Package lifecycle). `enabled` keeps its last value underneath and resurfaces on unarchive
- `depends_on` is omitted from serialization when empty
- `plugin` and `params` are present only on plugin-backed packages
- `plugins.<name>.url` is optional; absent for `plugin add --local` plugins, and the `url` field is omitted from the YAML when absent

### state.yml

Tracks which packages have been successfully installed on this machine. Excluded from version control via the `.gitignore` written by `homeos init`.

```yaml
installed:
  - neovim
  - zed
```

- `homeos package install <name>` appends after successful execution; the entry survives even if a later package in the batch fails
- `homeos package uninstall <name>` removes after successful execution
- `homeos package uninstall --all` reads this list

### plugin.yml (inside `plugins/<name>/`)

```yaml
description: DNF package manager plugin for homeos.
params:
  - name
```

- `description` is required (one-line summary; displayed by `plugin list`, `plugin info`, `plugin list-remote`)
- `params` is the parameter list expected by the templates; `package add --plugin <name> --param key=value` validates that every required param is supplied and substitutes `{{ key }}` in each `*.tmpl`

### .last-update-check (`<data_dir>/.last-update-check`)

JSON cache for the "update available" notification triggered by `homeos cd`:

```json
{ "last_checked_at": 1748242899, "latest_tag": "v0.3.9" }
```

`last_checked_at` is Unix epoch seconds. 7-day TTL. The timestamp is always updated after a check attempt (success or network failure) so an offline machine retries at most once per week. Seeded by `homeos init` with the current binary tag so freshly initialized data directories are silent for their first week.

## Directory structure

`homeos init` creates a flat layout at `<data_dir>`:

```
<data_dir>/
├── .gitignore             # ignores state.yml, AGENTS.md, CLAUDE.md, .last-update-check
├── homeos.yml
├── state.yml              # ignored
├── AGENTS.md              # generated; auto-refreshed on homeos cd when binary tag changes (see Agent entry points)
├── CLAUDE.md              # one-line Claude Code pointer: "@AGENTS.md"
├── .last-update-check     # ignored
├── packages/
└── plugins/
```

`<data_dir>` is resolved in this priority:

1. An explicit override passed by the test harness (only used in tests)
2. `HOMEOS_DATA_DIR` env var
3. The OS-appropriate per-user local data directory with `homeos` appended — `~/.local/share/homeos` on Linux, `%LOCALAPPDATA%\homeos` on Windows, `~/Library/Application Support/homeos` on macOS

## Agent entry points

Three entry points deliver the same operating guide to an AI agent. All of them resolve to the binary's `render()` (`templates/AGENTS.md.tmpl` plus the clap-generated command reference), so the guide always matches the installed version and there is no second copy to keep in sync.

| Entry | Where it lives | How the agent gets the guide |
|---|---|---|
| `<data_dir>/AGENTS.md` (+ `CLAUDE.md` pointer) | Data directory; written by `homeos init`, refreshed by `homeos cd` when the version marker is stale | Read automatically when the agent's working directory is the data directory (`homeos cd`, then start the agent) |
| `homeos-manage` skill | `skills/homeos-manage/SKILL.md` in this repository; installed per agent with `gh skill install hainet50b/homeos homeos-manage --scope user --agent <agent>` | Fires when software or an agent skill is about to be installed, updated, uninstalled, or restored on the machine; runs `homeos agents-md` and follows its stdout |
| `homeos-inventory` skill | `skills/homeos-inventory/SKILL.md`; installed the same way | Fires at the start of shell work; reads `homeos package list --json` only — never the guide, never the update notice |

Invariants:

- **Skills are thin.** A `SKILL.md` carries frontmatter (`name`, `description`, `license`) and a few lines that point at a homeos command. Anything that depends on the homeos version lives in the template, never in a skill: `gh skill install` resolves the repository's latest release tag regardless of which binary the user has installed, so a fat skill would drift.
- **The guide is working-directory independent.** It instructs the agent to resolve the data directory once via `homeos cd --print --json`, refers to files as `<data_dir>/...`, and writes every git command as `git -C <data_dir> ...`. A bare `git` command in the guide is a defect: an agent started inside an unrelated project would stage and commit that project's files. `test_render_includes_git_commit_convention` enforces this.
- **Discovery convention.** `gh skill` finds skills at `skills/*/SKILL.md`, and the directory name must equal the frontmatter `name`. `gh skill publish --dry-run` validates both and runs in CI (`build.yml`, `skills` job).
- **`homeos agents-md` works without a data directory.** It renders from the binary alone. An agent restoring a setup on a new machine reads the guide first and runs `homeos init <url>` from it, so nothing in `agents-md` — the update check included — may require or create the data directory.
- **The update notice reaches agents through `homeos agents-md`**, not through the skills (see *Update check*), so `homeos-inventory` sessions never see it.

## Action resolution

Scripts are resolved by convention based on OS and executed as subprocesses:

- Linux / macOS: `install.sh`, `update.sh`, `uninstall.sh` — run with `sh`
- Windows: `install.ps1`, `update.ps1`, `uninstall.ps1` — run with `pwsh` when available, `powershell.exe` (Windows PowerShell 5.1) as fallback, invoked as `-NoProfile -File <script>`

Each action looks up `script_aliases.<action>` first; if present, runs the aliased action's script in the same package directory. Otherwise runs the action's own script.

The subprocess inherits the user's stdin / stdout / stderr (it is not captured), so user-authored scripts can prompt interactively (e.g., for a `sudo` password).

## Package lifecycle

`homeos.yml` is the desired state shared across machines; `state.yml` is this machine's observed state; `apply` reconciles the two in both directions.

- **enabled** (default) — should be installed on every machine.
- **disabled** (`enabled: false`) — frozen: skipped by install / update, kept installed where it is installed.
- **archived** (`archived: true`) — should not be installed anywhere: skipped by install / update, and `apply` uninstalls it wherever `state.yml` still records it. The entry and `packages/<name>/` stay in the repo as a tombstone, so the uninstall scripts reach every machine through git; `package remove` is an optional final cleanup once no machine has it installed. `archived` dominates `enabled`/`disabled` behaviour.

Supporting rules:

- Archiving a package that non-archived packages depend on is refused (`dependent-exists`), mirroring `package remove`. Adding a `depends_on` reference to an archived package is refused (`validation-error`).
- Operate commands (`install` / `update` / `uninstall`) never edit `homeos.yml`. In particular, `uninstall` does not auto-disable (it did before the archived phase existed); after successfully uninstalling a non-archived package it emits a one-line stderr hint pointing at `package archive` (wording in `COMMAND_OUTPUT.md`).
- `apply` reports `state.yml` entries that have no `homeos.yml` definition (report-only, nothing executed) — the safety net for entries removed before every machine uninstalled.

## Plan classification

The planner classifies each requested package into exactly one bucket:

- **enabled** — will run the action
- **disabled** — `enabled: false`, skipped (uninstall ignores this and proceeds anyway)
- **archived** — `archived: true`, skipped with `(archived)` (install / update only; a named uninstall of an archived package still executes). `apply` additionally plans an uninstall for every archived package present in `state.yml`, in reverse dependency order
- **not_installed** — not in `state.yml`, skipped (update / uninstall only)
- **already_installed** — in `state.yml`, skipped (install only)
- **circular** — part of a `depends_on` cycle; skipped with `(circular dependency)` annotation, the rest of the plan continues
- **dependency_disabled** — depends transitively on a disabled package; skipped with `(dependency disabled: <name>)` (install only)
- **script_unmodified** — the action's script still has the `# Generated by homeos` marker; skipped with `(script unmodified: <filename>)`

Plan rendering also surfaces forward / reverse dependency context as annotations:

- `(required by <X>)` on dependencies pulled in by `install` or `apply`
- `(depends on <Y>)` on dependents pulled in by `uninstall`

## Output contracts

Every CLI command honours a global `--output` flag (`--json` is a shorthand for `--output json`) and the `HOMEOS_OUTPUT_FORMAT` env var. Priority: explicit flag > env var > `text` default.

### Text mode

Human-readable output to stdout; errors as `Error: <message>` on stderr.

### JSON mode

- **List commands** (`package list`, `plugin list`, `plugin list-remote`) emit a JSON array of objects matching the text-mode column set
- **Info commands** (`package info`, `plugin info`) emit a single JSON object
- **Plan commands** (`apply`, `package install/update/uninstall` with `--dry-run` or after confirmation) emit a plan object first, then one NDJSON line per package execution result with `status: "success" | "failed"`
- The plan object captures install / update / skipped sections, per-package annotations (see Plan classification), and an `is_empty` flag

### Stderr in JSON mode

Errors still print `Error: <message>` text on stderr — the same wording text mode uses. The contract is: **stdout differs between modes, stderr is identical across modes**. This keeps human-facing wording consistent regardless of which mode the user invoked.

## Error envelope

JSON mode emits a single-object error envelope on stdout AND the same `Error: ...` text on stderr:

```json
{"error":{"reason":"package-not-found","message":"Package 'neovim' not found"}}
```

Process exit code is non-zero. Use `reason` for control flow, `message` for user-facing display.

Canonical `reason` identifiers (kebab-case):

| Reason | Meaning |
|---|---|
| `package-not-found` | A named package is missing from `homeos.yml` |
| `plugin-not-found` | A named plugin is missing from `homeos.yml` |
| `already-exists` | An entry or directory exists where one is being created (also "Already initialized") |
| `validation-error` | Argument / input validation failed (name pattern, URL scheme, missing plugin params, malformed `key=value`) |
| `circular-dependency` | A dependency edit would introduce or expose a cycle |
| `dependency-not-found` | A `--depends-on` / `add-dep` target is not a package in `homeos.yml` |
| `dependent-exists` | A `package remove` or `package archive` target is depended on by other packages |
| `script-failed` | An action script exited non-zero |
| `script-not-found` | An action script file is missing at execute time |
| `script-unmodified` | An action script still contains the `# Generated by homeos` marker |
| `git-not-found` | The `git` binary could not be spawned (`NotFound`) — git is not installed or not on `PATH` |
| `git-clone-failed` | `git clone` returned non-zero |
| `not-a-valid-homeos-repo` | A cloned repository has no `homeos.yml` |
| `not-a-valid-homeos-plugin` | A cloned plugin has no `plugin.yml` |
| `not-initialized` | `homeos.yml` is missing — `homeos init` hasn't been run |
| `data-dir-not-empty` | `homeos init` target directory contains stray files |
| `data-dir-not-found` | `homeos cd` invoked before `homeos init` |
| `directory-not-found` | A package / plugin subdirectory does not exist on disk |
| `not-found-on-github` | A plugin name does not resolve to a `hainet50b/homeos-plugin-<name>` GitHub repo |
| `network-error` | A network request to GitHub failed |
| `package-installed` | A `package remove` target is currently recorded in `state.yml` |
| `internal-error` | Unclassified fallback (typically I/O failures) |

## Input validation

### Package / plugin / dependency names

Allowed pattern: `^[a-z0-9][a-z0-9._-]*$`

- Lowercase ASCII letters, digits, dot, underscore, hyphen
- Must start with a letter or digit (no leading dash, dot, or underscore)
- No path separators (`/`, `\`), whitespace, NUL byte, or control characters
- The substring `..` is rejected anywhere

### URL inputs for `homeos init <url>` and `homeos plugin add <url>`

Two forms are accepted:

- **Scheme-full**: `scheme://…` where the scheme is one of `http`, `https`, `git`, `ssh`, `git+ssh`.
- **SCP-like**: `user@host:path` — the form GitHub's SSH clone button produces (`git@github.com:owner/repo.git`). Split at the first `@` and the first `:` after it; all three components must be non-empty and match:
  - `user`: `[A-Za-z0-9._-]+`, no leading `-`
  - `host`: `[A-Za-z0-9._-]+`, no leading `-` — a character alphabet, not a host allowlist: any DNS name, IPv4 address, or `~/.ssh/config` alias passes, so GitHub Enterprise and self-hosted forges are unaffected. IPv6 literals are not representable in this form; use `ssh://`.
  - `path`: `[A-Za-z0-9._/~-]+`

Rejected in both forms: query strings (`?`), percent-encoded NUL (`%00`), percent-encoded `..` (`%2e%2e`, any case), and control characters. Bare filesystem paths remain rejected (no scheme, no `@`).

The SCP-like form is charset-validated rather than merely dash-checked because of git's remote-helper syntax: without `://`, git parses any input containing `::` as `<helper>::<address>` (e.g. `ext::`, which executes arbitrary local commands), so a leading-dash check alone would let `ext::…` through. The component charsets exclude `:` entirely, making the separator the only colon in the string and helper syntax structurally unreachable. The no-leading-`-` rules keep the argument from parsing as a git/ssh option (defense in depth — git 2.28+ carries its own protections here since CVE-2017-1000117).

Error messages: an input matching neither form → `URL '{url}' must be 'scheme://...' (allowed: http, https, git, ssh, git+ssh) or SCP-like 'user@host:path'`. An SCP-like attempt (no `://`, but an `@` followed by a later `:`) with an invalid component → `URL '{url}' has an invalid {user|host|path} in SCP-like form 'user@host:path'`.

## Environment variables

| Variable | Effect |
|---|---|
| `HOMEOS_DATA_DIR` | Overrides the default data directory (used verbatim, no `homeos/` segment appended) |
| `HOMEOS_OUTPUT_FORMAT` | `text` (default) or `json` — same effect as `--output` |
| `HOMEOS_SKIP_UPDATE_CHECK` | Any non-empty value skips the update check (`homeos cd`, `homeos agents-md`) entirely (no cache read, no network, no file write) |
| `HOMEOS_FORCE_INSTALL` | Any non-empty value bypasses `install.sh` / `install.ps1`'s version-check short-circuit |

## Update check (`homeos cd`, `homeos agents-md`)

`homeos cd` performs a best-effort update check as part of its entry sequence. `homeos agents-md` performs the same check after writing the guide to stdout. Common rules:

- **Cache**: `<data_dir>/.last-update-check`, JSON with `last_checked_at` (Unix epoch seconds) and `latest_tag` (e.g., `"v0.3.12"`). Seeded by `homeos init` with the current binary's tag, no network call.
- **TTL 7 days**: a fresh cache is reused without a network call; a stale or missing cache triggers a fetch of the latest release tag from the GitHub API (1500 ms timeout). `last_checked_at` always advances after a check attempt regardless of network outcome, bounding the cost for offline machines to one timeout per window.
- **Notify condition**: one stderr line `homeos: <latest> available — update at https://github.com/hainet50b/homeos` is emitted only when `latest_tag` is **strictly newer** than the current binary's tag, comparing `vX.Y.Z` numerically (major, minor, patch). If either tag does not parse as `vX.Y.Z`, nothing is emitted — the check is best-effort and silence beats false alarms. Equality and older-than-current (a stale cache right after a binary update) are silent.
- **Self-healing**: when the current binary's tag is strictly newer than the cached `latest_tag`, the cache's `latest_tag` is rewritten to the current tag (preserving `last_checked_at` — no check happened). The running binary is itself proof that a release at least that new exists.
- **Ordering**: the cd entry sequence refreshes AGENTS.md first and emits the update notice second, so the local state change (`homeos: refreshed AGENTS.md to vX.Y.Z`) precedes the outside-world advisory (`homeos: vX.Y.Z available — ...`) and the actionable line lands closest to the prompt.
- **Opt-out**: `HOMEOS_SKIP_UPDATE_CHECK` (any non-empty value) skips cache read, network call, and file write entirely.

## Git invocation conventions

All `git` invocations from inside homeos prefix `-c core.autocrlf=false -c core.eol=lf` so the data directory and every plugin checkout are byte-faithful regardless of the user's global git config. Git for Windows defaults to `core.autocrlf=true`, and `* text` in an upstream `.gitattributes` routes line endings through `core.eol` (default `native` = CRLF on Windows); both overrides are needed to keep `homeos plugin refresh` from reporting spurious modifications and to keep rendered shell scripts LF-clean.

`homeos init` scaffold mode uses `--initial-branch=main` so the branch name is deterministic across machines regardless of the user's `init.defaultBranch`.

## Cross-platform notes

- **Windows shell selection**: prefer `pwsh` (PowerShell 7+); fall back to `powershell.exe` (Windows PowerShell 5.1) when `pwsh` is absent. This rule applies to both shell-spawning contexts: action-script execution and the interactive subshell launched by the cd family (`homeos cd`, `homeos package cd`, `homeos plugin cd`). For subshells, the `SHELL` environment variable, when set, takes priority over the detection. For script execution the fallback is announced in the plan display (`(running under Windows PowerShell 5.1; PowerShell 7 recommended)`) so the user sees the constraint before script execution; the subshell launch stays silent because Windows PowerShell prints its own banner on startup.
- **PowerShell template authoring**: `.ps1.tmpl` files default to the PowerShell 5.1 subset (no `?.`, no ternary `? :`, no `??`, no `ForEach-Object -Parallel`) so the fallback shell can execute them. Plugins that intentionally require 7+ should call that out in their `plugin.yml` `description`.
- **PowerShell profile isolation**: action scripts on Windows are executed with `-NoProfile -File <script>`, so script behaviour cannot depend on the user's `$PROFILE` (hidden machine-local state) and profile startup cost is not paid per script. This mirrors Unix, where non-interactive `sh` reads no profile files. `-File` is explicit because the shells disagree on the default first-argument interpretation (`pwsh` assumes `-File`, `powershell.exe` assumes `-Command`). `-NonInteractive` is NOT passed — action scripts may prompt; stdin/stdout/stderr are inherited by design. The interactive subshell launched by the cd family is the user's own session and loads the profile as normal.
- **Windows binary linkage**: builds for the `*-pc-windows-msvc` targets statically link the MSVC C runtime (`-C target-feature=+crt-static` via `.cargo/config.toml`), so `homeos.exe` runs on a fresh Windows machine without the Visual C++ Redistributable (`vcruntime140.dll`). Linux and macOS targets are unaffected. CRT independence is enforced in CI: the Windows jobs of the Build and Release workflows inspect the built `homeos.exe` import table (`dumpbin /dependents`) and fail if any CRT DLL (`VCRUNTIME*.dll`, `MSVCP*.dll`, `api-ms-win-crt-*.dll`) appears; OS-level imports (`KERNEL32.dll` etc.) are expected and allowed.
- **AGENTS.md / CLAUDE.md pairing**: `CLAUDE.md` is a plain one-line file containing `@AGENTS.md` — Claude Code's import syntax, which resolves the path relative to the importing file and loads the adjacent `AGENTS.md` in full. The same file is written on every platform, and because it is a pointer rather than a copy it never goes stale when `AGENTS.md` is regenerated — it only needs creating when missing. Every `homeos cd` ensures the pointer: missing → create; legacy form → replace. The legacy-replacement branch is a time-limited migration shim for pre-v0.3.15 installs, where `CLAUDE.md` was a symlink to `AGENTS.md` on Linux/macOS and a content copy on Windows; the symlink case MUST be `remove_file`d before writing, because writing through the symlink would clobber `AGENTS.md` itself. The shim is marked for removal after 2027-01.
