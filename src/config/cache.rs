//! Sibling cache for runtime-discovered paths (currently PowerShell $PROFILE).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::model::Config;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub pwsh_profile: Option<String>,
    #[serde(default)]
    pub powershell_profile: Option<String>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl Cache {
    pub fn cache_path_for(config: &Config) -> PathBuf {
        config
            .source_path
            .parent()
            .map(|p| p.join("cache.toml"))
            .unwrap_or_else(|| PathBuf::from("cache.toml"))
    }

    pub fn load_or_default(config: &Config) -> Self {
        let p = Self::cache_path_for(config);
        let mut cache: Cache = if p.exists() {
            std::fs::read_to_string(&p)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Cache::default()
        };
        cache.source_path = p;
        if let Some(ref pp) = cache.pwsh_profile {
            if !std::path::Path::new(pp).exists() {
                cache.pwsh_profile = None;
            }
        }
        if let Some(ref pp) = cache.powershell_profile {
            if !std::path::Path::new(pp).exists() {
                cache.powershell_profile = None;
            }
        }
        cache
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.source_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.source_path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
