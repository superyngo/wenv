//! Export command implementation

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::cli::args::EntryTypeArg;
use crate::cli::context::Context;
use crate::formatter::get_formatter;
use crate::model::EntryType;

/// Execute the export action
pub fn execute(ctx: &Context, entry_type: Option<EntryTypeArg>, output: &PathBuf) -> Result<()> {
    let parse_result = ctx.parse_config_file()?;

    // Filter entries if type specified
    let entries: Vec<_> = if let Some(type_arg) = entry_type {
        let filter_type: EntryType = type_arg.into();
        parse_result
            .entries
            .iter()
            .filter(|e| e.entry_type == filter_type)
            .cloned()
            .collect()
    } else {
        parse_result.entries
    };

    if entries.is_empty() {
        ctx.print_warning("No entries to export.");
        return Ok(());
    }

    // Format entries
    let formatter = get_formatter(ctx.shell_type);
    let content = formatter.format(&entries, &ctx.config);

    // Write to output file
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, &content)?;

    ctx.print_success(&format!(
        "Exported {} entries to {}",
        entries.len(),
        output.display().to_string().cyan()
    ));

    Ok(())
}
