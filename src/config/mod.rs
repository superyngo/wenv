//! Configuration management module

pub mod path_resolver;
pub mod templates;

use anyhow::Result;
use std::path::PathBuf;

use crate::model::{Config, FilesConfig, Snippet};

/// Ensure the configuration directory exists
pub fn ensure_config_dir() -> Result<PathBuf> {
    let config_path = Config::config_path();
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(config_path)
}

/// Load or create default configuration
pub fn load_or_create_config() -> Result<Config> {
    let config_path = Config::config_path();

    if config_path.exists() {
        Config::load()
    } else {
        let config = Config::default();
        // Optionally save default config
        // config.save()?;
        Ok(config)
    }
}

/// Save configuration
pub fn save_config(config: &Config) -> Result<()> {
    ensure_config_dir()?;
    config.save()
}

/// First-run setup: create config with default file list for the detected shell
pub fn first_run_setup(shell_key: &str) -> Result<Config> {
    let mut config = Config::default();
    if let Some(paths) = templates::default_paths(shell_key) {
        config
            .files
            .insert(shell_key.to_string(), FilesConfig { paths });
    }
    config.save()?;
    Ok(config)
}

/// Ensure config has file list for the given shell. Returns true if added.
pub fn ensure_shell_files(config: &mut Config, shell_key: &str) -> Result<bool> {
    if config.files.contains_key(shell_key) {
        return Ok(false);
    }
    if let Some(paths) = templates::default_paths(shell_key) {
        config
            .files
            .insert(shell_key.to_string(), FilesConfig { paths });
        config.save()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Ensure config has snippets for the given shell. Returns true if added.
pub fn ensure_shell_snippets(config: &mut Config, shell_key: &str) -> anyhow::Result<bool> {
    if config.snippets.contains_key(shell_key) {
        return Ok(false);
    }
    if let Some(snippets) = templates::default_snippets(shell_key) {
        config.snippets.insert(shell_key.to_string(), snippets);
        config.save()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Load merged snippets for a shell: inline from config + external files, deduped by name.
pub fn load_snippets_for_shell(config: &Config, shell_key: &str) -> Vec<Snippet> {
    let mut result: Vec<Snippet> = config
        .snippets
        .get(shell_key)
        .cloned()
        .unwrap_or_default();

    let mut seen_names: std::collections::HashSet<String> =
        result.iter().map(|s| s.name.clone()).collect();

    for path_str in &config.template_paths.paths {
        let resolved = path_resolver::expand_tilde(path_str);
        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let external: toml::Value = match toml::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(shell_snippets) = external.get("snippets").and_then(|s| s.get(shell_key)) {
            if let Some(array) = shell_snippets.as_array() {
                for item in array {
                    if let Ok(snippet) = item.clone().try_into::<Snippet>() {
                        if !seen_names.contains(&snippet.name) {
                            seen_names.insert(snippet.name.clone());
                            result.push(snippet);
                        }
                    }
                }
            }
        }
    }

    result
}
