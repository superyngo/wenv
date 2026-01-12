//! Path utilities

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Expand tilde (~) in path to home directory
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped.trim_start_matches('/'));
        }
    }
    PathBuf::from(path)
}

/// Normalize a path (expand tilde, resolve relative paths)
pub fn normalize_path(path: &str) -> PathBuf {
    let expanded = expand_tilde(path);
    if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(expanded)
    }
}

/// Check if a file exists and is readable
pub fn check_file_readable(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }
    if !path.is_file() {
        anyhow::bail!("Not a file: {}", path.display());
    }
    Ok(())
}

/// Read file content with proper error handling
pub fn read_file(path: &Path) -> Result<String> {
    check_file_readable(path)?;
    let content = std::fs::read_to_string(path)?;
    Ok(content)
}

/// Write file content with proper error handling
pub fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let path = expand_tilde("~/.bashrc");
        assert!(!path.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_normalize_absolute_path() {
        let path = normalize_path("/etc/passwd");
        assert_eq!(path, PathBuf::from("/etc/passwd"));
    }
}
