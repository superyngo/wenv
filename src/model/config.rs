//! Application configuration structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A snippet template shown in the "new entry" picker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub template: Option<String>,
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(skip)]
    pub source_path: PathBuf,

    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub files: HashMap<String, FilesConfig>,
    #[serde(default)]
    pub snippets: HashMap<String, Vec<Snippet>>,
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

fn default_language() -> String {
    "en".to_string()
}


impl Default for UiConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
        }
    }
}

impl Config {
    /// Get the wenv configuration directory path (legacy fallback used by some modules)
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".config")
            .join("wenv")
    }

    /// Get the configuration file path (legacy fallback)
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load configuration from the legacy config path
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut cfg: Config = toml::from_str(&content)?;
            cfg.source_path = path;
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }

    /// Fallback search chain for config.toml.
    pub fn fallback_paths() -> Vec<PathBuf> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        let home = dirs::home_dir();
        let mut v: Vec<PathBuf> = Vec::new();

        if let Ok(d) = std::env::var("WENV_CONFIG_DIR") {
            if !d.trim().is_empty() {
                v.push(PathBuf::from(&d).join("config.toml"));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(d) = &exe_dir { v.push(d.join("Resources/config.toml")); }
            if let Some(h) = &home {
                v.push(h.join(".wenget/apps/wenv/Resources/config.toml"));
                v.push(h.join(".local/bin/Resources/config.toml"));
            }
            v.push(PathBuf::from("/opt/wenget/apps/wenv/Resources/config.toml"));
            v.push(PathBuf::from("/usr/local/bin/config/config.toml"));
        }

        #[cfg(target_os = "windows")]
        {
            let env = |k: &str| std::env::var(k).ok().map(PathBuf::from);
            if let Some(d) = &exe_dir { v.push(d.join("Resources").join("config.toml")); }
            if let Some(p) = env("USERPROFILE")  { v.push(p.join(".wenget/apps/wenv/Resources/config.toml")); }
            if let Some(p) = env("LOCALAPPDATA") { v.push(p.join("Programs/wenv/Resources/config.toml")); }
            if let Some(p) = env("ProgramW6432") { v.push(p.join("wenget/apps/wenv/Resources/config.toml")); }
            if let Some(p) = env("ProgramFiles") { v.push(p.join("gpinstall/Resources/config.toml")); }
        }

        v
    }

    pub fn resolve_or_create(shell_key: &str) -> anyhow::Result<Self> {
        // If WENV_CONFIG_DIR is set, prefer it for both reading and creating.
        if let Ok(d) = std::env::var("WENV_CONFIG_DIR") {
            if !d.trim().is_empty() {
                let p = PathBuf::from(&d).join("config.toml");
                if p.exists() {
                    let content = std::fs::read_to_string(&p)?;
                    let mut cfg: Config = toml::from_str(&content)?;
                    cfg.source_path = p;
                    return Ok(cfg);
                }
                // Try to create in WENV_CONFIG_DIR first
                if let Some(parent) = p.parent() {
                    if std::fs::create_dir_all(parent).is_ok() {
                        let mut cfg = Config::default();
                        if let Some(paths) = crate::config::templates::default_paths(shell_key) {
                            cfg.files.insert(
                                shell_key.to_string(),
                                crate::model::FilesConfig { paths },
                            );
                        }
                        let serialized = toml::to_string_pretty(&cfg)?;
                        if std::fs::write(&p, &serialized).is_ok() {
                            cfg.source_path = p.clone();
                            eprintln!("✓ Created default config at: {}", p.display());
                            return Ok(cfg);
                        }
                    }
                }
            }
        }

        // Next, look for any existing fallback file and return the first we can read.
        for p in Self::fallback_paths() {
            if p.exists() {
                let content = std::fs::read_to_string(&p)?;
                let mut cfg: Config = toml::from_str(&content)?;
                cfg.source_path = p;
                return Ok(cfg);
            }
        }

        // Finally, attempt to create in the first writable fallback path.
        for p in Self::fallback_paths() {
            let Some(parent) = p.parent() else { continue };
            if std::fs::create_dir_all(parent).is_err() { continue; }
            let mut cfg = Config::default();
            if let Some(paths) = crate::config::templates::default_paths(shell_key) {
                cfg.files.insert(
                    shell_key.to_string(),
                    crate::model::FilesConfig { paths },
                );
            }
            let serialized = toml::to_string_pretty(&cfg)?;
            if std::fs::write(&p, &serialized).is_ok() {
                cfg.source_path = p.clone();
                eprintln!("✓ Created default config at: {}", p.display());
                return Ok(cfg);
            }
        }
        anyhow::bail!("No writable config location among fallback paths")
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.source_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        match std::fs::write(&self.source_path, &serialized) {
            Ok(()) => Ok(()),
            Err(e) if matches!(e.kind(),
                std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::ReadOnlyFilesystem) =>
            {
                for p in Self::fallback_paths() {
                    if p == self.source_path { continue; }
                    let Some(parent) = p.parent() else { continue };
                    if std::fs::create_dir_all(parent).is_err() { continue; }
                    if std::fs::write(&p, &serialized).is_ok() {
                        eprintln!(
                            "⚠ Config at {} is read-only; saved to {} instead.",
                            self.source_path.display(), p.display()
                        );
                        self.source_path = p;
                        return Ok(());
                    }
                }
                Err(anyhow::anyhow!(
                    "Config save failed: {} is read-only and no writable fallback is available",
                    self.source_path.display()
                ))
            }
            Err(e) => Err(e.into()),
        }
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
}
