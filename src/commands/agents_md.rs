use crate::Cli;
use crate::context::Context;
use clap::{Arg, Command, CommandFactory};
use std::io::Write;
use std::path::Path;

const TEMPLATE: &str = include_str!("../../templates/AGENTS.md.tmpl");

pub fn run(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    run_to(
        ctx.data_dir(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// Write the rendered guide to `out`, then run the best-effort update check
/// against `data_dir` with `notice` as its writer (stderr in production).
///
/// The guide is the whole of stdout — `homeos agents-md > AGENTS.md` must keep
/// producing a byte-identical file — so the notice goes to `notice` only, after
/// the guide is flushed. When the data directory does not exist the check is
/// skipped entirely: `agents-md` is the entry point an agent reads *before*
/// `homeos init`, so it must neither require nor create the data directory. A
/// failing check never fails the command.
fn run_to<O: Write, N: Write>(
    data_dir: &Path,
    out: &mut O,
    notice: &mut N,
) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = render();
    out.write_all(rendered.as_bytes())?;
    out.flush()?;
    if data_dir.exists() {
        let _ = crate::commands::update_check::check_and_notify_to_writer(data_dir, notice);
    }
    Ok(())
}

fn render() -> String {
    let commands_reference = build_commands_reference();
    TEMPLATE
        .replace("\r\n", "\n")
        .replace("{{ commands_reference }}", &commands_reference)
}

fn build_commands_reference() -> String {
    let cmd = Cli::command();
    let mut output = String::new();
    walk_subcommands(&cmd, "homeos", &mut output);
    output.trim_end().to_string()
}

fn walk_subcommands(cmd: &Command, path: &str, output: &mut String) {
    for sub in cmd.get_subcommands() {
        let sub_name = sub.get_name();
        if sub_name == "help" {
            continue;
        }
        let sub_path = format!("{path} {sub_name}");
        let has_nested_leaves = sub.get_subcommands().any(|s| s.get_name() != "help");
        if has_nested_leaves {
            walk_subcommands(sub, &sub_path, output);
        } else {
            emit_leaf_entry(sub, &sub_path, output);
        }
    }
}

fn emit_leaf_entry(cmd: &Command, path: &str, output: &mut String) {
    output.push_str(&format!("### `{path}`\n\n"));
    if let Some(about) = cmd.get_about() {
        output.push_str(&format!("{about}\n\n"));
    }
    let args: Vec<&Arg> = cmd
        .get_arguments()
        .filter(|a| {
            !a.is_hide_set()
                && !a.is_global_set()
                && a.get_id().as_str() != "help"
                && a.get_id().as_str() != "version"
        })
        .collect();
    if !args.is_empty() {
        for arg in args {
            output.push_str(&format_arg_entry(arg));
            output.push('\n');
        }
        output.push('\n');
    }
}

fn format_arg_entry(arg: &Arg) -> String {
    let identifier = if arg.is_positional() {
        format!("<{}>", arg.get_id().as_str().to_uppercase())
    } else if let Some(long) = arg.get_long() {
        format!("--{long}")
    } else if let Some(short) = arg.get_short() {
        format!("-{short}")
    } else {
        arg.get_id().as_str().to_string()
    };
    let help = arg.get_help().map(|h| h.to_string()).unwrap_or_default();
    if help.is_empty() {
        format!("- `{identifier}`")
    } else {
        format!("- `{identifier}` — {help}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::update_check::{UpdateCheckCache, cache_path};
    use crate::env_test::EnvVarGuard;
    use std::collections::BTreeSet;
    use std::fs;

    /// Write a cache file at `data_dir` that is within the TTL (so no network
    /// call happens) and holds `tag` as the latest release.
    fn write_fresh_cache(data_dir: &Path, tag: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cache = UpdateCheckCache {
            last_checked_at: now,
            latest_tag: tag.to_string(),
        };
        fs::write(cache_path(data_dir), serde_json::to_string(&cache).unwrap()).unwrap();
    }

    #[test]
    fn test_render_substitutes_commands_reference_placeholder() {
        // Arrange & Act
        let rendered = render();

        // Assert
        assert!(!rendered.contains("{{ commands_reference }}"));
    }

    #[test]
    fn test_render_commands_reference_includes_top_level_leaves() {
        // Arrange & Act
        let rendered = render();

        // Assert
        assert!(rendered.contains("`homeos init`"));
        assert!(rendered.contains("`homeos cd`"));
        assert!(rendered.contains("`homeos apply`"));
        assert!(rendered.contains("`homeos completion`"));
        assert!(rendered.contains("`homeos agents-md`"));
    }

    #[test]
    fn test_render_commands_reference_includes_nested_leaves() {
        // Arrange & Act
        let rendered = render();

        // Assert
        assert!(rendered.contains("`homeos package list`"));
        assert!(rendered.contains("`homeos package add`"));
        assert!(rendered.contains("`homeos package install`"));
        assert!(rendered.contains("`homeos plugin list`"));
        assert!(rendered.contains("`homeos plugin add`"));
    }

    #[test]
    fn test_build_commands_reference_omits_help_subcommand() {
        // Arrange & Act
        let reference = build_commands_reference();

        // Assert
        assert!(!reference.contains("`homeos help`"));
        assert!(!reference.contains("`homeos package help`"));
    }

    #[test]
    fn test_run_to_writes_rendered_template() {
        // Arrange
        let tmp = tempfile::TempDir::new().unwrap();
        let mut out: Vec<u8> = Vec::new();

        // Act
        run_to(&tmp.path().join("absent"), &mut out, &mut std::io::sink()).unwrap();

        // Assert
        let output = String::from_utf8(out).unwrap();
        assert!(output.starts_with("# AGENTS.md"));
        assert!(output.contains("`homeos init`"));
    }

    #[test]
    fn test_run_to_emits_update_notice_when_cache_holds_newer_tag() {
        // Arrange — an existing data dir with a fresh cache holding a strictly
        // newer tag (fresh cache => no network call), and the skip env var cleared
        let guard = EnvVarGuard::capture("HOMEOS_SKIP_UPDATE_CHECK");
        guard.unset();
        let tmp = tempfile::TempDir::new().unwrap();
        write_fresh_cache(tmp.path(), "v99.0.0");
        let mut out: Vec<u8> = Vec::new();
        let mut notice: Vec<u8> = Vec::new();

        // Act
        run_to(tmp.path(), &mut out, &mut notice).unwrap();

        // Assert — the notice lands on the notice writer, stdout is the guide alone
        let notice_text = String::from_utf8(notice).unwrap();
        assert!(
            notice_text.contains("v99.0.0 available"),
            "expected an update notice, got: {notice_text:?}"
        );
        assert_eq!(String::from_utf8(out).unwrap(), render());
    }

    #[test]
    fn test_run_to_is_silent_when_cache_holds_equal_tag() {
        // Arrange — fresh cache holding exactly the running binary's tag
        let guard = EnvVarGuard::capture("HOMEOS_SKIP_UPDATE_CHECK");
        guard.unset();
        let tmp = tempfile::TempDir::new().unwrap();
        write_fresh_cache(tmp.path(), &crate::commands::update_check::current_tag());
        let mut out: Vec<u8> = Vec::new();
        let mut notice: Vec<u8> = Vec::new();

        // Act
        run_to(tmp.path(), &mut out, &mut notice).unwrap();

        // Assert
        assert!(notice.is_empty(), "an equal tag must not notify");
        assert_eq!(String::from_utf8(out).unwrap(), render());
    }

    #[test]
    fn test_run_to_skips_check_and_writes_nothing_when_data_dir_absent() {
        // Arrange — a path that does not exist; agents-md must work before
        // `homeos init` and must never create the data directory
        let guard = EnvVarGuard::capture("HOMEOS_SKIP_UPDATE_CHECK");
        guard.unset();
        let tmp = tempfile::TempDir::new().unwrap();
        let absent = tmp.path().join("not-initialized");
        let mut out: Vec<u8> = Vec::new();
        let mut notice: Vec<u8> = Vec::new();

        // Act
        run_to(&absent, &mut out, &mut notice).unwrap();

        // Assert — nothing on the notice writer, no data dir, no cache file
        assert!(notice.is_empty());
        assert!(!absent.exists(), "agents-md must not create the data dir");
        assert!(!crate::commands::update_check::cache_path(&absent).exists());
        assert_eq!(String::from_utf8(out).unwrap(), render());
    }

    #[test]
    fn test_render_includes_all_top_level_section_headers() {
        // Arrange & Act
        let rendered = render();

        // Assert — each PRD-mandated section header appears exactly as written
        let sections = [
            "## Overview",
            "## Speaking to the user",
            "## Operating principles",
            "## First-time setup of a new repository",
            "## Error JSON schema",
            "## Input safety",
            "## Canonical workflows",
            "## Choosing a package manager plugin",
            "## Per-command reference",
            "## Plugin authoring",
            "## Maintaining the repository README",
        ];
        for section in sections {
            assert!(
                rendered.contains(section),
                "rendered template missing section header: {section}"
            );
        }
    }

    #[test]
    fn test_render_documents_dry_run_yes_json_flags() {
        // Arrange & Act
        let rendered = render();

        // Assert — Operating principles must mention all three non-interactive flags
        assert!(rendered.contains("--dry-run"));
        assert!(rendered.contains("--yes"));
        assert!(rendered.contains("--json"));
    }

    /// Every reason string declared in the `reasons` module of `src/error.rs`,
    /// parsed from the source itself so that a newly added constant cannot
    /// escape the rendered reason table unnoticed.
    fn declared_reasons() -> BTreeSet<String> {
        const ERROR_RS: &str = include_str!("../error.rs");
        ERROR_RS
            .lines()
            .skip_while(|line| !line.starts_with("pub mod reasons {"))
            .skip(1)
            .take_while(|line| !line.starts_with('}'))
            .filter_map(|line| {
                let declaration = line.trim().strip_prefix("pub const ")?;
                let value = declaration.split_once("= \"")?.1;
                value.strip_suffix("\";").map(str::to_string)
            })
            .collect()
    }

    /// The reasons documented in the rendered reason table. It is the only
    /// table in the template whose rows open with a backticked cell, so the
    /// prefix alone identifies its rows.
    fn documented_reasons(rendered: &str) -> BTreeSet<String> {
        rendered
            .lines()
            .filter_map(|line| line.strip_prefix("| `"))
            .filter_map(|row| row.split_once('`').map(|(reason, _)| reason.to_string()))
            .collect()
    }

    #[test]
    fn test_render_enumerates_canonical_error_reasons() {
        // Arrange — the full set of reasons, straight from src/error.rs
        let declared = declared_reasons();
        assert!(
            declared.len() >= 20,
            "failed to parse the reasons module of src/error.rs, got: {declared:?}"
        );

        // Act
        let rendered = render();

        // Assert — the table matches src/error.rs exactly, in both directions
        assert_eq!(
            documented_reasons(&rendered),
            declared,
            "rendered reason table drifted from the reasons module of src/error.rs"
        );
    }

    #[test]
    fn test_render_documents_name_and_url_safety_rules() {
        // Arrange & Act
        let rendered = render();

        // Assert — the regex pattern and the allowed URL schemes must appear
        assert!(rendered.contains("^[a-z0-9][a-z0-9._-]*$"));
        for scheme in ["http", "https", "git", "ssh", "git+ssh"] {
            assert!(
                rendered.contains(scheme),
                "rendered template missing allowed URL scheme: {scheme}"
            );
        }
    }

    #[test]
    fn test_render_includes_install_neovim_walkthrough() {
        // Arrange & Act
        let rendered = render();

        // Assert — PRD names "Install Neovim for me" as the headline walk-through
        assert!(rendered.contains("Install Neovim for me"));
        assert!(rendered.contains("homeos --yes --json apply"));
    }

    #[test]
    fn test_render_uninstall_walkthrough_is_machine_local() {
        // Arrange & Act
        let rendered = render();

        // Assert — the pre-archived auto-disable guidance must be gone
        assert!(rendered.contains("machine-local"));
        assert!(rendered.contains("never edits `homeos.yml`"));
        assert!(!rendered.contains("disables it in `homeos.yml`"));
        assert!(!rendered.contains("Disable neovim after uninstall"));
    }

    #[test]
    fn test_render_documents_archive_then_uninstall_as_removal_from_every_machine() {
        // Arrange & Act
        let rendered = render();

        // Assert — archive + narrow uninstall is the every-machine removal path
        assert!(rendered.contains("homeos --yes --json package archive neovim"));
        assert!(rendered.contains("tombstone"));
        assert!(rendered.contains("homeos package unarchive <name>"));
        assert!(rendered.contains("package uninstall neovim"));
        // Assert — `apply` remains the mechanism the *other* machines use
        assert!(rendered.contains("reconciled by `apply`"));
        assert!(rendered.contains("next pull"));
    }

    #[test]
    fn test_render_documents_package_remove_as_optional_final_cleanup() {
        // Arrange & Act
        let rendered = render();

        // Assert
        assert!(rendered.contains("optional final cleanup"));
        assert!(rendered.contains("Don't offer `remove` as a shortcut for `archive`."));
    }

    #[test]
    fn test_render_apply_scope_mentions_uninstalling_archived_packages() {
        // Arrange & Act
        let rendered = render();

        // Assert
        assert!(rendered.contains("uninstalls every archived package still in state"));
    }

    #[test]
    fn test_render_readme_package_table_excludes_archived_packages() {
        // Arrange & Act
        let rendered = render();

        // Assert — archived packages get no row, and the example table shows none
        assert!(rendered.contains("`archived` is not `true`"));
        assert!(!rendered.contains("(packages/ollama/) (archived)"));
        // Assert — the disabled suffix rule survives untouched
        assert!(rendered.contains("` (disabled)` when the package's `enabled` is `false`"));
    }

    #[test]
    fn test_render_readme_documents_skills_table() {
        // Arrange & Act
        let rendered = render();

        // Assert — the README structure carries a Skills section of its own
        assert!(rendered.contains("## Skills"));
        assert!(rendered.contains("| Skill | Plugin | Dependencies | Purpose |"));
        // Assert — a package is classified by what it installs, not by its plugin
        assert!(rendered.contains("**Skills table**"));
        assert!(rendered.contains("not by which plugin backs it"));
        // Assert — a section with no rows is omitted entirely
        assert!(rendered.contains("Omit either section"));
        // Assert — the ownership boundary covers the new section
        assert!(
            rendered.contains("You own the **Packages**, **Skills**, and **Plugins** sections")
        );
    }

    #[test]
    fn test_render_includes_git_commit_convention() {
        // Arrange & Act
        let rendered = render();

        // Assert — the add/commit pattern after mutation is documented, and
        // every git command in the guide targets the home explicitly (an agent
        // started inside an unrelated project must never commit that project)
        assert!(rendered.contains("git -C <data_dir> add -A"));
        assert!(rendered.contains("git -C <data_dir> commit"));
        for line in rendered.lines() {
            if line.starts_with("git ") {
                assert!(
                    line.starts_with("git -C <data_dir> ")
                        || line.starts_with("git config --global "),
                    "bare git command in the guide: {line:?}"
                );
            }
        }
    }

    #[test]
    fn test_render_includes_os_to_plugin_mapping_entries() {
        // Arrange & Act
        let rendered = render();

        // Assert — the mapping table must cover the four canonical package managers
        assert!(rendered.contains("dnf"));
        assert!(rendered.contains("apt"));
        assert!(rendered.contains("homebrew"));
        assert!(rendered.contains("winget"));
    }
}
