//! Application configuration structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub files: HashMap<String, FilesConfig>,
    #[serde(default)]
    pub snippets: HashMap<String, Vec<Snippet>>,
    #[serde(default)]
    pub template_paths: TemplatePathsConfig,
}

/// UI configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_language")]
    pub language: String,
}

/// Files configuration for each shell type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesConfig {
    pub paths: Vec<String>,
}

/// A snippet template for new entry creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub template: Option<String>,
}

/// External template file paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePathsConfig {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Default for TemplatePathsConfig {
    fn default() -> Self {
        Self { paths: Vec::new() }
    }
}

fn default_language() -> String {
    "en".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig {
                language: default_language(),
            },
            files: HashMap::new(),
            snippets: HashMap::new(),
            template_paths: TemplatePathsConfig::default(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
        }
    }
}

impl Config {
    /// Get the wenv configuration directory path
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".config")
            .join("wenv")
    }

    /// Get the configuration file path
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load configuration from file, or return default if file doesn't exist
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            toml::from_str(&content)?
        } else {
            Config::default()
        };

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.ui.language, "en");
        assert!(config.files.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.ui.language, config.ui.language);
    }

    #[test]
    fn test_snippet_serialization() {
        let snippet = Snippet {
            name: "alias".into(),
            description: "Define an alias".into(),
            template: Some("alias NAME='VALUE'".into()),
        };
        let toml_str = toml::to_string(&snippet).unwrap();
        let parsed: Snippet = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.name, "alias");
        assert_eq!(parsed.template, Some("alias NAME='VALUE'".into()));
    }

    #[test]
    fn test_config_with_snippets() {
        let mut config = Config::default();
        let snippets = vec![
            Snippet {
                name: "Empty".into(),
                description: "Blank entry".into(),
                template: None,
            },
            Snippet {
                name: "alias".into(),
                description: "Define an alias".into(),
                template: Some("alias NAME='VALUE'".into()),
            },
        ];
        config.snippets.insert("zsh".into(), snippets);
        config.template_paths = TemplatePathsConfig {
            paths: vec!["~/.config/wenv/snippets/extra.toml".into()],
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("[snippets.zsh]"));
        assert!(toml_str.contains("[template_paths]"));

        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.snippets["zsh"].len(), 2);
        assert!(parsed.snippets["zsh"][0].template.is_none());
        assert_eq!(parsed.template_paths.paths.len(), 1);
    }

    #[test]
    fn test_config_without_snippets_parses() {
        // Existing config without snippets section should parse fine
        let toml_str = "[ui]\nlanguage = \"en\"\n";
        let parsed: Config = toml::from_str(toml_str).unwrap();
        assert!(parsed.snippets.is_empty());
        assert!(parsed.template_paths.paths.is_empty());
    }
}
