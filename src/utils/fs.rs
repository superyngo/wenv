//! Durable file writes: temp file + rename, symlink-aware, with a single `.bak` copy

use anyhow::{Context, Result};

use std::io::Write;
use std::path::{Path, PathBuf};

/// Path of the backup kept next to `path` (`<name>.bak`).
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

/// Atomically replace `path` with `bytes`.
///
/// Symlinks are resolved first so the link itself survives (stow/chezmoi setups), existing
/// permissions are preserved, and the previous contents are copied to `<name>.bak` when
/// `backup` is set. Falls back to a direct write when the directory is not writable.
pub fn atomic_write(path: &Path, bytes: &[u8], backup: bool) -> Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let existing = std::fs::metadata(&target).ok();

    if backup && existing.is_some() {
        std::fs::copy(&target, backup_path(&target))
            .with_context(|| format!("Failed to back up {}", target.display()))?;
    }

    let parent = match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let tmp = tempfile::Builder::new()
        .prefix(".wenv-")
        .tempfile_in(parent);
    let mut tmp = match tmp {
        Ok(t) => t,
        Err(_) => {
            return std::fs::write(&target, bytes)
                .with_context(|| format!("Failed to write {}", target.display()));
        }
    };
    tmp.write_all(bytes)
        .and_then(|_| tmp.as_file().sync_all())
        .with_context(|| format!("Failed to write {}", target.display()))?;
    if let Some(meta) = existing {
        std::fs::set_permissions(tmp.path(), meta.permissions()).ok();
    }
    tmp.persist(&target)
        .map_err(|e| e.error)
        .with_context(|| format!("Failed to replace {}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_creates_backup_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("rc");
        std::fs::write(&p, "old").unwrap();
        atomic_write(&p, b"new", true).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(backup_path(&p)).unwrap(), "old");
    }

    #[test]
    fn test_atomic_write_new_file_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("rc");
        atomic_write(&p, b"x", true).unwrap();
        assert!(!backup_path(&p).exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_atomic_write_keeps_symlink_and_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let link = dir.path().join("link");
        std::fs::write(&real, "old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        atomic_write(&link, b"new", false).unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        let mode = std::fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
