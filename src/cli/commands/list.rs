//! List command implementation

use anyhow::Result;
use colored::Colorize;

use super::CommandContext;
use crate::cli::args::EntryTypeArg;
use crate::model::EntryType;

/// Execute the list command
pub fn execute(ctx: &CommandContext, entry_type: Option<EntryTypeArg>) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;
    let msg = ctx.messages;

    // Print warnings if any
    for warning in &parse_result.warnings {
        ctx.print_warning(&format!(
            "Line {}: {}",
            warning.line_number, warning.message
        ));
    }

    // Filter entries if type specified
    let entries: Vec<_> = if let Some(type_arg) = entry_type {
        let filter_type: EntryType = type_arg.into();
        parse_result
            .entries
            .iter()
            .filter(|e| e.entry_type == filter_type)
            .collect()
    } else {
        parse_result.entries.iter().collect()
    };

    if entries.is_empty() {
        println!("{}", msg.no_entries_found.dimmed());
        return Ok(());
    }

    // Print header
    println!(
        "{:<10} {:<20} {:<10} {}",
        msg.header_type.bold().cyan(),
        msg.header_name.bold().cyan(),
        "LINE".bold().cyan(),
        msg.header_value.bold().cyan()
    );
    println!("{}", "─".repeat(70).dimmed());

    // Print entries
    let entry_count = entries.len();
    for entry in entries {
        let type_colored = ctx.color_entry_type(entry.entry_type);

        // Format line info (single line or range for multi-line)
        let line_info = match (entry.line_number, entry.end_line) {
            (Some(start), Some(end)) if end > start => format!("{}-{}", start, end),
            (Some(line), _) => format!("{}", line),
            (None, _) => "-".to_string(),
        };

        // Truncate long values
        let value = if entry.value.len() > 40 {
            format!("{}...", &entry.value[..37])
        } else {
            entry.value.clone()
        };

        // Handle multi-line functions
        let value = value.replace('\n', "\\n");

        println!(
            "{:<10} {:<20} {:<10} {}",
            type_colored,
            entry.name.white(),
            line_info.dimmed(),
            value.dimmed()
        );
    }

    println!();
    println!(
        "{}",
        msg.total_entries
            .replace("{}", &entry_count.to_string())
            .dimmed()
    );

    Ok(())
}
