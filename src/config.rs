use crate::error::{HomeosError, reasons};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub packages: BTreeMap<String, PackageConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<String, PluginConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PluginConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PluginManifest {
    pub description: String,
    #[serde(default)]
    pub params: Vec<String>,
}

impl PluginManifest {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let manifest: PluginManifest = yaml_serde::from_str(&contents)?;
        Ok(manifest)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PackageConfig {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub script_aliases: BTreeMap<String, String>,
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            script_aliases: BTreeMap::new(),
            enabled: true,
            archived: false,
            depends_on: Vec::new(),
            plugin: None,
            params: BTreeMap::new(),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Err(HomeosError::new(
                reasons::NOT_INITIALIZED,
                format!(
                    "homeos.yml not found at {}. Run 'homeos init' first.",
                    path.display()
                ),
            )
            .into());
        }
        let contents = std::fs::read_to_string(path)?;
        let config: Config = yaml_serde::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let contents = yaml_serde::to_string(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn fixture_file(content: &str) -> NamedTempFile {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "{}", content).unwrap();
        tmp
    }

    #[test]
    fn test_package_config_default_enabled_is_true() {
        // Arrange / Act
        let sut = PackageConfig::default();

        // Assert
        assert!(sut.enabled);
        assert!(!sut.archived);
        assert!(sut.script_aliases.is_empty());
        assert!(sut.depends_on.is_empty());
        assert_eq!(sut.plugin, None);
        assert!(sut.params.is_empty());
    }

    #[test]
    fn test_parse_archived() {
        // Arrange
        let yaml = "packages:\n  ollama:\n    archived: true\n  neovim: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.packages["ollama"].archived);
        assert!(!sut.packages["neovim"].archived);
    }

    #[test]
    fn test_archived_leaves_enabled_untouched() {
        // Arrange
        let yaml = "packages:\n  ollama:\n    enabled: false\n    archived: true\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.packages["ollama"].archived);
        assert!(!sut.packages["ollama"].enabled);
    }

    #[test]
    fn test_serialize_includes_archived_when_true() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "ollama".to_string(),
                PackageConfig {
                    archived: true,
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(sut.contains("archived"));
    }

    #[test]
    fn test_save_and_reload_with_archived() {
        // Arrange
        let mut config = Config::default();
        config.packages.insert(
            "ollama".to_string(),
            PackageConfig {
                enabled: false,
                archived: true,
                ..Default::default()
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert!(sut.packages["ollama"].archived);
        assert!(!sut.packages["ollama"].enabled);
    }

    #[test]
    fn test_parse_full_config() {
        // Arrange
        let yaml = r#"
packages:
  neovim:
    script_aliases:
      update: install
    enabled: false
  ripgrep:
    enabled: true
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.packages.len(), 2);
        let neovim = &sut.packages["neovim"];
        assert_eq!(neovim.script_aliases["update"], "install");
        assert!(!neovim.enabled);
        let ripgrep = &sut.packages["ripgrep"];
        assert!(ripgrep.script_aliases.is_empty());
        assert!(ripgrep.enabled);
    }

    #[test]
    fn test_parse_empty_packages() {
        // Arrange
        let yaml = "packages: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.packages.is_empty());
    }

    #[test]
    fn test_parse_minimal_package() {
        // Arrange
        let yaml = "packages:\n  git: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let git = &sut.packages["git"];
        assert!(git.enabled);
        assert!(git.script_aliases.is_empty());
    }

    #[test]
    fn test_defaults_on_missing_fields() {
        // Arrange
        let yaml = "packages:\n  fish: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let fish = &sut.packages["fish"];
        assert!(fish.enabled);
        assert!(fish.script_aliases.is_empty());
    }

    #[test]
    fn test_load_from_file() {
        // Arrange
        let tmp = fixture_file(
            "packages:\n  neovim:\n    script_aliases:\n      update: install\n    enabled: false\n",
        );

        // Act
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.packages.len(), 1);
        assert!(!sut.packages["neovim"].enabled);
    }

    #[test]
    fn test_save_and_reload() {
        // Arrange
        let mut config = Config::default();
        config.packages.insert(
            "starship".to_string(),
            PackageConfig {
                script_aliases: BTreeMap::from([("update".to_string(), "install".to_string())]),
                enabled: true,
                archived: false,
                depends_on: Vec::new(),
                plugin: None,
                params: BTreeMap::new(),
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(config, sut);
    }

    #[test]
    fn test_serialize_skips_defaults() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "git".to_string(),
                PackageConfig {
                    script_aliases: BTreeMap::new(),
                    enabled: true,
                    archived: false,
                    depends_on: Vec::new(),
                    plugin: None,
                    params: BTreeMap::new(),
                },
            )]),
            ..Default::default()
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(!sut.contains("script_aliases"));
        assert!(!sut.contains("enabled"));
        assert!(!sut.contains("archived"));
    }

    #[test]
    fn test_parse_depends_on() {
        // Arrange
        let yaml = r#"
packages:
  neovim:
    depends_on:
      - git
      - curl
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let neovim = &sut.packages["neovim"];
        assert_eq!(neovim.depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_depends_on_defaults_to_empty() {
        // Arrange
        let yaml = "packages:\n  git: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.packages["git"].depends_on.is_empty());
    }

    #[test]
    fn test_serialize_skips_empty_depends_on() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "git".to_string(),
                PackageConfig {
                    depends_on: Vec::new(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(!sut.contains("depends_on"));
    }

    #[test]
    fn test_serialize_includes_nonempty_depends_on() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "neovim".to_string(),
                PackageConfig {
                    depends_on: vec!["git".to_string(), "curl".to_string()],
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(sut.contains("depends_on"));
        assert!(sut.contains("git"));
        assert!(sut.contains("curl"));
    }

    #[test]
    fn test_save_and_reload_with_depends_on() {
        // Arrange
        let mut config = Config::default();
        config.packages.insert(
            "neovim".to_string(),
            PackageConfig {
                depends_on: vec!["git".to_string(), "curl".to_string()],
                ..Default::default()
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.packages["neovim"].depends_on, vec!["git", "curl"]);
    }

    #[test]
    fn test_load_nonexistent_file() {
        // Arrange
        let path = Path::new("/nonexistent/homeos.yml");

        // Act
        let result = Config::load(path);

        // Assert
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "homeos.yml not found at /nonexistent/homeos.yml. Run 'homeos init' first."
        );
    }

    #[test]
    fn test_parse_package_plugin() {
        // Arrange
        let yaml = r#"
packages:
  neovim:
    plugin: dnf
    params:
      name: neovim.x86_64
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        let neovim = &sut.packages["neovim"];
        assert_eq!(neovim.plugin, Some("dnf".to_string()));
        assert_eq!(neovim.params["name"], "neovim.x86_64");
    }

    #[test]
    fn test_package_plugin_defaults_to_none() {
        // Arrange
        let yaml = "packages:\n  git: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.packages["git"].plugin, None);
        assert!(sut.packages["git"].params.is_empty());
    }

    #[test]
    fn test_serialize_skips_none_plugin_and_empty_params() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "git".to_string(),
                PackageConfig {
                    plugin: None,
                    params: BTreeMap::new(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(!sut.contains("plugin"));
        assert!(!sut.contains("params"));
    }

    #[test]
    fn test_serialize_includes_plugin_and_params() {
        // Arrange
        let config = Config {
            packages: BTreeMap::from([(
                "neovim".to_string(),
                PackageConfig {
                    plugin: Some("dnf".to_string()),
                    params: BTreeMap::from([("name".to_string(), "neovim.x86_64".to_string())]),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(sut.contains("plugin"));
        assert!(sut.contains("dnf"));
        assert!(sut.contains("params"));
        assert!(sut.contains("neovim.x86_64"));
    }

    #[test]
    fn test_save_and_reload_with_plugin_and_params() {
        // Arrange
        let mut config = Config::default();
        config.packages.insert(
            "neovim".to_string(),
            PackageConfig {
                plugin: Some("dnf".to_string()),
                params: BTreeMap::from([("name".to_string(), "neovim.x86_64".to_string())]),
                ..Default::default()
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.packages["neovim"].plugin, Some("dnf".to_string()));
        assert_eq!(sut.packages["neovim"].params["name"], "neovim.x86_64");
    }

    #[test]
    fn test_parse_plugins() {
        // Arrange
        let yaml = r#"
packages: {}
plugins:
  dnf:
    url: https://github.com/homeos-dev/homeos-plugin-dnf
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.plugins.len(), 1);
        assert_eq!(
            sut.plugins["dnf"].url.as_deref(),
            Some("https://github.com/homeos-dev/homeos-plugin-dnf")
        );
    }

    #[test]
    fn test_plugins_defaults_to_empty() {
        // Arrange
        let yaml = "packages: {}\n";

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert!(sut.plugins.is_empty());
    }

    #[test]
    fn test_serialize_skips_empty_plugins() {
        // Arrange
        let config = Config {
            packages: BTreeMap::new(),
            plugins: BTreeMap::new(),
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(!sut.contains("plugins"));
    }

    #[test]
    fn test_serialize_includes_nonempty_plugins() {
        // Arrange
        let config = Config {
            packages: BTreeMap::new(),
            plugins: BTreeMap::from([(
                "dnf".to_string(),
                PluginConfig {
                    url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
                },
            )]),
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(sut.contains("plugins"));
        assert!(sut.contains("dnf"));
        assert!(sut.contains("https://github.com/homeos-dev/homeos-plugin-dnf"));
    }

    #[test]
    fn test_save_and_reload_with_plugins() {
        // Arrange
        let mut config = Config::default();
        config.plugins.insert(
            "dnf".to_string(),
            PluginConfig {
                url: Some("https://github.com/homeos-dev/homeos-plugin-dnf".to_string()),
            },
        );
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(
            sut.plugins["dnf"].url.as_deref(),
            Some("https://github.com/homeos-dev/homeos-plugin-dnf")
        );
    }

    #[test]
    fn test_plugin_config_default_url_is_none() {
        // Arrange / Act
        let sut = PluginConfig::default();

        // Assert
        assert_eq!(sut.url, None);
    }

    #[test]
    fn test_parse_plugin_without_url() {
        // Arrange
        let yaml = r#"
packages: {}
plugins:
  custom: {}
"#;

        // Act
        let sut: Config = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.plugins.len(), 1);
        assert_eq!(sut.plugins["custom"].url, None);
    }

    #[test]
    fn test_serialize_skips_none_url() {
        // Arrange
        let config = Config {
            packages: BTreeMap::new(),
            plugins: BTreeMap::from([("custom".to_string(), PluginConfig { url: None })]),
        };

        // Act
        let sut = yaml_serde::to_string(&config).unwrap();

        // Assert
        assert!(sut.contains("custom"));
        assert!(!sut.contains("url"));
    }

    #[test]
    fn test_save_and_reload_with_local_plugin() {
        // Arrange
        let mut config = Config::default();
        config
            .plugins
            .insert("custom".to_string(), PluginConfig { url: None });
        let tmp = NamedTempFile::new().unwrap();

        // Act
        config.save(tmp.path()).unwrap();
        let sut = Config::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.plugins["custom"].url, None);
    }

    #[test]
    fn test_parse_plugin_manifest() {
        // Arrange
        let yaml = "description: DNF plugin\nparams:\n  - name\n  - repo\n";

        // Act
        let sut: PluginManifest = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.description, "DNF plugin");
        assert_eq!(sut.params, vec!["name", "repo"]);
    }

    #[test]
    fn test_parse_plugin_manifest_empty_params() {
        // Arrange
        let yaml = "description: A plugin\nparams: []\n";

        // Act
        let sut: PluginManifest = yaml_serde::from_str(yaml).unwrap();

        // Assert
        assert_eq!(sut.description, "A plugin");
        assert!(sut.params.is_empty());
    }

    #[test]
    fn test_load_plugin_manifest_from_file() {
        // Arrange
        let tmp = fixture_file("description: DNF plugin\nparams:\n  - name\n");

        // Act
        let sut = PluginManifest::load(tmp.path()).unwrap();

        // Assert
        assert_eq!(sut.description, "DNF plugin");
        assert_eq!(sut.params, vec!["name"]);
    }

    #[test]
    fn test_load_plugin_manifest_nonexistent_file() {
        // Arrange
        let path = Path::new("/nonexistent/plugin.yml");

        // Act
        let result = PluginManifest::load(path);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_plugin_manifest_requires_description() {
        // Arrange
        let yaml = "params:\n  - name\n";

        // Act
        let result: Result<PluginManifest, _> = yaml_serde::from_str(yaml);

        // Assert
        assert!(result.is_err());
    }
}
