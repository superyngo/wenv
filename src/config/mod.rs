//! Configuration management module

pub mod cache;
pub mod path_resolver;
pub mod templates;

use anyhow::Result;

use crate::model::{Config, FilesConfig, Snippet};

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

/// Ensure config has default snippets for the given shell if none configured.
pub fn ensure_shell_snippets(config: &mut Config, shell_key: &str) -> Result<()> {
    if config.snippets.contains_key(shell_key) {
        return Ok(());
    }
    let defaults = templates::default_snippets(shell_key);
    if !defaults.is_empty() {
        config.snippets.insert(shell_key.to_string(), defaults);
        config.save()?;
    }
    Ok(())
}

/// Load snippets for the given shell, returning configured or defaults.
pub fn load_snippets_for_shell(config: &Config, shell_key: &str) -> Vec<Snippet> {
    if let Some(snippets) = config.snippets.get(shell_key) {
        if !snippets.is_empty() {
            return snippets.clone();
        }
    }
    templates::default_snippets(shell_key)
}
