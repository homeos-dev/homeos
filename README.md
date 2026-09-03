![homeos](assets/banner.png)

![Build](https://github.com/hainet50b/homeos/actions/workflows/build.yml/badge.svg)
![Release](https://img.shields.io/github/v/release/hainet50b/homeos)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

**Making install script hell feel like *home*, with your AI agent.**

*homeos* (ho-mee-os) — named after *homeostasis*: a layer above your install scripts for applications and Agent Skills, held in a single Git repository and kept steady by your AI agent.

> [!IMPORTANT]
> **Built for AI agents** *(since v0.3.0)*. *homeos* ships its own [Agent Skill](https://agentskills.io). It guides your AI agent to manage applications and Agent Skills on your instruction, and takes care of the documentation and Git operations along the way. See [Using with AI agents](#using-with-ai-agents).

## Features

- **Source of truth in a single Git repo** — Everything in your *home* is visible and under your control. Nothing happens beyond what you write.
- **One interface, any provider** — Manage custom scripts and package providers alike through one interface and one config file.
- **Install in the right order** — When packages depend on each other, homeos respects dependencies in every operation, executing scripts in the correct order.
- **Nothing runs without confirmation** — A plan is always shown before execution. If it's not in the plan, it doesn't run.
- **Run anywhere: Linux, macOS, Windows** — Built with Rust. Works on any OS.

[Quick Tour](#quick-tour) | [Install](#install) | [Using with AI agents](#using-with-ai-agents) | [Official Plugins](#official-plugins) | [Plugin Development Guide](#plugin-development-guide) | [Reference](#reference)

## Quick Tour

### With an AI agent — the new standard

With the CLI and the Agent Skills [installed](#install), create your home:

```sh
$ homeos init
Initialized homeos at /home/<username>/.local/share/homeos
```

Then tell your agent what you want:

> Install Neovim for me

The agent picks the right plugin for your OS, shows you a plan, installs after your approval, updates the repository README, and commits the change. See [Using with AI agents](#using-with-ai-agents) for what happens under the hood.

### Manual walk-through

If you prefer to drive homeos directly, here is the same flow without an agent.

1. Initialize a new repository

```sh
$ homeos init
Initialized homeos at /home/<username>/.local/share/homeos
```

2. Add a package

```sh
$ homeos package add rustup
Added package 'rustup'
```

3. Move to a package directory and edit its install scripts

```sh
# Open a new shell in the package directory
$ homeos package cd rustup
$ ls
install.ps1  install.sh  uninstall.ps1  uninstall.sh  update.ps1  update.sh

# Edit the scripts you need, remove the ones you don't

# Return to the previous shell
$ exit
```

4. Verify the scripts

```sh
$ homeos package cat rustup
=== install.sh ===
#!/usr/bin/env sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

=== install.ps1 ===
(not found)

=== update.sh ===
#!/usr/bin/env sh
rustup update

=== update.ps1 ===
(not found)

=== uninstall.sh ===
#!/usr/bin/env sh
rustup self uninstall

=== uninstall.ps1 ===
(not found)
```

5. Apply, review the plan and install

```sh
$ homeos apply
The following packages will be installed:
  rustup

Proceed? [y/N] y
Installing rustup...
(rustup installer output)
done

$ rustup --version
rustup 1.29.0 (28d1352db 2026-03-05)
```

<details>
<summary>Using a plugin</summary>

1. Browse official plugins

```sh
$ homeos plugin list-remote
Name      Description                                  URL
--------  -------------------------------------------  ---------------------------------------------------
apt       APT package manager plugin for homeos.       https://github.com/hainet50b/homeos-plugin-apt
dnf       DNF package manager plugin for homeos.       https://github.com/hainet50b/homeos-plugin-dnf
homebrew  Homebrew package manager plugin for homeos.  https://github.com/hainet50b/homeos-plugin-homebrew
winget    WinGet package manager plugin for homeos.    https://github.com/hainet50b/homeos-plugin-winget
```

2. Add a plugin to your repository

```sh
$ homeos plugin add dnf
Plugin 'dnf' added successfully
```

3. Create a package using the plugin — the plugin's templates render automatically, no script editing needed

```sh
$ homeos package add neovim --plugin dnf --param name=neovim
Added package 'neovim'
```

4. Apply

```sh
$ homeos apply
The following packages will be installed:
  neovim (plugin: dnf)

Proceed? [y/N] y
Installing neovim...
(dnf output)
done
```

</details>

<details>
<summary>Composing packages with a repo (e.g., COPR, tap, bucket)</summary>

Setup steps that several packages share — enabling a COPR repository, adding a Homebrew tap, registering a Scoop bucket — are best expressed as their own package, with dependents declaring `depends_on`. The setup runs once before any dependent and can be uninstalled cleanly.

1. Add the required plugins — both the setup plugin (e.g., `dnf-copr`, `homebrew-tap`, `scoop-bucket`) and the package manager plugin used by the dependent

```sh
$ homeos plugin add dnf-copr
Plugin 'dnf-copr' added successfully
$ homeos plugin add dnf
Plugin 'dnf' added successfully
```

2. Add a setup package using the setup plugin

```sh
$ homeos package add dnf-copr-mise --plugin dnf-copr --param name=jdxcode/mise
Added package 'dnf-copr-mise'
```

3. Add the actual package depending on it

```sh
$ homeos package add mise --plugin dnf --param name=mise --depends-on dnf-copr-mise
Added package 'mise'
```

4. Apply — the COPR enable runs first, then the package install

```sh
$ homeos apply
The following packages will be installed:
  dnf-copr-mise (required by mise)
  mise (plugin: dnf)

Proceed? [y/N]
```

</details>

<details>
<summary>Setting up a new machine from your repo</summary>

After installing homeos (see [Install](#install)), restore your environment from your existing homeos repository in two commands.

1. Clone your repository

```sh
$ homeos init https://github.com/<username>/<repo>
Initialized homeos at /home/<username>/.local/share/homeos (cloned from https://github.com/<username>/<repo>)
```

2. Apply — installs everything declared in `homeos.yml`

```sh
$ homeos apply
The following packages will be installed:
  ...

Proceed? [y/N] y
Installing ...
done
```

</details>

## Install

### Prerequisites

- **Git 2.28 or newer.** `homeos init` runs `git init --initial-branch=main`, which requires Git 2.28+.

### CLI

#### Linux / macOS

```sh
curl -sSf https://raw.githubusercontent.com/hainet50b/homeos/main/install.sh | sh
```

Installs to `~/.local/bin/homeos`. Ensure `~/.local/bin` is in your `PATH`.

#### Windows

```powershell
irm https://raw.githubusercontent.com/hainet50b/homeos/main/install.ps1 | iex
```

Installs to `%USERPROFILE%\.homeos\bin\homeos.exe` and adds the directory to your user `PATH`.

The install scripts also set up shell completion for your detected shell. If your shell needs an additional manual step, the script prints the exact instruction to follow.

Alternatively, download the prebuilt binary directly from [GitHub Releases](https://github.com/hainet50b/homeos/releases) and place it on your `PATH` manually. In that case, see [Shell completion](#shell-completion) to set up completion yourself.

### Agent Skill

homeos ships its own [Agent Skills](https://agentskills.io). They live under [`skills/`](skills/) in this repository:

| Skill | Purpose |
|---|---|
| [`homeos-manage`](skills/homeos-manage/SKILL.md) | Lets your AI agent manage applications through homeos |
| [`homeos-inventory`](skills/homeos-inventory/SKILL.md) | Tells your AI agent which applications are available to it |

Place them in any directory your agent reads skills from, or install them with the GitHub CLI:

```sh
gh skill install hainet50b/homeos --all --scope user --agent universal
```

Agents that read their own skills directory take a different `--agent` value: for Claude Code, pass `--agent claude-code`. See `gh skill install --help` for the full list.

Once the skills are in place, ask your agent to bring them under homeos management. It will turn them into homeos packages, even though they are already installed, so that homeos can update and uninstall them from now on and reproduce them on your other machines along with everything else.

## Using with AI agents

Once the [Agent Skills](#agent-skill) are installed, the `homeos-manage` skill teaches your AI agent how to operate homeos whenever something is to be installed, updated, or uninstalled on your machine, and the `homeos-inventory` skill tells it which applications are available to it while working on anything else.

### What happens under the hood

For example, asking *"Install Neovim for me"* on a Fedora machine:

1. The agent locates the data directory with `homeos cd --print --json`, then runs `homeos package list --json` and `homeos plugin list --json` to see what is currently declared, and `homeos plugin list-remote --json` to find the right package manager plugin if none is in place yet (e.g. `dnf` on Fedora, `homebrew` on macOS, `winget` on Windows).
2. It presents its proposal: install `neovim` through the `dnf` plugin, with the package's provenance checked with `dnf info` — so you can tell it is the official Fedora build and not a namesake — and the install script that would run (here `dnf -y install neovim`).
3. Once you approve the proposal, it declares the plugin and the package with `homeos plugin add` / `homeos package add`, then shows you the execution plan with `homeos package install neovim --dry-run --json`.
4. Once you approve that plan, it installs with `homeos --yes --json package install neovim`.
5. It updates the repository `README.md` to reflect what's now installed, then commits everything together with `git -C <data_dir> add -A && git -C <data_dir> commit -m "Add Neovim"`.

The plan and confirmation steps mean the agent cannot run anything you haven't seen. The `--json` output mode and the `--yes` flag let agents read structured state and execute approved plans on your behalf. When a newer homeos release exists, the agent tells you and asks whether to update before continuing.

> [!NOTE]
> Action scripts under `packages/` work best when **non-interactive**: pass a flag such as `-y` so the underlying tool doesn't prompt. The agent's subprocess has no controlling terminal, and a script that tries to read from stdin will fail immediately.
>
> `sudo` is the one operation that can't be made non-interactive — it always needs your password. When a script in the plan invokes `sudo`, the agent hands off: it asks you to run the equivalent `homeos package install <name>` in your own terminal, where you have a real tty and can type the password.

## Official Plugins

Official plugins are available. See each plugin's repository for details.

| Name | Description |
|------|-------------|
| [apt](https://github.com/hainet50b/homeos-plugin-apt) | APT package manager plugin for homeos. |
| [dnf](https://github.com/hainet50b/homeos-plugin-dnf) | DNF package manager plugin for homeos. |
| [dnf-copr](https://github.com/hainet50b/homeos-plugin-dnf-copr) | DNF COPR plugin for homeos. |
| [gh-skill](https://github.com/hainet50b/homeos-plugin-gh-skill) | Agent skill plugin for homeos, backed by GitHub CLI (gh skill, preview). |
| [homebrew](https://github.com/hainet50b/homeos-plugin-homebrew) | Homebrew package manager plugin for homeos. |
| [homebrew-cask](https://github.com/hainet50b/homeos-plugin-homebrew-cask) | Homebrew cask plugin for homeos. |
| [homebrew-tap](https://github.com/hainet50b/homeos-plugin-homebrew-tap) | Homebrew tap plugin for homeos. |
| [npm](https://github.com/hainet50b/homeos-plugin-npm) | npm package manager plugin for homeos. |
| [scoop](https://github.com/hainet50b/homeos-plugin-scoop) | Scoop package manager plugin for homeos. |
| [scoop-bucket](https://github.com/hainet50b/homeos-plugin-scoop-bucket) | Scoop bucket plugin for homeos. |
| [winget](https://github.com/hainet50b/homeos-plugin-winget) | WinGet package manager plugin for homeos. |

Built a community plugin? [Open an issue](https://github.com/hainet50b/homeos/issues/new) and we'll list it here.  
Want a plugin that doesn't exist yet? [Request it](https://github.com/hainet50b/homeos/issues/new) — we'd love to hear what you need.

## Plugin Development Guide

A plugin consists of a parameter list (`plugin.yml`) and template files (`*.tmpl`). When a user runs `homeos package add` with a plugin, the templates are rendered with the user's parameters.

Below is a step-by-step guide to creating a plugin.

### 1. Create a plugin

```sh
$ homeos plugin add --local my-provider
Plugin 'my-provider' created locally

$ homeos plugin cd my-provider
$ ls
install.ps1.tmpl  plugin.yml          uninstall.sh.tmpl  update.sh.tmpl
install.sh.tmpl   uninstall.ps1.tmpl  update.ps1.tmpl
```

### 2. Define parameters

Edit `plugin.yml` to set a `description` (a one-line summary of the plugin's purpose) and define the parameters that users must provide when using your plugin:

```yaml
description: My provider plugin for homeos.
params:
  - name
```

### 3. Write templates

Edit the template files for each action. `.sh.tmpl` files are for Linux/macOS, `.ps1.tmpl` files are for Windows. You can provide both, or only the ones relevant to your plugin's purpose — unused templates can be deleted.

Templates use `{{key}}` placeholders. When a user runs `homeos package add` with your plugin, each `{{key}}` is replaced with the corresponding value from the user's `params`.

For example, `install.sh.tmpl`:

```sh
#!/usr/bin/env sh
sudo dnf install -y {{name}}
```

When a user adds a package with `--param name=neovim.x86_64`:

```sh
$ homeos package add neovim --plugin my-provider --param name=neovim.x86_64

$ homeos package cat neovim
=== install.sh ===
#!/usr/bin/env sh
sudo dnf install -y neovim.x86_64
```

> [!TIP]
> Downloaded plugins can be freely edited locally. Changes only affect your machine and are not pushed upstream.

## Reference

### Directory Structure

The data directory depends on the operating system:

| OS      | Data directory                         |
|---------|----------------------------------------|
| Linux   | `~/.local/share/homeos`                |
| macOS   | `~/Library/Application Support/homeos` |
| Windows | `%LOCALAPPDATA%/homeos`                |

```
<data_dir>/
├── homeos.yml
├── state.yml
├── .gitignore
├── packages/
│   └── neovim/
│       ├── install.sh
│       ├── update.sh
│       └── uninstall.sh
└── plugins/
    └── dnf/
```

- `homeos.yml` — package and plugin definitions.
- `state.yml` — tracks installed packages. Machine-specific, excluded from version control via `.gitignore`.
- `packages/` — action scripts (`install.sh`, `update.sh`, `uninstall.sh` for Linux/macOS; `.ps1` for Windows).
- `plugins/` — plugin files.

### Overriding the data directory

Set `HOMEOS_DATA_DIR` to use an alternate data directory:

```sh
export HOMEOS_DATA_DIR="$HOME/.config/homeos-work"
homeos apply
```

Priority: `HOMEOS_DATA_DIR` overrides the OS default. If unset, the default from the table above applies.

#### Maintaining multiple configurations

You can keep more than one homeos repository on the same machine and switch between them with `HOMEOS_DATA_DIR`. A common case: your personal homeos repo lives at the OS default location, and a separate repository holds **your team's shared setup** (onboarding scripts, standardized developer tools, internal package definitions). Maintain and apply each independently:

```sh
# Your personal repo — the OS default, no override needed:
homeos apply

# Switch to your team's shared repo:
export HOMEOS_DATA_DIR="$HOME/.config/homeos-team"
homeos init https://github.com/<team>/<homeos-repo>
homeos apply
```

Each repository is fully isolated — its own `homeos.yml`, packages, plugins, and installation state.

### Configuration (homeos.yml)

```yaml
packages:
  claude:
    depends_on: [bubblewrap, sandbox-runtime, socat]
    script_aliases: { update: install }
  bubblewrap:
    plugin: dnf
    params:
      name: bubblewrap
  sandbox-runtime:
    plugin: npm
    params:
      name: "@anthropic-ai/claude-code-sandbox-runtime"
  socat:
    plugin: dnf
    params:
      name: socat
  ollama:
    script_aliases: { update: install }
    enabled: false

plugins:
  dnf:
    url: https://github.com/hainet50b/homeos-plugin-dnf
  npm:
    url: https://github.com/hainet50b/homeos-plugin-npm
```

- `depends_on` — declare dependencies on other packages.
- `script_aliases` — map an action to another script (e.g., run `install.sh` for `update`).
- `enabled` — `true` by default. Set to `false` to skip the package in operations.
- `archived` — `false` by default. `true` marks a package as removed from every machine: `apply` uninstalls it wherever it is still installed, and `install` / `update` skip it. Set via `homeos package archive`.
- `plugin` — plugin name to use for script generation.
- `params` — values passed to the plugin's templates.

### Core Commands

#### `homeos init`

Create the initial homeos structure at the data directory. Without arguments, creates an empty structure with a skeleton `homeos.yml`. With a URL, clones the remote repository into the data directory.

```
Usage: homeos init [OPTIONS] [URL]

Arguments:
  [URL]  Remote URL to clone

Options:
      --strip-git  Remove .git directory after cloning
```

#### `homeos cd`

Launch a shell in the data directory.

```
Usage: homeos cd [OPTIONS]

Options:
      --print  Print the data directory path to stdout and exit, without launching a shell
```

#### `homeos apply`

Install new packages, update installed ones, and uninstall archived ones.

```
Usage: homeos apply [OPTIONS]

Options:
      --dry-run  Display the plan without executing scripts or prompting
```

A confirmation prompt is shown before execution.

```
$ homeos apply
The following packages will be installed:
  bubblewrap
  socat
  claude
The following packages will be updated:
  neovim
The following packages will be uninstalled:
  ollama
The following packages will be skipped:
  zed (disabled)

Proceed? [y/N]
```

> [!NOTE]
> Packages are installed in dependency order based on `depends_on`; uninstalls run in reverse dependency order.

`apply` is how an uninstall performed on one machine reaches the others: archiving a package (see `homeos package archive`) marks it in `homeos.yml`, and each machine's next `apply` uninstalls it there. Entries that remain in `state.yml` without a package definition (e.g., removed before every machine caught up) are reported at the end of the plan; homeos cannot act on them because their scripts are gone.

### Manage packages

Packages can be **enabled**, **disabled**, or **archived**. Disabled packages are frozen: skipped by `apply`, `install`, and `update`, but kept installed where they are. Archived packages should not be installed anywhere: `install` and `update` skip them, and `apply` uninstalls them wherever they are still installed. `uninstall` runs regardless of the status. Newly added packages are enabled by default.

Packages can declare **dependencies** on other packages. homeos handles them in the right order.

#### `homeos package list`

List all packages.

```
Usage: homeos package list
```

Also available as `homeos package ls`.

Displays a table with package name, status (`enabled` / `disabled` / `archived`), installed status, backing plugin, and dependencies.

```
$ homeos package list
Package  Status    Installed  Plugin  Dependencies
-------  --------  ---------  ------  -----------------
neovim   enabled   yes        dnf     -
claude   enabled   yes        -       bubblewrap, socat
docker   disabled  no         dnf     -
ollama   archived  yes        -       -
```

#### `homeos package add`

Add a new package. Creates a package directory under `packages/` and adds an entry to `homeos.yml`. Skeleton scripts are generated for all OS (both `.sh` and `.ps1`).

```
Usage: homeos package add [OPTIONS] <PACKAGE>

Arguments:
  <PACKAGE>  Package name

Options:
      --depends-on <DEPENDENCY>    Add a dependency (can be repeated)
      --script-alias <ALIAS>       Add a script alias as target=source (can be repeated)
      --plugin <PLUGIN>            Plugin to use for generating scripts
      --param <PARAM>              Plugin parameter as key=value (can be repeated)
```

To generate scripts from a plugin instead of default skeletons:

```
$ homeos package add neovim --plugin dnf --param name=neovim.x86_64
```

#### `homeos package remove`

Remove package entries from `homeos.yml`. The package directory is not deleted unless `--purge` is specified.

```
Usage: homeos package remove [OPTIONS] <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names

Options:
      --purge  Also delete the package directory
```

#### `homeos package rename`

Rename a package. Renames the package directory on disk, updates the entry key in `homeos.yml`, updates `state.yml` if the package is installed, and updates any `depends_on` references in other packages to point to the new name.

```
Usage: homeos package rename <OLD> <NEW>

Arguments:
  <OLD>  Current package name
  <NEW>  New package name
```

#### `homeos package add-dep`

Add dependencies to an existing package.

```
Usage: homeos package add-dep <PACKAGE> <DEPENDENCY>...

Arguments:
  <PACKAGE>        Package name
  <DEPENDENCY>...  Dependencies to add
```

#### `homeos package remove-dep`

Remove dependencies from an existing package.

```
Usage: homeos package remove-dep <PACKAGE> <DEPENDENCY>...

Arguments:
  <PACKAGE>        Package name
  <DEPENDENCY>...  Dependencies to remove
```

#### `homeos package add-alias`

Add script aliases to an existing package.

```
Usage: homeos package add-alias <PACKAGE> <ALIAS>...

Arguments:
  <PACKAGE>    Package name
  <ALIAS>...   Aliases as target=source pairs (e.g., update=install)
```

#### `homeos package remove-alias`

Remove script aliases from a package.

```
Usage: homeos package remove-alias <PACKAGE> <ALIAS>...

Arguments:
  <PACKAGE>    Package name
  <ALIAS>...   Alias targets to remove (e.g., update)
```

#### `homeos package enable`

Enable packages.

```
Usage: homeos package enable <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package disable`

Disable packages.

```
Usage: homeos package disable <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package archive`

Archive packages: mark them as removed from every machine. Sets `archived: true` in `homeos.yml`; the entry and the package directory stay in the repository as a tombstone, so the uninstall scripts reach every machine. On each machine, the next `homeos apply` uninstalls archived packages that are still installed there — including the machine that ran `archive`.

```
Usage: homeos package archive <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

Archiving is refused while non-archived packages depend on the target.

The removal flow across machines: `archive` → `apply` on each machine → optionally `package remove` once no machine has it installed.

#### `homeos package unarchive`

Unarchive packages. The package returns with the enabled/disabled status it had before archiving.

```
Usage: homeos package unarchive <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names
```

#### `homeos package info`

Display package details.

```
Usage: homeos package info <PACKAGE>

Arguments:
  <PACKAGE>  Package name
```

Shows enabled/archived/installed status, plugin, dependencies, dependents, and script aliases.

```
$ homeos package info claude
Package: claude
Enabled: yes
Archived: no
Installed: yes
Plugin: -
Dependencies:
  bubblewrap
  sandbox-runtime
  socat
Dependents:
  (none)
Script aliases:
  update → install
```

#### `homeos package cat`

Display all scripts for a package.

```
Usage: homeos package cat <PACKAGE>

Arguments:
  <PACKAGE>  Package name
```

Displays all script files for both Linux/macOS (`.sh`) and Windows (`.ps1`) regardless of the current OS. If a script does not exist, `(not found)` is shown.

```
$ homeos package cat rustup
=== install.sh ===
#!/usr/bin/env sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

=== install.ps1 ===
(not found)

=== update.sh ===
#!/usr/bin/env sh
rustup update

=== update.ps1 ===
(not found)

=== uninstall.sh ===
(not found)

=== uninstall.ps1 ===
(not found)
```

#### `homeos package cd`

Launch a shell in the package root or specific package directory.

```
Usage: homeos package cd [OPTIONS] [PACKAGE]

Arguments:
  [PACKAGE]  Package name (optional — defaults to packages root)

Options:
      --print  Print the resolved path to stdout and exit, without launching a shell
```

### Operate packages

homeos tracks which packages are installed on this machine in `state.yml`. This file is machine-specific and managed automatically — you don't need to edit it. The behavior of `install`, `update`, and `uninstall` depends on the package's enabled status and whether it is in `state.yml`.

A confirmation prompt is shown before execution. On script failure, the error is reported and execution continues to the next package.

| State                       | install                  | update                  | uninstall               |
|-----------------------------|--------------------------|-------------------------|-------------------------|
| enabled + not in state      | Execute                  | Skip (not installed)    | Skip (not installed)    |
| enabled + in state          | Skip (already installed) | Execute                 | Execute                 |
| disabled + not in state     | Skip (disabled)          | Skip (disabled)         | Skip (not installed)    |
| disabled + in state         | Skip (disabled)          | Skip (disabled)         | Execute                 |
| archived + not in state     | Skip (archived)          | Skip (archived)         | Skip (not installed)    |
| archived + in state         | Skip (archived)          | Skip (archived)         | Execute                 |

> [!NOTE]
> `uninstall` ignores the enabled/disabled/archived status — its only concern is whether the package is in `state.yml`. These commands never modify `homeos.yml`: uninstalling a package that is not archived leaves it declared as before, so the next `apply` can reinstall it. homeos prints a hint pointing at `homeos package archive` in that case.

#### `homeos package install`

Execute install scripts. Records installed packages in `state.yml`.

```
Usage: homeos package install [OPTIONS] <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names

Options:
      --dry-run  Display the plan without executing scripts or prompting
```

#### `homeos package update`

Execute update scripts.

```
Usage: homeos package update [OPTIONS] <PACKAGES>...

Arguments:
  <PACKAGES>...  Package names

Options:
      --dry-run  Display the plan without executing scripts or prompting
```

#### `homeos package uninstall`

Execute uninstall scripts and remove the packages from `state.yml`. `homeos.yml` is not modified and the package directory is not deleted. To remove a package from every machine, use `homeos package archive` instead — `uninstall` only affects this machine.

```
Usage: homeos package uninstall [OPTIONS] [PACKAGES]...

Arguments:
  [PACKAGES]...  Package names

Options:
      --all      Uninstall all installed packages (from state.yml)
      --dry-run  Display the plan without executing scripts or prompting
```

### Manage plugins

#### `homeos plugin list`

List all plugins.

```
Usage: homeos plugin list
```

Also available as `homeos plugin ls`.

Displays a table with plugin name, description, and URL. `Description` is loaded from each plugin's `plugin.yml`.

```
$ homeos plugin list
Name  Description                              URL
----  ---------------------------------------  ----------------------------------------------
dnf   DNF package manager plugin for homeos.   https://github.com/hainet50b/homeos-plugin-dnf
```

#### `homeos plugin list-remote`

List official plugins available from GitHub.

```
Usage: homeos plugin list-remote
```

Also available as `homeos plugin ls-remote`.

Displays name, description, and URL for each official plugin.

```
$ homeos plugin list-remote
Name      Description                                  URL
--------  -------------------------------------------  ---------------------------------------------------
apt       APT package manager plugin for homeos.       https://github.com/hainet50b/homeos-plugin-apt
dnf       DNF package manager plugin for homeos.       https://github.com/hainet50b/homeos-plugin-dnf
homebrew  Homebrew package manager plugin for homeos.  https://github.com/hainet50b/homeos-plugin-homebrew
winget    WinGet package manager plugin for homeos.    https://github.com/hainet50b/homeos-plugin-winget
```

#### `homeos plugin add`

Add a plugin. Without a URL, resolves the official repository automatically. See [Plugin Development Guide](#plugin-development-guide) for `--local` usage.

```
Usage: homeos plugin add [OPTIONS] <PLUGIN> [URL]

Arguments:
  <PLUGIN>  Plugin name
  [URL]   Remote URL to clone (defaults to official repository)

Options:
      --local  Create an empty plugin skeleton for local development
```

#### `homeos plugin remove`

Remove a plugin. The plugin directory is not deleted unless `--purge` is specified.

```
Usage: homeos plugin remove [OPTIONS] <PLUGIN>

Arguments:
  <PLUGIN>  Plugin name

Options:
      --purge  Also delete the plugin directory
```

#### `homeos plugin refresh`

Refetch a plugin's templates from its registered URL. Useful when the upstream plugin has been updated and you want the new templates locally.

```
Usage: homeos plugin refresh [OPTIONS] [PLUGIN]

Arguments:
  [PLUGIN]  Plugin name (required unless --all)

Options:
      --all      Refresh every registered plugin
      --dry-run  Show what would change without writing
      --yes      Skip the confirmation prompt
```

The diff is computed file by file (added / modified / removed). Local edits to a plugin's templates are overwritten by the refresh — review the summary before confirming. Plugins added via `plugin add --local` are skipped (they have no upstream URL).

#### `homeos plugin info`

Display plugin details.

```
Usage: homeos plugin info <PLUGIN>

Arguments:
  <PLUGIN>  Plugin name
```

Shows description, URL (or `(local)`), parameters, and action templates.

```
$ homeos plugin info dnf
Plugin: dnf
Description: DNF package manager plugin for homeos.
URL: https://github.com/hainet50b/homeos-plugin-dnf
Parameters:
  name
Templates:
  install.sh.tmpl (/home/<username>/.local/share/homeos/plugins/dnf/install.sh.tmpl)
  install.ps1.tmpl (not found)
  update.sh.tmpl (/home/<username>/.local/share/homeos/plugins/dnf/update.sh.tmpl)
  update.ps1.tmpl (not found)
  uninstall.sh.tmpl (/home/<username>/.local/share/homeos/plugins/dnf/uninstall.sh.tmpl)
  uninstall.ps1.tmpl (not found)
```

#### `homeos plugin cat`

Display `plugin.yml` and all template files for a plugin.

```
Usage: homeos plugin cat <PLUGIN>

Arguments:
  <PLUGIN>  Plugin name
```

Example:

```
$ homeos plugin cat dnf
=== plugin.yml ===
params:
  - name

=== install.sh.tmpl ===
#!/usr/bin/env sh
sudo dnf install -y {{name}}

=== update.sh.tmpl ===
#!/usr/bin/env sh
sudo dnf install -y {{name}}

=== uninstall.sh.tmpl ===
#!/usr/bin/env sh
sudo dnf remove -y {{name}}
```

#### `homeos plugin cd`

Launch a shell in the plugins root or specific plugin directory.

```
Usage: homeos plugin cd [OPTIONS] [PLUGIN]

Arguments:
  [PLUGIN]  Plugin name (optional — defaults to plugins root)

Options:
      --print  Print the resolved path to stdout and exit, without launching a shell
```

### Shell completion

Print a shell completion script for the given shell to stdout.

```
Usage: homeos completion <SHELL>

Arguments:
  <SHELL>  Target shell [possible values: bash, zsh, fish, powershell, elvish]
```

Redirect the output to the shell-specific location:

```sh
# bash
homeos completion bash > ~/.local/share/bash-completion/completions/homeos

# zsh
homeos completion zsh > ~/.local/share/zsh/site-functions/_homeos

# fish
homeos completion fish > ~/.config/fish/completions/homeos.fish

# PowerShell
homeos completion powershell >> $PROFILE

# Elvish
homeos completion elvish > ~/.config/elvish/lib/homeos.elv
```

> [!NOTE]
> `install.sh` / `install.ps1` set up completion automatically for the detected shell.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
