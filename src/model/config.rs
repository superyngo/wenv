//! Application configuration structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub format: FormatConfig,
    #[serde(default)]
    pub backup: BackupConfig,
}

/// UI configuration options
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "en".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig {
            language: default_language(),
        }
    }
}

/// Format configuration options
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FormatConfig {
    pub indent: usize,
    pub group_by_type: bool,
    pub sort_alphabetically: bool,
    pub blank_lines_between_groups: usize,
    pub order: TypeOrder,
}

/// Type ordering for formatted output
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TypeOrder {
    pub types: Vec<String>,
}

/// Backup configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupConfig {
    pub max_count: usize,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig {
            indent: 2,
            group_by_type: true,
            sort_alphabetically: true,
            blank_lines_between_groups: 1,
            order: TypeOrder::default(),
        }
    }
}

impl Default for TypeOrder {
    fn default() -> Self {
        TypeOrder {
            types: vec!["env".into(), "alias".into(), "func".into(), "source".into()],
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        BackupConfig { max_count: 20 }
    }
}

impl Config {
    /// Get the wenv configuration directory path
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("~"))
                    .join(".config")
            })
            .join("wenv")
    }

    /// Get the configuration file path
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Get the backups directory path
    pub fn backups_dir() -> PathBuf {
        Self::config_dir().join("backups")
    }

    /// Load configuration from file, or return default if file doesn't exist
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
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
        assert_eq!(config.format.indent, 2);
        assert!(config.format.group_by_type);
        assert!(config.format.sort_alphabetically);
        assert_eq!(config.backup.max_count, 20);
    }

    #[test]
    fn test_type_order_default() {
        let order = TypeOrder::default();
        assert_eq!(order.types, vec!["env", "alias", "func", "source"]);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.format.indent, config.format.indent);
    }
}
