//! Info command implementation - Display detailed information about an entry

use anyhow::{bail, Result};
use colored::Colorize;

use super::CommandContext;
use crate::cli::args::EntryTypeArg;
use crate::model::EntryType;

/// Execute the info command
pub fn execute(ctx: &CommandContext, entry_type: EntryTypeArg, name: &str) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;
    let msg = ctx.messages;

    // Convert entry type argument
    let filter_type: EntryType = entry_type.into();

    // Find the entry
    let entry = ctx.find_entry(&parse_result.entries, filter_type, name);

    match entry {
        Some(entry) => {
            // Display entry details in a formatted box
            let type_colored = ctx.color_entry_type(entry.entry_type);

            // Calculate max width for the box
            let title = format!("{}: {}", type_colored, entry.name.white().bold());
            let value_lines: Vec<&str> = entry.value.lines().collect();
            let max_value_len = value_lines.iter().map(|l| l.len()).max().unwrap_or(0);
            let box_width = std::cmp::max(50, max_value_len + 10);

            // Print top border with title
            println!(
                "╭─ {} {}",
                title,
                "─".repeat(box_width.saturating_sub(title.len() + 4))
            );

            // Print line number
            if let Some(line_num) = entry.line_number {
                if let Some(end_line) = entry.end_line {
                    println!(
                        "│ {:<12} {}-{}",
                        msg.header_lines.cyan(),
                        line_num,
                        end_line
                    );
                } else {
                    println!("│ {:<12} {}", msg.header_line.cyan(), line_num);
                }
            }

            // Print value
            println!("│ {:<12} ", msg.header_value.cyan());
            for line in &value_lines {
                println!("│ {:<12} {}", "", line.dimmed());
            }

            // Print comment if exists
            if let Some(comment) = &entry.comment {
                println!("│ {:<12} {}", msg.header_comment.cyan(), comment.dimmed());
            }

            // Print raw line if exists
            if let Some(raw) = &entry.raw_line {
                println!("│ {:<12} {}", msg.header_raw.cyan(), raw.dimmed());
            }

            // Print bottom border
            println!("╰{}", "─".repeat(box_width + 1));

            Ok(())
        }
        None => {
            bail!(
                "{}",
                msg.entry_not_found
                    .replace("{}", &format!("{}", filter_type))
                    .replacen("{}", name, 1)
            );
        }
    }
}
