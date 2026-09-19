use crate::config::{Config, PluginConfig, PluginManifest};
use crate::context::Context;
use crate::error::{HomeosError, reasons};
use crate::git;
use crate::output::OutputFormat;
use crate::plan::prompt_confirm;
use serde::Deserialize;
use std::io::{BufRead, Write};
use std::path::Path;

pub fn list(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    list_to(ctx, &mut std::io::stdout())
}

fn load_plugin_description(plugin_dir: &Path) -> String {
    let manifest_path = plugin_dir.join("plugin.yml");
    if manifest_path.is_file() {
        PluginManifest::load(&manifest_path)
            .map(|m| m.description)
            .unwrap_or_default()
    } else {
        String::new()
    }
}

fn list_to<W: Write>(ctx: &Context, writer: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    let entries: Vec<(String, String, Option<String>)> = config
        .plugins
        .iter()
        .map(|(name, plugin)| {
            let desc = load_plugin_description(&ctx.plugins_dir().join(name));
            (name.clone(), desc, plugin.url.clone())
        })
        .collect();

    match ctx.output_format() {
        OutputFormat::Json => list_json(writer, &entries),
        OutputFormat::Text => list_text(writer, &entries),
    }
}

fn list_json<W: Write>(
    writer: &mut W,
    entries: &[(String, String, Option<String>)],
) -> Result<(), Box<dyn std::error::Error>> {
    let rows: Vec<serde_json::Value> = entries
        .iter()
        .map(|(name, desc, url)| {
            serde_json::json!({
                "name": name,
                "description": desc,
                "url": url,
            })
        })
        .collect();
    writeln!(writer, "{}", serde_json::Value::Array(rows))?;
    Ok(())
}

fn list_text<W: Write>(
    writer: &mut W,
    entries: &[(String, String, Option<String>)],
) -> Result<(), Box<dyn std::error::Error>> {
    let displayed: Vec<(String, String, String)> = entries
        .iter()
        .map(|(name, desc, url)| {
            let url_str = url.clone().unwrap_or_else(|| "(local)".to_string());
            (name.clone(), desc.clone(), url_str)
        })
        .collect();

    let name_width = displayed
        .iter()
        .map(|(n, _, _)| n.len())
        .max()
        .unwrap_or(0)
        .max(4); // "Name" header length

    let desc_width = displayed
        .iter()
        .map(|(_, d, _)| d.len())
        .max()
        .unwrap_or(0)
        .max(11); // "Description" header length

    let url_width = displayed
        .iter()
        .map(|(_, _, u)| u.len())
        .max()
        .unwrap_or(0)
        .max(3); // "URL" header length

    writeln!(
        writer,
        "{:<name_width$}  {:<desc_width$}  {:<url_width$}",
        "Name", "Description", "URL"
    )?;
    writeln!(
        writer,
        "{:<name_width$}  {:<desc_width$}  {:<url_width$}",
        "-".repeat(name_width),
        "-".repeat(desc_width),
        "-".repeat(url_width)
    )?;

    for (name, desc, url) in &displayed {
        writeln!(
            writer,
            "{:<name_width$}  {:<desc_width$}  {}",
            name, desc, url
        )?;
    }

    Ok(())
}

#[derive(Deserialize)]
struct GitHubSearchResponse {
    items: Vec<GitHubRepo>,
}

#[derive(Deserialize)]
struct GitHubRepo {
    name: String,
    description: Option<String>,
    html_url: String,
}

pub struct RemotePlugin {
    pub name: String,
    pub description: String,
    pub url: String,
}

fn fetch_remote_plugins() -> Result<Vec<RemotePlugin>, Box<dyn std::error::Error>> {
    let response: GitHubSearchResponse = ureq::get(
        "https://api.github.com/search/repositories?q=homeos-plugin-+in:name+org:homeos-dev",
    )
    .header("User-Agent", "homeos")
    .call()
    .map_err(|e| HomeosError::new(reasons::NETWORK_ERROR, e.to_string()))?
    .body_mut()
    .read_json()
    .map_err(|e| HomeosError::new(reasons::NETWORK_ERROR, e.to_string()))?;

    let plugins = response
        .items
        .into_iter()
        .filter(|r| r.name.starts_with("homeos-plugin-"))
        .map(|r| {
            let name = r.name.strip_prefix("homeos-plugin-").unwrap().to_string();
            RemotePlugin {
                name,
                description: r.description.unwrap_or_default(),
                url: r.html_url,
            }
        })
        .collect();

    Ok(plugins)
}

pub fn list_remote(ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
    list_remote_to(
        ctx.output_format(),
        &mut std::io::stdout(),
        fetch_remote_plugins,
    )
}

fn list_remote_to<W: Write, F>(
    format: OutputFormat,
    writer: &mut W,
    fetch: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce() -> Result<Vec<RemotePlugin>, Box<dyn std::error::Error>>,
{
    let mut plugins = fetch()?;
    plugins.sort_by(|a, b| a.name.cmp(&b.name));

    match format {
        OutputFormat::Json => list_remote_json(writer, &plugins),
        OutputFormat::Text => list_remote_text(writer, &plugins),
    }
}

fn list_remote_json<W: Write>(
    writer: &mut W,
    plugins: &[RemotePlugin],
) -> Result<(), Box<dyn std::error::Error>> {
    let rows: Vec<serde_json::Value> = plugins
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "description": p.description,
                "url": p.url,
            })
        })
        .collect();
    writeln!(writer, "{}", serde_json::Value::Array(rows))?;
    Ok(())
}

fn list_remote_text<W: Write>(
    writer: &mut W,
    plugins: &[RemotePlugin],
) -> Result<(), Box<dyn std::error::Error>> {
    if plugins.is_empty() {
        writeln!(writer, "No remote plugins found.")?;
        return Ok(());
    }

    let name_width = plugins
        .iter()
        .map(|p| p.name.len())
        .max()
        .unwrap_or(0)
        .max(4); // "Name" header length

    let desc_width = plugins
        .iter()
        .map(|p| p.description.len())
        .max()
        .unwrap_or(0)
        .max(11); // "Description" header length

    let url_width = plugins
        .iter()
        .map(|p| p.url.len())
        .max()
        .unwrap_or(0)
        .max(3); // "URL" header length

    writeln!(
        writer,
        "{:<name_width$}  {:<desc_width$}  {:<url_width$}",
        "Name", "Description", "URL"
    )?;
    writeln!(
        writer,
        "{:<name_width$}  {:<desc_width$}  {:<url_width$}",
        "-".repeat(name_width),
        "-".repeat(desc_width),
        "-".repeat(url_width)
    )?;

    for plugin in plugins {
        writeln!(
            writer,
            "{:<name_width$}  {:<desc_width$}  {}",
            plugin.name, plugin.description, plugin.url
        )?;
    }

    Ok(())
}

fn check_repo_exists(plugin: &str) -> Result<(), Box<dyn std::error::Error>> {
    let api_url = format!(
        "https://api.github.com/repos/homeos-dev/homeos-plugin-{}",
        plugin
    );
    match ureq::get(&api_url).header("User-Agent", "homeos").call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(404)) => Err(HomeosError::new(
            reasons::NOT_FOUND_ON_GITHUB,
            format!(
                "Plugin '{}' not found on GitHub (homeos-plugin-{})",
                plugin, plugin
            ),
        )
        .into()),
        Err(e) => Err(HomeosError::new(reasons::NETWORK_ERROR, e.to_string()).into()),
    }
}

/// The clone URL `plugin add <name>` resolves to when no explicit URL is given.
fn default_plugin_url(plugin: &str) -> String {
    format!("https://github.com/homeos-dev/homeos-plugin-{}", plugin)
}

pub fn add(
    ctx: &Context,
    plugin: &str,
    url: Option<&str>,
    local: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    add_with(ctx, plugin, url, local, check_repo_exists)
}

fn add_local(ctx: &Context, plugin: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(&ctx.config_path())?;

    if config.plugins.contains_key(plugin) {
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Plugin '{}' already exists", plugin),
        )
        .into());
    }

    let plugins_dir = ctx.plugins_dir();
    let target = plugins_dir.join(plugin);

    if target.exists() {
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Plugin directory '{}' already exists", plugin),
        )
        .into());
    }

    std::fs::create_dir_all(&target)?;

    // Create plugin.yml
    std::fs::write(
        target.join("plugin.yml"),
        "description: Brief description of what this plugin does.\nparams: []\n",
    )?;

    // Create template files for all OS (both .sh and .ps1)
    for action in &["install", "update", "uninstall"] {
        for ext in &["sh", "ps1"] {
            let template_name = format!("{}.{}.tmpl", action, ext);
            let content = format!(
                "# Generated by homeos. Edit this template for {} action.\n",
                action
            );
            std::fs::write(target.join(&template_name), content)?;
        }
    }

    let mut config = config;
    config
        .plugins
        .insert(plugin.to_string(), PluginConfig { url: None });
    config.save(&ctx.config_path())?;

    println!("Plugin '{}' created locally", plugin);
    Ok(())
}

fn add_with<F>(
    ctx: &Context,
    plugin: &str,
    url: Option<&str>,
    local: bool,
    repo_checker: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&str) -> Result<(), Box<dyn std::error::Error>>,
{
    if local {
        return add_local(ctx, plugin);
    }

    let config = Config::load(&ctx.config_path())?;

    if config.plugins.contains_key(plugin) {
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Plugin '{}' already exists", plugin),
        )
        .into());
    }

    let auto_resolved = url.is_none();
    let url = url
        .map(|u| u.to_string())
        .unwrap_or_else(|| default_plugin_url(plugin));

    let plugins_dir = ctx.plugins_dir();
    let target = plugins_dir.join(plugin);

    if target.exists() {
        return Err(HomeosError::new(
            reasons::ALREADY_EXISTS,
            format!("Plugin directory '{}' already exists", plugin),
        )
        .into());
    }

    if auto_resolved {
        repo_checker(plugin)?;
    }

    std::fs::create_dir_all(&plugins_dir)?;

    git::clone(&url, &target)?;

    if !target.join("plugin.yml").exists() {
        std::fs::remove_dir_all(&target)?;
        return Err(HomeosError::new(
            reasons::NOT_A_VALID_HOMEOS_PLUGIN,
            "Not a valid homeos plugin. Cloned directory removed.",
        )
        .into());
    }

    let git_dir = target.join(".git");
    if git_dir.exists() {
        std::fs::remove_dir_all(&git_dir)?;
    }

    let mut config = config;
    config.plugins.insert(
        plugin.to_string(),
        PluginConfig {
            url: Some(url.clone()),
        },
    );
    config.save(&ctx.config_path())?;

    println!("Plugin '{}' added successfully", plugin);
    Ok(())
}

pub fn remove(ctx: &Context, plugin: &str, purge: bool) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stdout();
    remove_to(ctx, plugin, purge, &mut reader, &mut writer)
}

fn remove_to<R: BufRead, W: Write>(
    ctx: &Context,
    plugin: &str,
    purge: bool,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load(&ctx.config_path())?;

    if !config.plugins.contains_key(plugin) {
        return Err(HomeosError::new(
            reasons::PLUGIN_NOT_FOUND,
            format!("Plugin '{}' not found", plugin),
        )
        .into());
    }

    // Warn if packages reference this plugin
    let referencing: Vec<&String> = config
        .packages
        .iter()
        .filter(|(_, pkg)| pkg.plugin.as_deref() == Some(plugin))
        .map(|(pkg_name, _)| pkg_name)
        .collect();

    if !referencing.is_empty() {
        let names = referencing
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            writer,
            "Warning: the following packages reference plugin '{}': {}",
            plugin, names
        )?;
    }

    writeln!(
        writer,
        "The following plugins will be removed from homeos.yml:"
    )?;
    writeln!(writer, "  {plugin}")?;

    if purge {
        let plugin_dir = ctx.plugins_dir().join(plugin);
        if plugin_dir.exists() {
            writeln!(writer, "\nThe following directories will be deleted:")?;
            writeln!(writer, "  {}", plugin_dir.display())?;
        }
    }

    if !ctx.yes() && !prompt_confirm(reader, writer) {
        writeln!(writer, "Aborted.")?;
        return Ok(());
    }

    config.plugins.remove(plugin);
    if purge {
        let plugin_dir = ctx.plugins_dir().join(plugin);
        if plugin_dir.exists() {
            std::fs::remove_dir_all(&plugin_dir)?;
            writeln!(writer, "Removed plugin '{}' and removed directory", plugin)?;
        } else {
            writeln!(writer, "Removed plugin '{}'", plugin)?;
        }
    } else {
        writeln!(writer, "Removed plugin '{}'", plugin)?;
    }
    config.save(&ctx.config_path())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PluginConfig};
    use std::io::Cursor;
    use std::process::Command;
    use tempfile::TempDir;

    fn fixture(base_dir: &TempDir) -> Context {
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));
        std::fs::create_dir_all(ctx.data_dir()).unwrap();
        ctx
    }

    fn fixture_with_config(base_dir: &TempDir) -> Context {
        let ctx = fixture(base_dir);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        ctx
    }

    fn create_local_git_repo(dir: &std::path::Path) {
        std::fs::create_dir_all(dir).unwrap();
        Command::new("git")
            .args(["init", &dir.to_string_lossy()])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ])
            .output()
            .unwrap();
    }

    fn create_local_plugin_repo(dir: &std::path::Path) {
        create_local_git_repo(dir);
        std::fs::write(
            dir.join("plugin.yml"),
            "description: Test plugin\nparams: []\n",
        )
        .unwrap();
        Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "add", "plugin.yml"])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                &dir.to_string_lossy(),
                "commit",
                "-m",
                "add plugin.yml",
            ])
            .output()
            .unwrap();
    }

    #[test]
    fn test_list_no_plugins() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Name"));
        assert!(text.contains("URL"));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator only
    }

    #[test]
    fn test_list_single_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-mise".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("mise"));
        assert!(text.contains("https://github.com/homeos-dev/homeos-plugin-mise"));
    }

    #[test]
    fn test_list_multiple_plugins_sorted() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "rustup".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-rustup".to_string()),
            },
        );
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-mise".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Header + separator + 2 plugins
        assert_eq!(lines.len(), 4);
        // BTreeMap sorts alphabetically: mise before rustup
        assert!(lines[2].starts_with("mise"));
        assert!(lines[3].starts_with("rustup"));
    }

    #[test]
    fn test_list_table_header_format() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("Name"));
        assert!(lines[0].contains("Description"));
        assert!(lines[0].contains("URL"));
        assert!(lines[1].starts_with("----"));
        assert!(lines[1].contains("---"));
    }

    #[test]
    fn test_list_shows_description_from_plugin_yml() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "description: DNF package manager plugin for homeos.\nparams:\n  - name\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("DNF package manager plugin for homeos."));
    }

    #[test]
    fn test_list_description_empty_when_plugin_yml_missing() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert — header + separator + plugin row; description column is blank
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[2].starts_with("dnf"));
        assert!(lines[2].contains("https://example.com"));
    }

    #[test]
    fn test_list_name_column_width_adjusts() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "long-plugin-name".to_string(),
            PluginConfig {
                url: Some("https://example.com".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Separator dashes should be at least as long as "long-plugin-name" (16 chars)
        assert!(lines[1].starts_with(&"-".repeat(16)));
    }

    #[test]
    fn test_list_url_column_separator_matches_widest_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert — URL separator should match the widest URL value, not the header width
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let url_len = "https://github.com/homeos-dev/homeos-plugin-dnf".len();
        let separator_dashes_at_url_position = lines[1]
            .rsplit("  ")
            .next()
            .expect("separator line should have a URL segment");
        assert_eq!(separator_dashes_at_url_position, "-".repeat(url_len));
    }

    #[test]
    fn test_list_renders_local_marker_when_url_is_none() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir);
        let mut config = Config::default();
        config
            .plugins
            .insert("custom".to_string(), PluginConfig { url: None });
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("custom"));
        assert!(text.contains("(local)"));
    }

    #[test]
    fn test_list_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));
        let mut output = Vec::new();

        // Act
        let result = list_to(&ctx, &mut output);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_list_remote_no_plugins() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || Ok(vec![]);

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text, "No remote plugins found.\n");
    }

    #[test]
    fn test_list_remote_single_plugin() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "mise".to_string(),
                description: "Manage mise tools".to_string(),
                url: "https://github.com/homeos-dev/homeos-plugin-mise".to_string(),
            }])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("mise"));
        assert!(text.contains("Manage mise tools"));
        assert!(text.contains("https://github.com/homeos-dev/homeos-plugin-mise"));
    }

    #[test]
    fn test_list_remote_multiple_plugins() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![
                RemotePlugin {
                    name: "mise".to_string(),
                    description: "Manage mise tools".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-mise".to_string(),
                },
                RemotePlugin {
                    name: "rustup".to_string(),
                    description: "Manage Rust toolchains".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-rustup".to_string(),
                },
            ])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Header + separator + 2 plugins
        assert_eq!(lines.len(), 4);
        assert!(lines[2].contains("mise"));
        assert!(lines[3].contains("rustup"));
    }

    #[test]
    fn test_list_remote_sorts_alphabetically_by_name() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![
                RemotePlugin {
                    name: "winget".to_string(),
                    description: "WinGet plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-winget".to_string(),
                },
                RemotePlugin {
                    name: "dnf".to_string(),
                    description: "DNF plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-dnf".to_string(),
                },
                RemotePlugin {
                    name: "npm".to_string(),
                    description: "npm plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-npm".to_string(),
                },
            ])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Header + separator + 3 plugins, sorted alphabetically: dnf, npm, winget
        assert_eq!(lines.len(), 5);
        assert!(lines[2].starts_with("dnf"));
        assert!(lines[3].starts_with("npm"));
        assert!(lines[4].starts_with("winget"));
    }

    #[test]
    fn test_list_remote_table_header_format() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "mise".to_string(),
                description: "A tool manager".to_string(),
                url: "https://example.com".to_string(),
            }])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("Name"));
        assert!(lines[0].contains("Description"));
        assert!(lines[0].contains("URL"));
        assert!(lines[1].starts_with("----"));
        assert!(lines[1].contains("---"));
    }

    #[test]
    fn test_list_remote_name_column_width_adjusts() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "long-plugin-name".to_string(),
                description: "A plugin".to_string(),
                url: "https://example.com".to_string(),
            }])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Separator dashes should be at least as long as "long-plugin-name" (16 chars)
        assert!(lines[1].starts_with(&"-".repeat(16)));
    }

    #[test]
    fn test_list_remote_url_column_separator_matches_widest_url() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![
                RemotePlugin {
                    name: "homebrew".to_string(),
                    description: "Homebrew package manager plugin for homeos.".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-homebrew".to_string(),
                },
                RemotePlugin {
                    name: "dnf".to_string(),
                    description: "DNF package manager plugin for homeos.".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-dnf".to_string(),
                },
            ])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert — URL separator should match the widest URL value, not the header width
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let widest_url_len = "https://github.com/homeos-dev/homeos-plugin-homebrew".len();
        let separator_dashes_at_url_position = lines[1]
            .rsplit("  ")
            .next()
            .expect("separator line should have a URL segment");
        assert_eq!(separator_dashes_at_url_position, "-".repeat(widest_url_len));
    }

    #[test]
    fn test_list_remote_empty_description() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![RemotePlugin {
                name: "mise".to_string(),
                description: "".to_string(),
                url: "https://example.com".to_string(),
            }])
        };

        // Act
        list_remote_to(OutputFormat::Text, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("mise"));
        assert!(text.contains("https://example.com"));
    }

    #[test]
    fn test_list_remote_fetch_error() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || Err("Network error".into());

        // Act
        let result = list_remote_to(OutputFormat::Text, &mut output, fetch);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Network error"));
    }

    #[test]
    fn test_add_clones_and_registers_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act
        let result = add(
            &ctx,
            "dnf",
            Some(&source_dir.path().to_string_lossy()),
            false,
        );

        // Assert
        assert!(result.is_ok());
        assert!(ctx.plugins_dir().join("dnf").exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("dnf"));
        assert_eq!(
            config.plugins["dnf"].url.as_deref(),
            Some(source_dir.path().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn test_add_removes_git_directory_after_clone() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act
        add(
            &ctx,
            "dnf",
            Some(&source_dir.path().to_string_lossy()),
            false,
        )
        .unwrap();

        // Assert
        let plugin_dir = ctx.plugins_dir().join("dnf");
        assert!(plugin_dir.exists());
        assert!(!plugin_dir.join(".git").exists());
    }

    #[test]
    fn test_add_default_url_without_explicit_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        // Create a local plugin repo to simulate the default URL clone target
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act — use explicit URL since we can't clone from GitHub in tests
        let result = add(
            &ctx,
            "mise",
            Some(&source_dir.path().to_string_lossy()),
            false,
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("mise"));
    }

    #[test]
    fn test_add_rejects_repo_without_plugin_yml() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let result = add(
            &ctx,
            "bad",
            Some(&source_dir.path().to_string_lossy()),
            false,
        );

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Not a valid homeos plugin. Cloned directory removed."
        );
    }

    #[test]
    fn test_add_rejects_repo_without_plugin_yml_cleans_up() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_git_repo(source_dir.path());

        // Act
        let _ = add(
            &ctx,
            "bad",
            Some(&source_dir.path().to_string_lossy()),
            false,
        );

        // Assert — cloned directory should be removed
        assert!(!ctx.plugins_dir().join("bad").exists());
        // Config should not be modified
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("bad"));
    }

    #[test]
    fn test_add_already_registered() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        let result = add(&ctx, "dnf", Some("https://example.com/repo.git"), false);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin 'dnf' already exists");
    }

    #[test]
    fn test_add_directory_already_exists() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();

        // Act
        let result = add(&ctx, "dnf", Some("https://example.com/repo.git"), false);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin directory 'dnf' already exists");
    }

    #[test]
    fn test_add_invalid_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = add(&ctx, "bad-plugin", Some("not-a-valid-url"), false);

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.starts_with("git clone failed:"));
    }

    #[test]
    fn test_add_creates_plugins_dir() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());
        assert!(!ctx.plugins_dir().exists());

        // Act
        let result = add(
            &ctx,
            "dnf",
            Some(&source_dir.path().to_string_lossy()),
            false,
        );

        // Assert
        assert!(result.is_ok());
        assert!(ctx.plugins_dir().exists());
        assert!(ctx.plugins_dir().join("dnf").exists());
    }

    #[test]
    fn test_add_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));

        // Act
        let result = add(&ctx, "dnf", Some("https://example.com/repo.git"), false);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_default_plugin_url_points_at_the_homeos_dev_org() {
        // Arrange
        let plugin = "dnf";

        // Act
        let url = default_plugin_url(plugin);

        // Assert
        assert_eq!(url, "https://github.com/homeos-dev/homeos-plugin-dnf");
    }

    #[test]
    fn test_add_resolves_default_url() {
        // Arrange — use add_with to inject a checker that records it was called
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act — no URL provided, repo_checker should be called
        // We use add_with directly to inject a fake checker that always succeeds,
        // but we can't actually clone from the default GitHub URL, so we test
        // via the error path: the checker rejects the plugin.
        let result = add_with(&ctx, "nonexistent-plugin-xyz", None, false, |name| {
            Err(format!(
                "Plugin '{}' not found on GitHub (homeos-plugin-{})",
                name, name
            )
            .into())
        });

        // Assert — should fail with the checker's error, not git clone
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found on GitHub"));
        assert!(err.contains("nonexistent-plugin-xyz"));
    }

    #[test]
    fn test_add_auto_resolved_url_checks_repo_exists() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act — no URL, checker returns error
        let result = add_with(&ctx, "missing", None, false, |_name| {
            Err("Plugin 'missing' not found on GitHub (homeos-plugin-missing)".into())
        });

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(
            err,
            "Plugin 'missing' not found on GitHub (homeos-plugin-missing)"
        );
    }

    #[test]
    fn test_add_auto_resolved_url_skips_check_with_explicit_url() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let source_dir = TempDir::new().unwrap();
        create_local_plugin_repo(source_dir.path());

        // Act — explicit URL provided, checker should NOT be called
        let result = add_with(
            &ctx,
            "dnf",
            Some(&source_dir.path().to_string_lossy()),
            false,
            |_name| {
                panic!("repo_checker should not be called when URL is explicit");
            },
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_add_auto_resolved_url_no_clone_on_check_failure() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act — checker fails, clone should not be attempted
        let _ = add_with(&ctx, "bad-plugin", None, false, |_name| {
            Err("not found".into())
        });

        // Assert — plugins directory should not have been created
        assert!(!ctx.plugins_dir().join("bad-plugin").exists());
    }

    #[test]
    fn test_add_local_creates_skeleton() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = add(&ctx, "custom", None, true);

        // Assert
        assert!(result.is_ok());
        let plugin_dir = ctx.plugins_dir().join("custom");
        assert!(plugin_dir.exists());
        assert!(plugin_dir.join("plugin.yml").exists());
        for ext in &["sh", "ps1"] {
            assert!(plugin_dir.join(format!("install.{}.tmpl", ext)).exists());
            assert!(plugin_dir.join(format!("update.{}.tmpl", ext)).exists());
            assert!(plugin_dir.join(format!("uninstall.{}.tmpl", ext)).exists());
        }
    }

    #[test]
    fn test_add_local_registers_in_config() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        add(&ctx, "custom", None, true).unwrap();

        // Assert
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("custom"));
        assert_eq!(config.plugins["custom"].url, None);
    }

    #[test]
    fn test_add_local_omits_url_in_serialized_homeos_yml() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        add(&ctx, "custom", None, true).unwrap();

        // Assert — no `url` field should appear in homeos.yml for --local plugins
        let yaml = std::fs::read_to_string(ctx.config_path()).unwrap();
        assert!(yaml.contains("custom"));
        assert!(!yaml.contains("url"));
    }

    #[test]
    fn test_add_local_plugin_yml_content() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        add(&ctx, "custom", None, true).unwrap();

        // Assert
        let content = std::fs::read_to_string(ctx.plugins_dir().join("custom/plugin.yml")).unwrap();
        assert_eq!(
            content,
            "description: Brief description of what this plugin does.\nparams: []\n"
        );
    }

    #[test]
    fn test_add_local_template_content() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        add(&ctx, "custom", None, true).unwrap();

        // Assert
        for ext in &["sh", "ps1"] {
            let content = std::fs::read_to_string(
                ctx.plugins_dir()
                    .join(format!("custom/install.{}.tmpl", ext)),
            )
            .unwrap();
            assert!(content.contains("install"));
            assert!(content.contains("Generated by homeos"));
        }
    }

    #[test]
    fn test_add_local_rejects_existing_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        add(&ctx, "custom", None, true).unwrap();

        // Act
        let result = add(&ctx, "custom", None, true);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Plugin 'custom' already exists"
        );
    }

    #[test]
    fn test_add_local_rejects_existing_directory() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        std::fs::create_dir_all(ctx.plugins_dir().join("custom")).unwrap();

        // Act
        let result = add(&ctx, "custom", None, true);

        // Assert
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Plugin directory 'custom' already exists"
        );
    }

    #[test]
    fn test_add_local_ignores_url_argument() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act — URL is provided but --local takes precedence, no clone happens
        let result = add(&ctx, "custom", Some("https://example.com/repo.git"), true);

        // Assert
        assert!(result.is_ok());
        let plugin_dir = ctx.plugins_dir().join("custom");
        assert!(plugin_dir.join("plugin.yml").exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert_eq!(config.plugins["custom"].url, None);
    }

    #[test]
    fn test_remove_keeps_directory_and_removes_config_entry() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        // Act
        let result = remove_to(
            &ctx,
            "dnf",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        assert!(result.is_ok());
        assert!(plugin_dir.exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_plugin_not_found() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);

        // Act
        let result = remove_to(
            &ctx,
            "nonexistent",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Plugin 'nonexistent' not found");
    }

    #[test]
    fn test_remove_without_directory() {
        // Arrange — plugin registered in config but directory is missing
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        let result = remove_to(
            &ctx,
            "dnf",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_warns_when_packages_reference_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.packages.insert(
            "neovim".to_string(),
            crate::config::PackageConfig {
                plugin: Some("dnf".to_string()),
                ..Default::default()
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act — should succeed (warn but not block)
        let result = remove_to(
            &ctx,
            "dnf",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        assert!(result.is_ok());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_does_not_affect_other_plugins() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-mise".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("mise")).unwrap();

        // Act
        let result = remove_to(
            &ctx,
            "dnf",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        assert!(result.is_ok());
        assert!(ctx.plugins_dir().join("dnf").exists());
        assert!(ctx.plugins_dir().join("mise").exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
        assert!(config.plugins.contains_key("mise"));
    }

    #[test]
    fn test_remove_error_when_not_initialized() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = Context::new(Some(base_dir.path().to_path_buf()));

        // Act
        let result = remove_to(
            &ctx,
            "dnf",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_purge_deletes_plugin_directory() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.yml"), "params: {}").unwrap();

        // Act
        let mut output = Vec::new();
        let result = remove_to(&ctx, "dnf", true, &mut Cursor::new(b"y\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(!plugin_dir.exists());
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Removed plugin 'dnf' and removed directory"));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_purge_succeeds_when_directory_does_not_exist() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();

        // Act
        let mut output = Vec::new();
        let result = remove_to(&ctx, "dnf", true, &mut Cursor::new(b"y\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Removed plugin 'dnf'"));
        assert!(!output_str.contains("removed directory"));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_without_purge_preserves_plugin_directory() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.yml"), "params: {}").unwrap();

        // Act
        let result = remove_to(
            &ctx,
            "dnf",
            false,
            &mut Cursor::new(b"y\n"),
            &mut Vec::new(),
        );

        // Assert
        assert!(result.is_ok());
        assert!(plugin_dir.exists());
        assert!(plugin_dir.join("plugin.yml").exists());
    }

    #[test]
    fn test_remove_purge_does_not_affect_other_plugins() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-mise".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("dnf")).unwrap();
        std::fs::create_dir_all(ctx.plugins_dir().join("mise")).unwrap();

        // Act
        let result = remove_to(&ctx, "dnf", true, &mut Cursor::new(b"y\n"), &mut Vec::new());

        // Assert
        assert!(result.is_ok());
        assert!(!ctx.plugins_dir().join("dnf").exists());
        assert!(ctx.plugins_dir().join("mise").exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
        assert!(config.plugins.contains_key("mise"));
    }

    #[test]
    fn test_remove_prompt_shows_plugin_name() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", false, &mut Cursor::new(b"y\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("The following plugins will be removed from homeos.yml:"));
        assert!(output.contains("  dnf"));
    }

    #[test]
    fn test_remove_declined_aborts() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", false, &mut Cursor::new(b"n\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Aborted."));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_purge_prompt_shows_directory() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", true, &mut Cursor::new(b"y\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("The following directories will be deleted:"));
        assert!(output.contains(&plugin_dir.display().to_string()));
    }

    #[test]
    fn test_remove_purge_no_directory_section_when_dir_missing() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", true, &mut Cursor::new(b"y\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("The following directories will be deleted:"));
    }

    #[test]
    fn test_remove_yes_skips_prompt_and_removes() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir).with_yes(true);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut reader = Cursor::new(b"");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", false, &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(written.contains("The following plugins will be removed from homeos.yml:"));
        assert!(!written.contains("Proceed? [y/N]"));
        assert!(!written.contains("Aborted."));
        assert!(written.contains("Removed plugin 'dnf'"));
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(!config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_remove_yes_with_purge_skips_prompt_and_deletes_directory() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir).with_yes(true);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let mut reader = Cursor::new(b"");
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", true, &mut reader, &mut output);

        // Assert
        assert!(result.is_ok());
        let written = String::from_utf8(output).unwrap();
        assert!(!written.contains("Proceed? [y/N]"));
        assert!(written.contains("Removed plugin 'dnf' and removed directory"));
        assert!(!plugin_dir.exists());
    }

    #[test]
    fn test_remove_purge_declined_preserves_directory() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture_with_config(&base_dir);
        let mut config = Config::load(&ctx.config_path()).unwrap();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let mut output = Vec::new();

        // Act
        let result = remove_to(&ctx, "dnf", true, &mut Cursor::new(b"n\n"), &mut output);

        // Assert
        assert!(result.is_ok());
        assert!(plugin_dir.exists());
        let config = Config::load(&ctx.config_path()).unwrap();
        assert!(config.plugins.contains_key("dnf"));
    }

    #[test]
    fn test_list_json_emits_array_of_objects() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir).with_output_format(OutputFormat::Json);
        let mut config = Config::default();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.plugins.insert(
            "mise".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-mise".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().expect("expected JSON array");
        assert_eq!(array.len(), 2);
        assert_eq!(array[0]["name"], "dnf");
        assert_eq!(
            array[0]["url"],
            "https://github.com/homeos-dev/homeos-plugin-dnf"
        );
        assert_eq!(array[1]["name"], "mise");
    }

    #[test]
    fn test_list_json_emits_empty_array_when_no_plugins() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir).with_output_format(OutputFormat::Json);
        let config = Config::default();
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().expect("expected JSON array");
        assert!(array.is_empty());
    }

    #[test]
    fn test_list_json_url_is_null_for_local_plugin() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir).with_output_format(OutputFormat::Json);
        let mut config = Config::default();
        config
            .plugins
            .insert("custom".to_string(), PluginConfig { url: None });
        config.save(&ctx.config_path()).unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().unwrap();
        assert_eq!(array[0]["name"], "custom");
        assert!(array[0]["url"].is_null());
    }

    #[test]
    fn test_list_json_description_loaded_from_plugin_yml() {
        // Arrange
        let base_dir = TempDir::new().unwrap();
        let ctx = fixture(&base_dir).with_output_format(OutputFormat::Json);
        let mut config = Config::default();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        config.save(&ctx.config_path()).unwrap();
        let plugin_dir = ctx.plugins_dir().join("dnf");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.yml"),
            "description: DNF package manager plugin for homeos.\nparams:\n  - name\n",
        )
        .unwrap();
        let mut output = Vec::new();

        // Act
        list_to(&ctx, &mut output).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().unwrap();
        assert_eq!(
            array[0]["description"],
            "DNF package manager plugin for homeos."
        );
    }

    #[test]
    fn test_list_remote_json_emits_array_of_objects() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![
                RemotePlugin {
                    name: "dnf".to_string(),
                    description: "DNF plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-dnf".to_string(),
                },
                RemotePlugin {
                    name: "mise".to_string(),
                    description: "Mise plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-mise".to_string(),
                },
            ])
        };

        // Act
        list_remote_to(OutputFormat::Json, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().expect("expected JSON array");
        assert_eq!(array.len(), 2);
        assert_eq!(array[0]["name"], "dnf");
        assert_eq!(array[0]["description"], "DNF plugin");
        assert_eq!(
            array[0]["url"],
            "https://github.com/homeos-dev/homeos-plugin-dnf"
        );
        assert_eq!(array[1]["name"], "mise");
    }

    #[test]
    fn test_list_remote_json_emits_empty_array_when_no_plugins() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || Ok(vec![]);

        // Act
        list_remote_to(OutputFormat::Json, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().expect("expected JSON array");
        assert!(array.is_empty());
    }

    #[test]
    fn test_list_remote_json_sorts_alphabetically() {
        // Arrange
        let mut output = Vec::new();
        let fetch = || {
            Ok(vec![
                RemotePlugin {
                    name: "winget".to_string(),
                    description: "WinGet plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-winget".to_string(),
                },
                RemotePlugin {
                    name: "dnf".to_string(),
                    description: "DNF plugin".to_string(),
                    url: "https://github.com/homeos-dev/homeos-plugin-dnf".to_string(),
                },
            ])
        };

        // Act
        list_remote_to(OutputFormat::Json, &mut output, fetch).unwrap();

        // Assert
        let text = String::from_utf8(output).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let array = value.as_array().unwrap();
        assert_eq!(array[0]["name"], "dnf");
        assert_eq!(array[1]["name"], "winget");
    }
}
