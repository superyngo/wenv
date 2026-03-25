//! Configuration management module

pub mod path_resolver;
pub mod templates;

use anyhow::Result;
use std::path::PathBuf;

use crate::model::{Config, FilesConfig};

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
        config.files.insert(
            shell_key.to_string(),
            FilesConfig { paths },
        );
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
        config.files.insert(
            shell_key.to_string(),
            FilesConfig { paths },
        );
        config.save()?;
        Ok(true)
    } else {
        Ok(false)
    }
}
