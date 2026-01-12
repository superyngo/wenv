//! Configuration management module

use anyhow::Result;
use std::path::PathBuf;

use crate::model::Config;

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
