use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::engine::ArgValueCompleter;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value pair: no '=' found in '{s}'"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

mod commands;
mod completers;
mod config;
mod context;
#[cfg(test)]
mod env_test;
mod error;
mod git;
mod output;
mod plan;
mod state;
mod topo;
mod validation;

use output::OutputFormat;

#[derive(Parser)]
#[command(name = "homeos", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Override the data directory (defaults to OS data directory)
    #[arg(long, global = true, hide = true)]
    pub data_dir: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, value_enum, conflicts_with = "json")]
    pub output: Option<OutputFormat>,

    /// Shorthand for --output json
    #[arg(long, global = true)]
    pub json: bool,

    /// Skip the confirmation prompt and proceed immediately
    #[arg(long, global = true)]
    pub yes: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create the initial repository structure
    Init {
        /// Remote URL to clone
        url: Option<String>,
        /// Remove .git directory after cloning
        #[arg(long, requires = "url")]
        strip_git: bool,
    },
    /// Launch a shell in the repositories directory
    Cd {
        /// Print the data directory path to stdout and exit, without launching a shell
        #[arg(long)]
        print: bool,
    },
    /// Install new packages and update installed ones
    Apply {
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage packages
    Package {
        #[command(subcommand)]
        command: PackageCommands,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
    /// Print a shell completion script to stdout
    Completion {
        /// Target shell
        #[arg(value_enum)]
        shell: commands::completion::CompletionShell,
    },
    /// Print the operating guide
    Guide,
}

#[derive(Subcommand)]
pub enum PluginCommands {
    /// List all plugins
    #[command(visible_alias = "ls")]
    List,
    /// List official plugins available from GitHub
    #[command(visible_alias = "ls-remote")]
    ListRemote,
    /// Register a plugin
    Add {
        /// Plugin name
        plugin: String,
        /// Remote URL to clone (defaults to official repository)
        url: Option<String>,
        /// Create an empty plugin skeleton for local development
        #[arg(long)]
        local: bool,
    },
    /// Remove a plugin
    Remove {
        /// Plugin name
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: String,
        /// Also delete the plugin directory
        #[arg(long)]
        purge: bool,
    },
    /// Refetch a plugin's templates from its registered URL
    Refresh {
        /// Plugin name (required unless --all)
        #[arg(
            required_unless_present = "all",
            add = ArgValueCompleter::new(completers::plugin_completer),
        )]
        plugin: Option<String>,
        /// Refresh every registered plugin
        #[arg(long)]
        all: bool,
        /// Show what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Display plugin details
    Info {
        /// Plugin name
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: String,
    },
    /// Display plugin.yml and all template files for a plugin
    Cat {
        /// Plugin name
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: String,
    },
    /// Launch a shell in the plugins root or specific plugin directory
    Cd {
        /// Plugin name (optional — defaults to plugins root)
        #[arg(add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: Option<String>,
        /// Print the resolved path to stdout and exit, without launching a shell
        #[arg(long)]
        print: bool,
    },
}

#[derive(Subcommand)]
pub enum PackageCommands {
    /// List all packages
    #[command(visible_alias = "ls")]
    List,
    /// Add a new package
    Add {
        /// Package name
        package: String,
        /// Add a dependency (can be repeated)
        #[arg(
            long = "depends-on",
            action = clap::ArgAction::Append,
            add = ArgValueCompleter::new(completers::package_completer),
        )]
        depends_on: Vec<String>,
        /// Add a script alias as target=source (can be repeated)
        #[arg(long = "script-alias", action = clap::ArgAction::Append, value_parser = parse_key_value)]
        script_aliases: Vec<(String, String)>,
        /// Plugin to use for generating scripts
        #[arg(long, add = ArgValueCompleter::new(completers::plugin_completer))]
        plugin: Option<String>,
        /// Plugin parameter as key=value (can be repeated)
        #[arg(long = "param", action = clap::ArgAction::Append, value_parser = parse_key_value)]
        params: Vec<(String, String)>,
    },
    /// Remove package entries from homeos.yml
    Remove {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
        /// Also delete the package directory
        #[arg(long)]
        purge: bool,
    },
    /// Rename a package
    Rename {
        /// Current package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        old: String,
        /// New package name
        new: String,
    },
    /// Add dependencies to an existing package
    AddDep {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Dependencies to add
        #[arg(required = true)]
        dependency: Vec<String>,
    },
    /// Remove dependencies from an existing package
    RemoveDep {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Dependencies to remove
        #[arg(required = true)]
        dependency: Vec<String>,
    },
    /// Add script aliases to an existing package
    AddAlias {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Aliases as target=source pairs (e.g., update=install)
        #[arg(required = true, value_parser = parse_key_value)]
        alias: Vec<(String, String)>,
    },
    /// Remove script aliases from a package
    RemoveAlias {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
        /// Alias targets to remove (e.g., update)
        #[arg(required = true)]
        alias: Vec<String>,
    },
    /// Enable packages
    Enable {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
    },
    /// Disable packages
    Disable {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
    },
    /// Archive packages: mark them as removed from every machine
    Archive {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
    },
    /// Unarchive packages
    Unarchive {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
    },
    /// Display package details
    Info {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
    },
    /// Display all scripts for a package
    Cat {
        /// Package name
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: String,
    },
    /// Launch a shell in the package root or specific package directory
    Cd {
        /// Package name (optional — defaults to packages root)
        #[arg(add = ArgValueCompleter::new(completers::package_completer))]
        package: Option<String>,
        /// Print the resolved path to stdout and exit, without launching a shell
        #[arg(long)]
        print: bool,
    },
    /// Execute install scripts
    Install {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute update scripts
    Update {
        /// Package names
        #[arg(required = true, add = ArgValueCompleter::new(completers::package_completer))]
        packages: Vec<String>,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute uninstall scripts
    Uninstall {
        /// Package names
        #[arg(
            required_unless_present = "all",
            add = ArgValueCompleter::new(completers::package_completer),
        )]
        packages: Vec<String>,
        /// Uninstall all installed packages (from state.yml)
        #[arg(long)]
        all: bool,
        /// Display the plan without executing scripts or prompting
        #[arg(long)]
        dry_run: bool,
    },
}

fn validate_args(command: &Commands) -> Result<(), error::HomeosError> {
    use validation::{validate_name, validate_url};
    match command {
        Commands::Init { url, .. } => {
            if let Some(u) = url {
                validate_url(u)?;
            }
        }
        Commands::Cd { .. }
        | Commands::Apply { .. }
        | Commands::Completion { .. }
        | Commands::Guide => {}
        Commands::Package { command } => match command {
            PackageCommands::List => {}
            PackageCommands::Add {
                package,
                depends_on,
                plugin,
                ..
            } => {
                validate_name(package)?;
                for d in depends_on {
                    validate_name(d)?;
                }
                if let Some(p) = plugin {
                    validate_name(p)?;
                }
            }
            PackageCommands::Remove { packages, .. } => {
                for p in packages {
                    validate_name(p)?;
                }
            }
            PackageCommands::Rename { old, new } => {
                validate_name(old)?;
                validate_name(new)?;
            }
            PackageCommands::AddDep {
                package,
                dependency,
            }
            | PackageCommands::RemoveDep {
                package,
                dependency,
            } => {
                validate_name(package)?;
                for d in dependency {
                    validate_name(d)?;
                }
            }
            PackageCommands::AddAlias { package, .. }
            | PackageCommands::RemoveAlias { package, .. }
            | PackageCommands::Info { package }
            | PackageCommands::Cat { package } => {
                validate_name(package)?;
            }
            PackageCommands::Cd { package, .. } => {
                if let Some(p) = package {
                    validate_name(p)?;
                }
            }
            PackageCommands::Enable { packages }
            | PackageCommands::Disable { packages }
            | PackageCommands::Archive { packages }
            | PackageCommands::Unarchive { packages }
            | PackageCommands::Install { packages, .. }
            | PackageCommands::Update { packages, .. }
            | PackageCommands::Uninstall { packages, .. } => {
                for p in packages {
                    validate_name(p)?;
                }
            }
        },
        Commands::Plugin { command } => match command {
            PluginCommands::List | PluginCommands::ListRemote => {}
            PluginCommands::Add { plugin, url, .. } => {
                validate_name(plugin)?;
                if let Some(u) = url {
                    validate_url(u)?;
                }
            }
            PluginCommands::Remove { plugin, .. }
            | PluginCommands::Info { plugin }
            | PluginCommands::Cat { plugin } => {
                validate_name(plugin)?;
            }
            PluginCommands::Refresh { plugin, .. } => {
                if let Some(p) = plugin {
                    validate_name(p)?;
                }
            }
            PluginCommands::Cd { plugin, .. } => {
                if let Some(p) = plugin {
                    validate_name(p)?;
                }
            }
        },
    }
    Ok(())
}

fn dispatch(ctx: &context::Context, command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    validate_args(&command)?;
    match command {
        Commands::Init { url, strip_git } => commands::init::run(ctx, url.as_deref(), strip_git),
        Commands::Cd { print } => commands::cd::run(ctx, print),
        Commands::Apply { dry_run } => commands::package::apply(ctx, dry_run),
        Commands::Package { command } => match command {
            PackageCommands::List => commands::package::list(ctx),
            PackageCommands::Add {
                package,
                depends_on,
                script_aliases,
                plugin,
                params,
            } => {
                let script_aliases_map: BTreeMap<String, String> =
                    script_aliases.into_iter().collect();
                let params_map: BTreeMap<String, String> = params.into_iter().collect();
                commands::package::add(
                    ctx,
                    &package,
                    &depends_on,
                    &script_aliases_map,
                    plugin.as_deref(),
                    &params_map,
                )
            }
            PackageCommands::Remove { packages, purge } => {
                commands::package::remove(ctx, &packages, purge)
            }
            PackageCommands::Rename { old, new } => commands::package::rename(ctx, &old, &new),
            PackageCommands::AddDep {
                package,
                dependency,
            } => commands::package::add_dep(ctx, &package, &dependency),
            PackageCommands::RemoveDep {
                package,
                dependency,
            } => commands::package::remove_dep(ctx, &package, &dependency),
            PackageCommands::AddAlias { package, alias } => {
                commands::package::add_alias(ctx, &package, &alias)
            }
            PackageCommands::RemoveAlias { package, alias } => {
                commands::package::remove_alias(ctx, &package, &alias)
            }
            PackageCommands::Enable { packages } => commands::package::enable(ctx, &packages),
            PackageCommands::Disable { packages } => commands::package::disable(ctx, &packages),
            PackageCommands::Archive { packages } => commands::package::archive(ctx, &packages),
            PackageCommands::Unarchive { packages } => commands::package::unarchive(ctx, &packages),
            PackageCommands::Info { package } => commands::package::info(ctx, &package),
            PackageCommands::Cat { package } => commands::package::cat(ctx, &package),
            PackageCommands::Cd { package, print } => {
                commands::package::cd(ctx, package.as_deref(), print)
            }
            PackageCommands::Install { packages, dry_run } => {
                commands::package::install(ctx, &packages, dry_run)
            }
            PackageCommands::Update { packages, dry_run } => {
                commands::package::update(ctx, &packages, dry_run)
            }
            PackageCommands::Uninstall {
                packages,
                all,
                dry_run,
            } => commands::package::uninstall(ctx, &packages, all, dry_run),
        },
        Commands::Plugin { command } => match command {
            PluginCommands::List => commands::plugin::list(ctx),
            PluginCommands::ListRemote => commands::plugin::list_remote(ctx),
            PluginCommands::Add { plugin, url, local } => {
                commands::plugin::add(ctx, &plugin, url.as_deref(), local)
            }
            PluginCommands::Remove { plugin, purge } => {
                commands::plugin::remove(ctx, &plugin, purge)
            }
            PluginCommands::Refresh {
                plugin,
                all,
                dry_run,
            } => commands::plugin::refresh(ctx, plugin.as_deref(), all, dry_run),
            PluginCommands::Info { plugin } => commands::plugin::info(ctx, &plugin),
            PluginCommands::Cat { plugin } => commands::plugin::cat(ctx, &plugin),
            PluginCommands::Cd { plugin, print } => {
                commands::plugin::cd(ctx, plugin.as_deref(), print)
            }
        },
        Commands::Completion { shell } => commands::completion::run(shell),
        Commands::Guide => commands::guide::run(),
    }
}

fn main() {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();
    let output_format = OutputFormat::resolve(cli.output, cli.json);
    let ctx = context::Context::new(cli.data_dir)
        .with_output_format(output_format)
        .with_yes(cli.yes);

    if let Err(e) = dispatch(&ctx, cli.command) {
        error::report(e.as_ref(), output_format);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_add_dep_help_shows_dependency_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let add_dep_cmd = package_cmd.find_subcommand("add-dep").unwrap();

        // Act
        let args: Vec<&str> = add_dep_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "dependency")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["dependency"]);
    }

    #[test]
    fn test_package_ls_alias_resolves_to_list() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "ls"]).unwrap();

        // Assert
        assert!(matches!(
            cli.command,
            Commands::Package {
                command: PackageCommands::List
            }
        ));
    }

    #[test]
    fn test_plugin_ls_alias_resolves_to_list() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "ls"]).unwrap();

        // Assert
        assert!(matches!(
            cli.command,
            Commands::Plugin {
                command: PluginCommands::List
            }
        ));
    }

    #[test]
    fn test_plugin_ls_remote_alias_resolves_to_list_remote() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "ls-remote"]).unwrap();

        // Assert
        assert!(matches!(
            cli.command,
            Commands::Plugin {
                command: PluginCommands::ListRemote
            }
        ));
    }

    #[test]
    fn test_package_ls_alias_visible_in_help() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let list_cmd = package_cmd.find_subcommand("list").unwrap();

        // Act
        let aliases: Vec<&str> = list_cmd.get_visible_aliases().collect();

        // Assert
        assert_eq!(aliases, vec!["ls"]);
    }

    #[test]
    fn test_plugin_ls_aliases_visible_in_help() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let list_cmd = plugin_cmd.find_subcommand("list").unwrap();
        let list_remote_cmd = plugin_cmd.find_subcommand("list-remote").unwrap();

        // Act
        let list_aliases: Vec<&str> = list_cmd.get_visible_aliases().collect();
        let list_remote_aliases: Vec<&str> = list_remote_cmd.get_visible_aliases().collect();

        // Assert
        assert_eq!(list_aliases, vec!["ls"]);
        assert_eq!(list_remote_aliases, vec!["ls-remote"]);
    }

    #[test]
    fn test_cli_version_matches_cargo_pkg_version() {
        // Arrange
        let cmd = Cli::command();

        // Act
        let version = cmd.get_version();

        // Assert
        assert_eq!(version, Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_cli_version_flag_long() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "--version"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected DisplayVersion error"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_cli_version_flag_short() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "-V"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected DisplayVersion error"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_remove_dep_help_shows_dependency_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let remove_dep_cmd = package_cmd.find_subcommand("remove-dep").unwrap();

        // Act
        let args: Vec<&str> = remove_dep_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "dependency")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["dependency"]);
    }

    #[test]
    fn test_add_alias_help_shows_alias_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let add_alias_cmd = package_cmd.find_subcommand("add-alias").unwrap();

        // Act
        let args: Vec<&str> = add_alias_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "alias")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["alias"]);
    }

    #[test]
    fn test_remove_alias_help_shows_alias_argument() {
        // Arrange
        let cmd = Cli::command();
        let package_cmd = cmd.find_subcommand("package").unwrap();
        let remove_alias_cmd = package_cmd.find_subcommand("remove-alias").unwrap();

        // Act
        let args: Vec<&str> = remove_alias_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "alias")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["alias"]);
    }

    #[test]
    fn test_plugin_add_local_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "add", "custom", "--local"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Add { plugin, url, local },
        } = cli.command
        {
            assert_eq!(plugin, "custom");
            assert!(url.is_none());
            assert!(local);
        } else {
            panic!("Expected PluginCommands::Add");
        }
    }

    #[test]
    fn test_plugin_add_without_local_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "add", "dnf"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Add { local, .. },
        } = cli.command
        {
            assert!(!local);
        } else {
            panic!("Expected PluginCommands::Add");
        }
    }

    #[test]
    fn test_add_plugin_option() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "add", "neovim", "--plugin", "dnf"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { plugin, .. },
        } = cli.command
        {
            assert_eq!(plugin, Some("dnf".to_string()));
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_script_alias_option() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--script-alias",
            "update=install",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { script_aliases, .. },
        } = cli.command
        {
            assert_eq!(
                script_aliases,
                vec![("update".to_string(), "install".to_string())]
            );
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_without_script_alias_defaults_to_empty() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "add", "neovim"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { script_aliases, .. },
        } = cli.command
        {
            assert!(script_aliases.is_empty());
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_param_option() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--plugin",
            "dnf",
            "--param",
            "name=neovim.x86_64",
            "--param",
            "repo=extra",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { params, .. },
        } = cli.command
        {
            assert_eq!(
                params,
                vec![
                    ("name".to_string(), "neovim.x86_64".to_string()),
                    ("repo".to_string(), "extra".to_string()),
                ]
            );
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_plugin_add_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let add_cmd = plugin_cmd.find_subcommand("add").unwrap();

        // Act
        let args: Vec<&str> = add_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_plugin_remove_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let remove_cmd = plugin_cmd.find_subcommand("remove").unwrap();

        // Act
        let args: Vec<&str> = remove_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_plugin_remove_purge_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "remove", "dnf", "--purge"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Remove { plugin, purge },
        } = cli.command
        {
            assert_eq!(plugin, "dnf");
            assert!(purge);
        } else {
            panic!("Expected PluginCommands::Remove");
        }
    }

    #[test]
    fn test_plugin_remove_without_purge_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "plugin", "remove", "dnf"]).unwrap();

        // Assert
        if let Commands::Plugin {
            command: PluginCommands::Remove { purge, .. },
        } = cli.command
        {
            assert!(!purge);
        } else {
            panic!("Expected PluginCommands::Remove");
        }
    }

    #[test]
    fn test_plugin_cat_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let cat_cmd = plugin_cmd.find_subcommand("cat").unwrap();

        // Act
        let args: Vec<&str> = cat_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_plugin_cd_help_shows_plugin_argument() {
        // Arrange
        let cmd = Cli::command();
        let plugin_cmd = cmd.find_subcommand("plugin").unwrap();
        let cd_cmd = plugin_cmd.find_subcommand("cd").unwrap();

        // Act
        let args: Vec<&str> = cd_cmd
            .get_positionals()
            .filter(|a| a.get_id() == "plugin")
            .map(|a| a.get_id().as_str())
            .collect();

        // Assert
        assert_eq!(args, vec!["plugin"]);
    }

    #[test]
    fn test_add_without_plugin_defaults_to_none() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "add", "neovim"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { plugin, params, .. },
        } = cli.command
        {
            assert!(plugin.is_none());
            assert!(params.is_empty());
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_depends_on_does_not_consume_subsequent_options() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "claude",
            "--depends-on",
            "git",
            "--depends-on",
            "curl",
            "--plugin",
            "dnf",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add {
                depends_on, plugin, ..
            },
        } = cli.command
        {
            assert_eq!(depends_on, vec!["git", "curl"]);
            assert_eq!(plugin, Some("dnf".to_string()));
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_script_alias_can_be_repeated() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--script-alias",
            "update=install",
            "--script-alias",
            "uninstall=install",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Add { script_aliases, .. },
        } = cli.command
        {
            assert_eq!(
                script_aliases,
                vec![
                    ("update".to_string(), "install".to_string()),
                    ("uninstall".to_string(), "install".to_string()),
                ]
            );
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_add_param_does_not_consume_subsequent_options() {
        // Arrange & Act
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "neovim",
            "--plugin",
            "dnf",
            "--param",
            "name=neovim.x86_64",
            "--depends-on",
            "git",
            "--param",
            "repo=extra",
        ])
        .unwrap();

        // Assert
        if let Commands::Package {
            command:
                PackageCommands::Add {
                    depends_on,
                    params,
                    plugin,
                    ..
                },
        } = cli.command
        {
            assert_eq!(
                params,
                vec![
                    ("name".to_string(), "neovim.x86_64".to_string()),
                    ("repo".to_string(), "extra".to_string()),
                ]
            );
            assert_eq!(depends_on, vec!["git"]);
            assert_eq!(plugin, Some("dnf".to_string()));
        } else {
            panic!("Expected PackageCommands::Add");
        }
    }

    #[test]
    fn test_package_remove_purge_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "remove", "neovim", "--purge"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Remove { packages, purge },
        } = cli.command
        {
            assert_eq!(packages, vec!["neovim".to_string()]);
            assert!(purge);
        } else {
            panic!("Expected PackageCommands::Remove");
        }
    }

    #[test]
    fn test_package_remove_without_purge_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "package", "remove", "neovim"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Remove { purge, .. },
        } = cli.command
        {
            assert!(!purge);
        } else {
            panic!("Expected PackageCommands::Remove");
        }
    }

    #[test]
    fn test_apply_dry_run_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Apply { dry_run } = cli.command {
            assert!(dry_run);
        } else {
            panic!("Expected Commands::Apply");
        }
    }

    #[test]
    fn test_apply_without_dry_run_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply"]).unwrap();

        // Assert
        if let Commands::Apply { dry_run } = cli.command {
            assert!(!dry_run);
        } else {
            panic!("Expected Commands::Apply");
        }
    }

    #[test]
    fn test_package_install_dry_run_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "install", "neovim", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Install { dry_run, .. },
        } = cli.command
        {
            assert!(dry_run);
        } else {
            panic!("Expected PackageCommands::Install");
        }
    }

    #[test]
    fn test_package_update_dry_run_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "update", "neovim", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Update { dry_run, .. },
        } = cli.command
        {
            assert!(dry_run);
        } else {
            panic!("Expected PackageCommands::Update");
        }
    }

    #[test]
    fn test_package_uninstall_dry_run_flag() {
        // Arrange & Act
        let cli =
            Cli::try_parse_from(["homeos", "package", "uninstall", "neovim", "--dry-run"]).unwrap();

        // Assert
        if let Commands::Package {
            command: PackageCommands::Uninstall { dry_run, .. },
        } = cli.command
        {
            assert!(dry_run);
        } else {
            panic!("Expected PackageCommands::Uninstall");
        }
    }

    #[test]
    fn test_output_flag_defaults_to_none() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply"]).unwrap();

        // Assert
        assert!(cli.output.is_none());
        assert!(!cli.json);
    }

    #[test]
    fn test_output_flag_parses_json() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--output", "json", "apply"]).unwrap();

        // Assert
        assert_eq!(cli.output, Some(OutputFormat::Json));
    }

    #[test]
    fn test_output_flag_parses_text() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--output", "text", "apply"]).unwrap();

        // Assert
        assert_eq!(cli.output, Some(OutputFormat::Text));
    }

    #[test]
    fn test_json_shorthand_flag() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--json", "apply"]).unwrap();

        // Assert
        assert!(cli.json);
        assert!(cli.output.is_none());
    }

    #[test]
    fn test_output_and_json_flags_conflict() {
        // Arrange & Act
        let result = Cli::try_parse_from(["homeos", "--output", "json", "--json", "apply"]);

        // Assert
        let err = match result {
            Ok(_) => panic!("expected --output and --json to conflict"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_output_flag_is_global() {
        // Arrange & Act — accept after the subcommand
        let cli = Cli::try_parse_from(["homeos", "package", "list", "--output", "json"]).unwrap();

        // Assert
        assert_eq!(cli.output, Some(OutputFormat::Json));
    }

    #[test]
    fn test_json_flag_is_global() {
        // Arrange & Act — accept after the subcommand
        let cli = Cli::try_parse_from(["homeos", "package", "list", "--json"]).unwrap();

        // Assert
        assert!(cli.json);
    }

    #[test]
    fn test_validate_args_accepts_well_formed_package_add() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "package", "add", "neovim"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_rejects_path_traversal_in_package_name() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "package", "info", "../etc"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_invalid_plugin_name() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "plugin", "info", "BadName"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_invalid_dependency_in_package_add() {
        // Arrange
        let cli = Cli::try_parse_from([
            "homeos",
            "package",
            "add",
            "claude",
            "--depends-on",
            "foo/bar",
        ])
        .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_invalid_plugin_value_in_package_add() {
        // Arrange
        let cli =
            Cli::try_parse_from(["homeos", "package", "add", "claude", "--plugin", "Foo Bar"])
                .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_validates_every_element_in_packages_list() {
        // Arrange — first element valid, second element rejected
        let cli =
            Cli::try_parse_from(["homeos", "package", "install", "neovim", "bad/name"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_invalid_rename_new() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "package", "rename", "old", ".hidden"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_skips_validation_when_no_names_present() {
        // Arrange — `package list` carries no name args
        let cli = Cli::try_parse_from(["homeos", "package", "list"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_skips_optional_package_when_none() {
        // Arrange — `package cd` without an argument
        let cli = Cli::try_parse_from(["homeos", "package", "cd"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_validates_optional_package_when_some() {
        // Arrange — `package cd <name>` with a bad name
        let cli = Cli::try_parse_from(["homeos", "package", "cd", "-rf"]);

        // Assert clap rejects the leading-dash form at parse time — it
        // looks like an unknown flag. Validation is a defense-in-depth
        // layer on top of clap, not a replacement.
        assert!(cli.is_err());
    }

    #[test]
    fn test_validate_args_accepts_init_without_url() {
        // Arrange — scaffold mode has no URL to validate
        let cli = Cli::try_parse_from(["homeos", "init"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_accepts_init_with_https_url() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "init", "https://github.com/hainet50b/dotfiles"])
            .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_rejects_init_url_with_unsupported_scheme() {
        // Arrange — `file://` is not in the allowed scheme list
        let cli = Cli::try_parse_from(["homeos", "init", "file:///etc/passwd"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_init_url_with_no_scheme() {
        // Arrange — bare host without explicit scheme
        let cli = Cli::try_parse_from(["homeos", "init", "github.com/user/repo"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_init_url_with_percent_encoded_dotdot() {
        // Arrange — common URL-encoded path traversal payload
        let cli =
            Cli::try_parse_from(["homeos", "init", "https://example.com/%2e%2e/etc"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_init_url_with_query_string() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "init", "https://example.com/repo.git?inject=1"])
            .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_accepts_plugin_add_without_url() {
        // Arrange — URL is optional; auto-resolves to the official repo
        let cli = Cli::try_parse_from(["homeos", "plugin", "add", "dnf"]).unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_accepts_plugin_add_with_https_url() {
        // Arrange
        let cli = Cli::try_parse_from([
            "homeos",
            "plugin",
            "add",
            "dnf",
            "https://github.com/hainet50b/homeos-plugin-dnf",
        ])
        .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_args_rejects_plugin_add_url_with_unsupported_scheme() {
        // Arrange
        let cli = Cli::try_parse_from(["homeos", "plugin", "add", "evil", "javascript:alert(1)"])
            .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_validate_args_rejects_plugin_add_url_with_query_string() {
        // Arrange
        let cli = Cli::try_parse_from([
            "homeos",
            "plugin",
            "add",
            "dnf",
            "https://example.com/repo.git?evil=1",
        ])
        .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
    }

    #[test]
    fn test_yes_flag_defaults_to_false() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply"]).unwrap();

        // Assert
        assert!(!cli.yes);
    }

    #[test]
    fn test_yes_flag_is_global() {
        // Arrange & Act — accept after the subcommand
        let cli = Cli::try_parse_from(["homeos", "package", "install", "neovim", "--yes"]).unwrap();

        // Assert
        assert!(cli.yes);
    }

    #[test]
    fn test_yes_flag_compatible_with_json() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "--json", "--yes", "apply"]).unwrap();

        // Assert
        assert!(cli.yes);
        assert!(cli.json);
    }

    #[test]
    fn test_yes_flag_compatible_with_dry_run() {
        // Arrange & Act
        let cli = Cli::try_parse_from(["homeos", "apply", "--dry-run", "--yes"]).unwrap();

        // Assert
        assert!(cli.yes);
        if let Commands::Apply { dry_run } = cli.command {
            assert!(dry_run);
        } else {
            panic!("Expected Commands::Apply");
        }
    }

    #[test]
    fn test_validate_args_validates_plugin_name_before_url() {
        // Arrange — both name and URL are invalid; name check should fire first
        let cli =
            Cli::try_parse_from(["homeos", "plugin", "add", "Bad/Name", "javascript:alert(1)"])
                .unwrap();

        // Act
        let result = validate_args(&cli.command);

        // Assert — error message should reference the name, not the URL
        let err = result.unwrap_err();
        assert_eq!(err.reason, error::reasons::VALIDATION_ERROR);
        assert!(
            err.message.contains("Name 'Bad/Name'"),
            "expected name-validation message, got: {}",
            err.message
        );
    }
}
