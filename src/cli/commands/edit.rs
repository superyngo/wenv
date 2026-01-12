//! Edit command implementation

use anyhow::Result;
use colored::Colorize;
use dialoguer::{Editor, Input};
use std::env;
use std::process::Command;

use super::CommandContext;
use crate::backup::BackupManager;
use crate::cli::args::EntryTypeArg;
use crate::formatter::get_formatter;
use crate::model::{Entry, EntryType};

/// Execute the edit command
pub fn execute(ctx: &CommandContext, entry_type: EntryTypeArg, name: &str) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;
    let target_type: EntryType = entry_type.into();

    // Find the entry
    let entry = parse_result
        .entries
        .iter()
        .find(|e| e.entry_type == target_type && e.name == name);

    let entry = match entry {
        Some(e) => e.clone(),
        None => {
            ctx.print_error(&format!("{} '{}' not found", target_type, name));
            return Ok(());
        }
    };

    println!(
        "Editing {} '{}' (current value: '{}')",
        target_type,
        name.cyan(),
        entry.value.dimmed()
    );

    // Get new value based on entry type
    let new_value = if target_type == EntryType::Function || target_type == EntryType::Code {
        // Use editor for functions and code blocks
        edit_with_editor(&entry.value)?
    } else {
        // Use interactive input for simple entries
        Input::new()
            .with_prompt("New value")
            .with_initial_text(&entry.value)
            .interact_text()?
    };

    if new_value == entry.value {
        println!("No changes made.");
        return Ok(());
    }

    // Create backup
    let backup_manager = BackupManager::new(ctx.shell_type, &ctx.config);
    backup_manager.create_backup(&ctx.config_file)?;

    // Create updated entry
    let updated_entry = Entry::new(target_type, name.to_string(), new_value.clone())
        .with_line_number(entry.line_number.unwrap_or(1));

    // Format the updated entry
    let formatter = get_formatter(ctx.shell_type);
    let new_line = formatter.format_entry(&updated_entry);

    // Read and update file
    let content = std::fs::read_to_string(&ctx.config_file)?;
    let lines: Vec<&str> = content.lines().collect();

    let mut new_lines: Vec<String> = Vec::new();
    let line_to_update = entry.line_number.unwrap_or(0);

    // Handle multi-line entries (functions and code blocks)
    let is_multiline = (target_type == EntryType::Function || target_type == EntryType::Code)
        && entry
            .raw_line
            .as_ref()
            .map(|r| r.contains('\n'))
            .unwrap_or(false);

    if is_multiline {
        let line_count = entry
            .raw_line
            .as_ref()
            .map(|r| r.lines().count())
            .unwrap_or(1);
        let start_line = line_to_update;
        let end_line = start_line + line_count - 1;

        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            if line_num == start_line {
                new_lines.push(new_line.clone());
            } else if line_num > start_line && line_num <= end_line {
                // Skip old lines
                continue;
            } else {
                new_lines.push(line.to_string());
            }
        }
    } else {
        for (idx, line) in lines.iter().enumerate() {
            let line_num = idx + 1;
            if line_num == line_to_update {
                new_lines.push(new_line.clone());
            } else {
                new_lines.push(line.to_string());
            }
        }
    }

    // Write back
    let new_content = format!("{}\n", new_lines.join("\n"));
    std::fs::write(&ctx.config_file, new_content)?;

    ctx.print_success(&format!(
        "Updated {} '{}' = '{}'",
        target_type,
        name.cyan(),
        new_value
    ));

    ctx.print_reload_hint();

    Ok(())
}

/// Open the config file directly in editor
pub fn edit_config_file_directly(ctx: &CommandContext) -> Result<()> {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    println!(
        "Opening {} in {}...",
        ctx.config_file.display().to_string().cyan(),
        editor.yellow()
    );

    let status = Command::new(&editor).arg(&ctx.config_file).status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    ctx.print_success("Config file edited successfully");
    ctx.print_reload_hint();

    Ok(())
}

fn edit_with_editor(content: &str) -> Result<String> {
    // Try to use $EDITOR, fall back to dialoguer's editor
    if let Ok(editor) = env::var("EDITOR") {
        // Create temp file
        let temp_dir = env::temp_dir();
        let temp_file = temp_dir.join("wenv_edit.tmp");
        std::fs::write(&temp_file, content)?;

        // Open editor
        let status = Command::new(&editor).arg(&temp_file).status()?;

        if !status.success() {
            anyhow::bail!("Editor exited with non-zero status");
        }

        // Read result
        let result = std::fs::read_to_string(&temp_file)?;
        std::fs::remove_file(&temp_file).ok();

        Ok(result.trim().to_string())
    } else {
        // Use dialoguer's editor
        if let Some(edited) = Editor::new().edit(content)? {
            Ok(edited.trim().to_string())
        } else {
            Ok(content.to_string())
        }
    }
}
