//! Format command implementation

use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use super::CommandContext;
use crate::backup::BackupManager;
use crate::formatter::get_formatter;
use crate::model::{Entry, EntryType};

/// Execute the format command
pub fn execute(ctx: &CommandContext, dry_run: bool) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;
    let msg = ctx.messages;

    if parse_result.entries.is_empty() {
        ctx.print_warning(msg.no_entries_found);
        return Ok(());
    }

    // Check for multiple PATH entries and ask if user wants to merge them
    let path_entries: Vec<&Entry> = parse_result
        .entries
        .iter()
        .filter(|e| e.entry_type == EntryType::EnvVar && e.name == "PATH")
        .collect();

    let mut entries_to_format = parse_result.entries.clone();

    if path_entries.len() > 1 && !dry_run {
        println!(
            "{} Found {} PATH environment variable entries:",
            "ℹ".cyan(),
            path_entries.len()
        );
        for (idx, entry) in path_entries.iter().enumerate() {
            println!(
                "  {}. {} = {}",
                idx + 1,
                "PATH".yellow(),
                entry.value.dimmed()
            );
        }

        let should_merge = Confirm::new()
            .with_prompt("Would you like to merge these PATH entries into one?")
            .default(false)
            .interact()?;

        if should_merge {
            // Merge PATH entries
            let merged_value = path_entries
                .iter()
                .map(|e| e.value.as_str())
                .collect::<Vec<_>>()
                .join(":");

            // Create merged entry using the first PATH entry's line number
            let merged_entry = Entry::new(EntryType::EnvVar, "PATH".to_string(), merged_value)
                .with_line_number(path_entries[0].line_number.unwrap_or(1));

            // Remove all PATH entries and add the merged one
            entries_to_format.retain(|e| !(e.entry_type == EntryType::EnvVar && e.name == "PATH"));
            entries_to_format.push(merged_entry);

            println!("{} Merged PATH entries", "✓".green());
        }
    }

    // Format entries
    let formatter = get_formatter(ctx.shell_type);
    let formatted = formatter.format(&entries_to_format, &ctx.config);

    // Read current content
    let current = std::fs::read_to_string(&ctx.config_file)?;

    if formatted == current {
        ctx.print_success(msg.file_formatted);
        return Ok(());
    }

    if dry_run {
        println!("{}", "Dry run - showing formatted output:".yellow().bold());
        println!();
        println!("{}", "─".repeat(60).dimmed());
        println!("{}", formatted);
        println!("{}", "─".repeat(60).dimmed());
        println!();

        // Show diff summary
        let current_lines = current.lines().count();
        let formatted_lines = formatted.lines().count();

        println!(
            "{}",
            format!(
                "Current: {} lines, Formatted: {} lines",
                current_lines, formatted_lines
            )
            .dimmed()
        );

        return Ok(());
    }

    // Create backup
    let backup_manager = BackupManager::new(ctx.shell_type, &ctx.config);
    backup_manager.create_backup(&ctx.config_file)?;

    // Write formatted content
    std::fs::write(&ctx.config_file, &formatted)?;

    ctx.print_success(msg.file_formatted);

    ctx.print_reload_hint();

    Ok(())
}
