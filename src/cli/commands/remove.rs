//! Remove command implementation

use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use super::CommandContext;
use crate::backup::BackupManager;
use crate::cli::args::EntryTypeArg;
use crate::model::EntryType;

/// Execute the remove command
pub fn execute(ctx: &CommandContext, entry_type: EntryTypeArg, name: &str) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;
    let target_type: EntryType = entry_type.into();

    // Find the entry
    let entry = parse_result
        .entries
        .iter()
        .find(|e| e.entry_type == target_type && e.name == name);

    let entry = match entry {
        Some(e) => e,
        None => {
            ctx.print_error(&format!("{} '{}' not found", target_type, name));
            return Ok(());
        }
    };

    // Confirm deletion
    println!(
        "Found {} '{}' = '{}'",
        target_type,
        name.cyan(),
        entry.value.dimmed()
    );

    if !Confirm::new()
        .with_prompt("Remove this entry?")
        .default(false)
        .interact()?
    {
        println!("Cancelled.");
        return Ok(());
    }

    // Create backup
    let backup_manager = BackupManager::new(ctx.shell_type, &ctx.config);
    backup_manager.create_backup(&ctx.config_file)?;

    // Read file and remove the line
    let content = std::fs::read_to_string(&ctx.config_file)?;
    let lines: Vec<&str> = content.lines().collect();

    let line_to_remove = entry.line_number.unwrap_or(0);
    let mut new_lines: Vec<&str> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;

        // Skip the target line
        if line_num == line_to_remove {
            continue;
        }

        // Also skip preceding comment if associated
        if line_num == line_to_remove.saturating_sub(1) {
            let trimmed = line.trim();
            if trimmed.starts_with('#') && entry.comment.is_some() {
                continue;
            }
        }

        new_lines.push(line);
    }

    // For multi-line functions, we need to remove all lines
    if entry.entry_type == EntryType::Function {
        if let Some(ref raw) = entry.raw_line {
            let func_lines: Vec<&str> = raw.lines().collect();
            if func_lines.len() > 1 {
                // Re-process: remove all function lines
                let start_line = entry.line_number.unwrap_or(1);
                let end_line = start_line + func_lines.len() - 1;

                new_lines = lines
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        let line_num = idx + 1;
                        line_num < start_line || line_num > end_line
                    })
                    .map(|(_, line)| *line)
                    .collect();
            }
        }
    }

    // Write back
    let new_content = new_lines.join("\n");
    let new_content = if new_content.is_empty() {
        String::new()
    } else {
        format!("{}\n", new_content)
    };

    std::fs::write(&ctx.config_file, new_content)?;

    ctx.print_success(&format!("Removed {} '{}'", target_type, name));

    ctx.print_reload_hint();

    Ok(())
}
