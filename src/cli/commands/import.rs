//! Import command implementation

use anyhow::Result;
use colored::Colorize;
use dialoguer::{Confirm, Select};

use super::CommandContext;
use crate::backup::BackupManager;
use crate::cli::args::ConflictStrategy;
use crate::formatter::get_formatter;
use crate::parser::get_parser;
use crate::utils::http::{fetch_url, is_url};
use crate::utils::path::expand_tilde;

/// Execute the import command
pub fn execute(ctx: &CommandContext, source: &str, yes: bool) -> Result<()> {
    // Fetch content from source
    let content = if is_url(source) {
        println!("Fetching from URL: {}", source.cyan());
        fetch_url(source)?
    } else {
        let path = expand_tilde(source);
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }
        std::fs::read_to_string(&path)?
    };

    // Parse the source content
    let parser = get_parser(ctx.shell_type);
    let parse_result = parser.parse(&content);

    if parse_result.entries.is_empty() {
        println!("{}", "No entries found in source.".yellow());
        return Ok(());
    }

    // Print preview
    println!();
    println!(
        "{}",
        format!("Found {} entries:", parse_result.entries.len())
            .green()
            .bold()
    );
    println!();

    println!(
        "{:<10} {:<20} {}",
        "TYPE".bold().cyan(),
        "NAME".bold().cyan(),
        "VALUE".bold().cyan()
    );
    println!("{}", "─".repeat(60).dimmed());

    for entry in &parse_result.entries {
        let value = if entry.value.len() > 35 {
            format!("{}...", &entry.value[..32])
        } else {
            entry.value.clone()
        };
        let value = value.replace('\n', "\\n");

        println!(
            "{:<10} {:<20} {}",
            format!("{}", entry.entry_type).green(),
            entry.name.white(),
            value.dimmed()
        );
    }

    println!();

    // Confirm import
    if !yes
        && !Confirm::new()
            .with_prompt("Proceed with import?")
            .default(false)
            .interact()?
    {
        println!("Cancelled.");
        return Ok(());
    }

    // Parse current file to check for conflicts
    let current_result = ctx.parse_config_file().unwrap_or_default();
    let current_entries: std::collections::HashMap<_, _> = current_result
        .entries
        .iter()
        .map(|e| ((e.entry_type, &e.name), e))
        .collect();

    // Create backup
    let backup_manager = BackupManager::new(ctx.shell_type, &ctx.config);
    if ctx.config_file.exists() {
        backup_manager.create_backup(&ctx.config_file)?;
    }

    // Process entries
    let formatter = get_formatter(ctx.shell_type);
    let mut content = if ctx.config_file.exists() {
        std::fs::read_to_string(&ctx.config_file)?
    } else {
        String::new()
    };

    let mut imported = 0;
    let mut skipped = 0;
    let mut overwritten = 0;

    for entry in &parse_result.entries {
        let key = (entry.entry_type, &entry.name);

        if let Some(existing) = current_entries.get(&key) {
            // Handle conflict
            let action = match ctx.on_conflict {
                ConflictStrategy::Skip => {
                    skipped += 1;
                    continue;
                }
                ConflictStrategy::Overwrite => "overwrite",
                ConflictStrategy::Ask => {
                    if yes {
                        "skip"
                    } else {
                        let choices = ["Skip", "Overwrite", "Rename"];
                        let selection = Select::new()
                            .with_prompt(format!(
                                "{} '{}' already exists. Current: '{}', New: '{}'",
                                entry.entry_type,
                                entry.name.cyan(),
                                existing.value.dimmed(),
                                entry.value.dimmed()
                            ))
                            .items(&choices)
                            .default(0)
                            .interact()?;

                        match selection {
                            0 => "skip",
                            1 => "overwrite",
                            _ => "skip",
                        }
                    }
                }
            };

            match action {
                "skip" => {
                    skipped += 1;
                    continue;
                }
                "overwrite" => {
                    // Replace in content (simplified - append and handle duplicates later)
                    overwritten += 1;
                }
                _ => {
                    skipped += 1;
                    continue;
                }
            }
        }

        // Append entry
        let line = formatter.format_entry(entry);
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&line);
        content.push('\n');
        imported += 1;
    }

    // Write back
    std::fs::write(&ctx.config_file, &content)?;

    // Summary
    println!();
    ctx.print_success(&format!(
        "Imported {} entries ({} skipped, {} overwritten)",
        imported, skipped, overwritten
    ));

    if imported > 0 || overwritten > 0 {
        ctx.print_reload_hint();
    }

    Ok(())
}
