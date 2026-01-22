//! Path caching system for shell profile paths

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct PathCache {
    pub pwsh_profile: Option<String>,
    pub powershell_profile: Option<String>,
}

impl PathCache {
    /// Get the path to the cache file
    fn cache_file_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|p| p.join(".config")))
            .context("Cannot determine config directory")?;

        let wenv_dir = config_dir.join("wenv");
        fs::create_dir_all(&wenv_dir)
            .context("Failed to create wenv config directory")?;

        Ok(wenv_dir.join(".path_cache.toml"))
    }

    /// Load cache from disk
    pub fn load() -> Result<Self> {
        let cache_path = Self::cache_file_path()?;

        if !cache_path.exists() {
            return Ok(PathCache {
                pwsh_profile: None,
                powershell_profile: None,
            });
        }

        let content = fs::read_to_string(&cache_path)
            .context("Failed to read cache file")?;

        toml::from_str(&content)
            .context("Failed to parse cache file")
    }

    /// Save cache to disk
    pub fn save(&self) -> Result<()> {
        let cache_path = Self::cache_file_path()?;
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize cache")?;

        fs::write(&cache_path, content)
            .context("Failed to write cache file")
    }

    /// Clear the cache file
    pub fn clear() -> Result<()> {
        let cache_path = Self::cache_file_path()?;
        if cache_path.exists() {
            fs::remove_file(&cache_path)
                .context("Failed to remove cache file")?;
        }
        Ok(())
    }

    /// Get cached PowerShell profile path, validating it still exists
    pub fn get_pwsh_profile(&self) -> Option<PathBuf> {
        self.pwsh_profile
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
    }

    /// Get cached Windows PowerShell profile path, validating it still exists
    pub fn get_powershell_profile(&self) -> Option<PathBuf> {
        self.powershell_profile
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
    }

    /// Set pwsh profile path
    pub fn set_pwsh_profile(&mut self, path: PathBuf) {
        self.pwsh_profile = Some(path.to_string_lossy().to_string());
    }

    /// Set Windows PowerShell profile path
    pub fn set_powershell_profile(&mut self, path: PathBuf) {
        self.powershell_profile = Some(path.to_string_lossy().to_string());
    }
}
