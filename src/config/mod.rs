//! Configuration management module

pub mod cache;
pub mod path_resolver;
pub mod templates;

use anyhow::Result;

use crate::model::{Config, FilesConfig};

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
