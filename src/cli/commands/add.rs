//! Add command implementation

use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

use super::CommandContext;
use crate::backup::BackupManager;
use crate::cli::args::{AddCommands, ConflictStrategy};
use crate::formatter::get_formatter;
use crate::model::{Entry, EntryType};

/// Execute the add command
pub fn execute(ctx: &CommandContext, add_cmd: &AddCommands) -> Result<()> {
    // Parse current file
    let parse_result = ctx.parse_config_file()?;

    // Create the new entry
    let new_entry = match add_cmd {
        AddCommands::Alias { definition } => parse_alias_definition(definition)?,
        AddCommands::Func { name, body } => {
            Entry::new(EntryType::Function, name.clone(), body.clone())
        }
        AddCommands::Env { definition } => parse_env_definition(definition)?,
        // For Source, use a placeholder name; it will get line number when parsed
        AddCommands::Source { path } => {
            Entry::new(EntryType::Source, "new".to_string(), path.clone())
        }
    };

    // Check for duplicates
    // For Source entries, compare by value (path) not name (which is line-based)
    let existing = parse_result.entries.iter().find(|e| {
        e.entry_type == new_entry.entry_type
            && if new_entry.entry_type == EntryType::Source {
                e.value == new_entry.value // Compare paths for source
            } else {
                e.name == new_entry.name // Compare names for others
            }
    });

    if let Some(existing_entry) = existing {
        let should_overwrite = match ctx.on_conflict {
            ConflictStrategy::Skip => {
                ctx.print_warning(&format!(
                    "{} '{}' already exists, skipping",
                    new_entry.entry_type, new_entry.name
                ));
                return Ok(());
            }
            ConflictStrategy::Overwrite => true,
            ConflictStrategy::Ask => {
                println!(
                    "{} '{}' already exists with value: {}",
                    new_entry.entry_type,
                    new_entry.name.cyan(),
                    existing_entry.value.dimmed()
                );
                Confirm::new()
                    .with_prompt("Overwrite?")
                    .default(false)
                    .interact()?
            }
        };

        if !should_overwrite {
            println!("Skipped.");
            return Ok(());
        }
    }

    // Create backup before modification
    let backup_manager = BackupManager::new(ctx.shell_type, &ctx.config);
    if ctx.config_file.exists() {
        backup_manager.create_backup(&ctx.config_file)?;
    }

    // Read current file content
    let content = if ctx.config_file.exists() {
        std::fs::read_to_string(&ctx.config_file)?
    } else {
        String::new()
    };

    // Format the new entry
    let formatter = get_formatter(ctx.shell_type);
    let new_line = formatter.format_entry(&new_entry);

    // Append or replace
    let new_content = if existing.is_some() {
        // Replace existing entry
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        if let Some(line_num) = existing.and_then(|e| e.line_number) {
            if line_num > 0 && line_num <= lines.len() {
                lines[line_num - 1] = new_line.clone();
            }
        }
        lines.join("\n")
    } else {
        // Append new entry
        if content.is_empty() {
            new_line.clone()
        } else if content.ends_with('\n') {
            format!("{}{}\n", content, new_line)
        } else {
            format!("{}\n{}\n", content, new_line)
        }
    };

    // Write back
    std::fs::write(&ctx.config_file, new_content)?;

    ctx.print_success(&format!(
        "Added {} '{}' = '{}'",
        new_entry.entry_type,
        if new_entry.entry_type == EntryType::Source {
            &new_entry.value // Show path for source entries
        } else {
            &new_entry.name
        }
        .cyan(),
        new_entry.value
    ));

    ctx.print_reload_hint();

    Ok(())
}

fn parse_alias_definition(definition: &str) -> Result<Entry> {
    let parts: Vec<&str> = definition.splitn(2, '=').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid alias format. Use: NAME=VALUE");
    }

    let name = parts[0].trim().to_string();
    let value = parts[1]
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .to_string();

    Ok(Entry::new(EntryType::Alias, name, value))
}

fn parse_env_definition(definition: &str) -> Result<Entry> {
    let parts: Vec<&str> = definition.splitn(2, '=').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid env format. Use: NAME=VALUE");
    }

    let name = parts[0].trim().to_string();
    let value = parts[1]
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .to_string();

    Ok(Entry::new(EntryType::EnvVar, name, value))
}
