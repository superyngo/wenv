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

/// Main configuration structure (UI + file lists). Lives at a single user
/// location (`~/.config/wenv/config.toml`) or wherever `-c/--config` points.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(skip)]
    pub source_path: PathBuf,

    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub files: HashMap<String, FilesConfig>,
}

/// Snippet templates for the "new entry" picker. This is a mandatory bundled
/// resource (`Resources/snippets.toml`) shipped alongside the binary; it is
/// never auto-generated, and the app refuses to run if it cannot be found.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snippets {
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
    /// The wenv configuration directory: `~/.config/wenv` on all platforms.
    /// Also used by i18n to locate `i18n/{lang}.toml`.
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".config")
            .join("wenv")
    }

    /// The default configuration file path: `~/.config/wenv/config.toml`.
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load the config from a single location, creating a default if missing.
    ///
    /// The location is `config_override` when `-c/--config` was given, otherwise
    /// the fixed `config_path()`. There is no multi-location search: the chosen
    /// path is authoritative for both reading and creation.
    pub fn resolve_or_create(
        shell_key: &str,
        config_override: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let path = config_override.unwrap_or_else(Self::config_path);

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let mut cfg: Config = toml::from_str(&content)?;
            cfg.source_path = path;
            return Ok(cfg);
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut cfg = Config::default();
        if let Some(paths) = crate::config::templates::default_paths(shell_key) {
            cfg.files
                .insert(shell_key.to_string(), crate::model::FilesConfig { paths });
        }
        cfg.source_path = path.clone();
        let serialized = toml::to_string_pretty(&cfg)?;
        std::fs::write(&path, &serialized)?;
        eprintln!("✓ Created default config at: {}", path.display());
        Ok(cfg)
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.source_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        std::fs::write(&self.source_path, &serialized)?;
        Ok(())
    }
}

impl Snippets {
    /// Candidate locations for the bundled `snippets.toml`, in priority order.
    /// Debug builds look in the in-repo `Resources/` first so `cargo run` works
    /// without any install step. Release builds rely on the binary-relative
    /// `Resources/` (matching the release archive layout) plus the documented
    /// install locations.
    fn search_paths() -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Vec::new();

        #[cfg(debug_assertions)]
        v.push(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/Resources/snippets.toml"
        )));

        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            v.push(dir.join("Resources").join("snippets.toml"));
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(h) = dirs::home_dir() {
                v.push(h.join(".wenget/apps/wenv/Resources/snippets.toml"));
                v.push(h.join(".local/bin/Resources/snippets.toml"));
            }
            v.push(PathBuf::from(
                "/opt/wenget/apps/wenv/Resources/snippets.toml",
            ));
            v.push(PathBuf::from("/usr/local/bin/Resources/snippets.toml"));
        }

        #[cfg(target_os = "windows")]
        {
            let env = |k: &str| std::env::var(k).ok().map(PathBuf::from);
            if let Some(p) = env("USERPROFILE") {
                v.push(p.join(".wenget/apps/wenv/Resources/snippets.toml"));
            }
            if let Some(p) = env("LOCALAPPDATA") {
                v.push(p.join("Programs/wenv/Resources/snippets.toml"));
            }
            if let Some(p) = env("ProgramW6432") {
                v.push(p.join("wenget/apps/wenv/Resources/snippets.toml"));
            }
            if let Some(p) = env("ProgramFiles") {
                v.push(p.join("gpinstall/Resources/snippets.toml"));
            }
        }

        v
    }

    /// Load the mandatory snippets resource. Returns an error listing every
    /// searched location if none exist — the caller is expected to abort.
    pub fn resolve() -> anyhow::Result<Self> {
        let candidates = Self::search_paths();
        for p in &candidates {
            if p.exists() {
                let content = std::fs::read_to_string(p)?;
                return Ok(toml::from_str(&content)?);
            }
        }
        let searched = candidates
            .iter()
            .map(|p| format!("  - {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!(
            "Required snippets resource 'Resources/snippets.toml' not found. Searched:\n{}",
            searched
        )
    }

    /// Snippets configured for the given shell, or an empty list.
    pub fn for_shell(&self, shell_key: &str) -> Vec<Snippet> {
        self.snippets.get(shell_key).cloned().unwrap_or_default()
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
