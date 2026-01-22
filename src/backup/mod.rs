//! Backup management module

use anyhow::Result;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

use crate::model::{Config, ShellType};

/// Backup entry information
#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub id: String,
    pub path: PathBuf,
    pub timestamp: String,
    pub filename: String,
    pub size: u64,
}

/// Backup manager
pub struct BackupManager {
    backup_dir: PathBuf,
    max_count: usize,
    cleanup_counter: std::cell::Cell<u32>,
    last_cleanup_time: std::cell::Cell<Option<time::OffsetDateTime>>,
}

impl BackupManager {
    pub fn new(shell_type: ShellType, config: &Config) -> Self {
        let backup_dir = Config::backups_dir().join(shell_type.name());
        Self {
            backup_dir,
            max_count: config.backup.max_count,
            cleanup_counter: std::cell::Cell::new(0),
            last_cleanup_time: std::cell::Cell::new(None),
        }
    }

    /// Ensure backup directory exists
    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.backup_dir)?;
        Ok(())
    }

    /// Create a backup of the specified file
    pub fn create_backup(&self, source_file: &Path) -> Result<PathBuf> {
        self.ensure_dir()?;

        let now = OffsetDateTime::now_utc();
        let timestamp = format!(
            "{:04}-{:02}-{:02}_{:02}{:02}{:02}",
            now.year(),
            now.month() as u8,
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        );

        let filename = source_file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "config".to_string());

        let backup_name = format!("{}_{}.bak", timestamp, filename);
        let backup_path = self.backup_dir.join(&backup_name);

        std::fs::copy(source_file, &backup_path)?;

        // Auto-cleanup old backups (with frequency control)
        // Only cleanup every 10 backups or after 1 hour
        let counter = self.cleanup_counter.get();
        let should_cleanup = if counter >= 10 {
            true
        } else if let Some(last_cleanup) = self.last_cleanup_time.get() {
            let now = OffsetDateTime::now_utc();
            let duration = now - last_cleanup;
            duration.whole_hours() >= 1
        } else {
            true // First backup, always cleanup
        };

        if should_cleanup {
            self.cleanup_old_backups()?;
            self.cleanup_counter.set(0);
            self.last_cleanup_time.set(Some(OffsetDateTime::now_utc()));
        } else {
            self.cleanup_counter.set(counter + 1);
        }

        Ok(backup_path)
    }

    /// List all backups
    pub fn list_backups(&self) -> Result<Vec<BackupEntry>> {
        self.ensure_dir()?;

        let mut entries = Vec::new();

        for entry in std::fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "bak").unwrap_or(false) {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let metadata = entry.metadata()?;
                let size = metadata.len();

                // Extract timestamp from filename
                let timestamp = filename.split('_').take(2).collect::<Vec<_>>().join("_");

                let id = filename.trim_end_matches(".bak").to_string();

                entries.push(BackupEntry {
                    id,
                    path: path.clone(),
                    timestamp,
                    filename,
                    size,
                });
            }
        }

        // Sort by timestamp (newest first)
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(entries)
    }

    /// Restore a backup by ID
    pub fn restore_backup(&self, id: &str, target_file: &Path) -> Result<()> {
        let backups = self.list_backups()?;

        let backup = backups
            .iter()
            .find(|b| b.id == id || b.filename.contains(id))
            .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", id))?;

        // Create a backup of current state before restoring
        if target_file.exists() {
            self.create_backup(target_file)?;
        }

        std::fs::copy(&backup.path, target_file)?;

        Ok(())
    }

    /// Clean up old backups, keeping only the specified number
    pub fn cleanup_old_backups(&self) -> Result<usize> {
        self.cleanup_keep(self.max_count)
    }

    /// Clean up backups, keeping only the specified number
    pub fn cleanup_keep(&self, keep: usize) -> Result<usize> {
        let backups = self.list_backups()?;

        if backups.len() <= keep {
            return Ok(0);
        }

        let mut removed = 0;
        for backup in backups.into_iter().skip(keep) {
            std::fs::remove_file(&backup.path)?;
            removed += 1;
        }

        Ok(removed)
    }

    /// Get a specific backup by ID
    pub fn get_backup(&self, id: &str) -> Result<Option<BackupEntry>> {
        let backups = self.list_backups()?;
        Ok(backups
            .into_iter()
            .find(|b| b.id == id || b.filename.contains(id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_backup_creation() {
        let temp_dir = tempdir().unwrap();
        let source_file = temp_dir.path().join("test.bashrc");
        std::fs::write(&source_file, "alias ll='ls -la'").unwrap();

        let config = Config::default();
        let manager = BackupManager {
            backup_dir: temp_dir.path().join("backups"),
            max_count: config.backup.max_count,
            cleanup_counter: std::cell::Cell::new(0),
            last_cleanup_time: std::cell::Cell::new(None),
        };

        let backup_path = manager.create_backup(&source_file).unwrap();
        assert!(backup_path.exists());
    }
}
