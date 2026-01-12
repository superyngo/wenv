//! Backup command implementation

use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use super::CommandContext;
use crate::backup::BackupManager;
use crate::cli::args::BackupCommands;

/// Execute the backup command
pub fn execute(ctx: &CommandContext, backup_cmd: &BackupCommands) -> Result<()> {
    let backup_manager = BackupManager::new(ctx.shell_type, &ctx.config);

    match backup_cmd {
        BackupCommands::List => list_backups(&backup_manager),
        BackupCommands::Restore { id } => restore_backup(ctx, &backup_manager, id),
        BackupCommands::Clean { keep } => clean_backups(&backup_manager, *keep),
    }
}

fn list_backups(manager: &BackupManager) -> Result<()> {
    let backups = manager.list_backups()?;

    if backups.is_empty() {
        println!("{}", "No backups found.".dimmed());
        return Ok(());
    }

    println!("{}", "Available backups:".bold());
    println!();

    println!(
        "{:<30} {:<20} {}",
        "ID".bold().cyan(),
        "TIMESTAMP".bold().cyan(),
        "SIZE".bold().cyan()
    );
    println!("{}", "─".repeat(60).dimmed());

    for backup in &backups {
        let size_str = format_size(backup.size);
        println!(
            "{:<30} {:<20} {}",
            backup.id.white(),
            backup.timestamp.dimmed(),
            size_str.dimmed()
        );
    }

    println!();
    println!("{}", format!("Total: {} backup(s)", backups.len()).dimmed());

    Ok(())
}

fn restore_backup(ctx: &CommandContext, manager: &BackupManager, id: &str) -> Result<()> {
    let backup = manager.get_backup(id)?;

    let backup = match backup {
        Some(b) => b,
        None => {
            anyhow::bail!("Backup not found: {}", id);
        }
    };

    println!(
        "Restoring backup: {} ({})",
        backup.id.cyan(),
        backup.timestamp.dimmed()
    );

    if !Confirm::new()
        .with_prompt("This will overwrite your current configuration. Continue?")
        .default(false)
        .interact()?
    {
        println!("Cancelled.");
        return Ok(());
    }

    manager.restore_backup(id, &ctx.config_file)?;

    println!(
        "{} Restored backup to {}",
        "✓".green(),
        ctx.config_file.display().to_string().cyan()
    );

    ctx.print_reload_hint();

    Ok(())
}

fn clean_backups(manager: &BackupManager, keep: usize) -> Result<()> {
    let removed = manager.cleanup_keep(keep)?;

    if removed == 0 {
        println!("{}", "No old backups to clean.".dimmed());
    } else {
        println!(
            "{} Removed {} old backup(s), keeping {}",
            "✓".green(),
            removed,
            keep
        );
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
