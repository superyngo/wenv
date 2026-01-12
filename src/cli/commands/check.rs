//! Check command implementation

use anyhow::Result;
use colored::Colorize;

use super::CommandContext;
use crate::checker::{check_all, Severity};

/// Execute the check command
pub fn execute(ctx: &CommandContext) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;
    let msg = ctx.messages;

    // Print parsing warnings
    if !parse_result.warnings.is_empty() {
        println!("{}", msg.parse_warnings.yellow().bold());
        for warning in &parse_result.warnings {
            println!(
                "  {} Line {}: {}",
                "⚠".yellow(),
                warning.line_number,
                warning.message
            );
        }
        println!();
    }

    // Run checks
    let check_result = check_all(&parse_result.entries);

    if check_result.is_ok() && parse_result.warnings.is_empty() {
        ctx.print_success(msg.no_issues_found);
        println!(
            "{}",
            msg.checked_entries
                .replace("{}", &parse_result.entries.len().to_string())
                .dimmed()
        );
        return Ok(());
    }

    // Print check issues
    if !check_result.issues.is_empty() {
        println!("{}", msg.issues_found.red().bold());
        for issue in &check_result.issues {
            let icon = match issue.severity {
                Severity::Error => "✗".red(),
                Severity::Warning => "⚠".yellow(),
            };

            let severity = match issue.severity {
                Severity::Error => "ERROR".red(),
                Severity::Warning => "WARNING".yellow(),
            };

            print!("  {} [{}]", icon, severity);

            if let Some(line) = issue.line_number {
                print!(" Line {}", line);
            }

            if let Some(ref name) = issue.entry_name {
                print!(" ({})", name.cyan());
            }

            println!(": {}", issue.message);
        }
    }

    // Summary
    println!();
    let error_count = check_result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, Severity::Error))
        .count();
    let warning_count = check_result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, Severity::Warning))
        .count();
    let parse_warning_count = parse_result.warnings.len();

    if error_count > 0 {
        println!(
            "{}",
            msg.found_errors_warnings
                .replace("{}", &error_count.to_string())
                .replacen("{}", &warning_count.to_string(), 1)
                .replacen("{}", &parse_warning_count.to_string(), 1)
                .red()
        );
    } else {
        println!(
            "{}",
            msg.found_warnings
                .replace("{}", &(warning_count + parse_warning_count).to_string())
                .replacen("{}", &parse_warning_count.to_string(), 1)
                .yellow()
        );
    }

    Ok(())
}
