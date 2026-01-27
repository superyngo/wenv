//! Path caching system for shell profile paths

use crate::model::Config;
use anyhow::Result;
use std::path::PathBuf;

pub struct PathCache;

impl PathCache {
    /// Load cache from config
    pub fn load() -> Result<CacheData> {
        let config = Config::load()?;
        Ok(CacheData {
            pwsh_profile: config.cache.pwsh_profile,
            powershell_profile: config.cache.powershell_profile,
        })
    }

    /// Save cache to config
    pub fn save(data: &CacheData) -> Result<()> {
        let mut config = Config::load()?;
        config.cache.pwsh_profile = data.pwsh_profile.clone();
        config.cache.powershell_profile = data.powershell_profile.clone();
        config.save()
    }

    /// Clear the cache
    pub fn clear() -> Result<()> {
        let mut config = Config::load()?;
        config.cache.pwsh_profile = None;
        config.cache.powershell_profile = None;
        config.save()
    }
}

/// Cache data structure
#[derive(Debug, Clone)]
pub struct CacheData {
    pub pwsh_profile: Option<String>,
    pub powershell_profile: Option<String>,
}

impl CacheData {
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
