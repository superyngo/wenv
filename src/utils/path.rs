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

/// Check if a file path is writable.
/// For existing files: try opening for write.
/// For non-existent files: check parent directory is writable via metadata.
pub fn check_writable(path: &Path) -> bool {
    if path.exists() {
        std::fs::OpenOptions::new().write(true).open(path).is_ok()
    } else {
        path.parent().is_some_and(|p| {
            p.exists()
                && std::fs::metadata(p)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false)
        })
    }
}

/// Returns true if the file at `path` appears to be text (no null bytes
/// in the first 8 KiB). Missing files or unreadable files return true so
/// the dir-expansion filter doesn't accidentally hide pending or transient
/// entries.
pub fn is_likely_text(path: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 8192];
    let n = f.read(&mut buf).unwrap_or(0);
    !buf[..n].contains(&0)
}

/// Probe whether `dir` is writable by attempting to create and delete a
/// unique temporary file. Used by Cache::cache_path_for fallback logic.
pub fn is_dir_writable(dir: &std::path::Path) -> bool {
    if !dir.exists() {
        return false;
    }
    let probe = dir.join(format!(".wenv-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_expand_tilde() {
        let path = expand_tilde("~/.bashrc");
        assert!(!path.to_string_lossy().starts_with('~'));
    }

    #[test]
    #[cfg(unix)]
    fn test_normalize_absolute_path() {
        let path = normalize_path("/etc/passwd");
        assert_eq!(path, PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn is_likely_text_handles_pure_text() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a.sh");
        std::fs::write(&p, b"#!/bin/sh\necho hi\n").unwrap();
        assert!(is_likely_text(&p));
    }

    #[test]
    fn is_likely_text_rejects_null_byte() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bin.dat");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(&[0x7f, 0x45, 0x4c, 0x46, 0x00, 0x01]).unwrap();
        assert!(!is_likely_text(&p));
    }

    #[test]
    fn is_likely_text_handles_empty_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("empty");
        std::fs::write(&p, b"").unwrap();
        assert!(is_likely_text(&p));
    }

    #[test]
    fn is_likely_text_handles_missing_file() {
        let p = std::path::PathBuf::from("/definitely/not/here/x");
        assert!(is_likely_text(&p));
    }

    #[test]
    fn is_dir_writable_true_for_tempdir() {
        let dir = tempdir().unwrap();
        assert!(is_dir_writable(dir.path()));
    }
}
