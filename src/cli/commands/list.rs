//! List command implementation with TUI-style output

use anyhow::Result;
use colored::Colorize;

use super::CommandContext;
use crate::cli::args::EntryTypeArg;
use crate::model::EntryType;

/// Get terminal width, defaulting to 80 if unable to detect
fn get_terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}

/// Calculate dynamic column widths based on terminal width
fn calculate_column_widths(term_width: usize) -> (usize, usize, usize, usize) {
    // Fixed overhead: "│ " prefix (2) + " │" suffix (2) + spaces between columns (3)
    let fixed_overhead = 7;
    let type_width = 10;
    let line_width = 10;

    // Available width for name and value
    let available = term_width.saturating_sub(fixed_overhead + type_width + line_width);

    // Allocate: name gets ~30%, value gets the rest
    let name_width = (available * 30 / 100).clamp(10, 25);
    let value_width = available.saturating_sub(name_width + 1).max(10);

    (type_width, name_width, line_width, value_width)
}

/// Truncate a string to fit within max_width, adding "..." if truncated
fn truncate_value(s: &str, max_width: usize) -> String {
    // Replace newlines with visible representation
    let s = s.replace('\n', "\\n");

    if s.chars().count() <= max_width {
        s
    } else if max_width <= 3 {
        s.chars().take(max_width).collect()
    } else {
        format!("{}...", s.chars().take(max_width - 3).collect::<String>())
    }
}

/// Execute the list command with TUI-style formatting
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

    // Get terminal width and calculate column widths
    let term_width = get_terminal_width();
    let (type_w, name_w, line_w, value_w) = calculate_column_widths(term_width);

    // TUI-style title bar
    let title = msg
        .tui_title
        .replace("{}", &ctx.config_file.display().to_string());
    let title_display = truncate_value(&title, term_width.saturating_sub(6));
    let title_line = format!("  {}  ", title_display);
    let title_padding = "─".repeat(title_line.chars().count());

    println!("{}", format!("┌{}┐", title_padding).blue());
    println!("{}", format!("│{}│", title_line).blue().bold());
    println!("{}", format!("└{}┘", title_padding).blue());
    println!();

    // TUI-style header with entries count
    let entries_title = msg.tui_entries.replace("{}", &entries.len().to_string());
    let header_line_len = term_width.saturating_sub(entries_title.len() + 4);
    println!(
        "┌─ {} {}",
        entries_title.bold(),
        "─".repeat(header_line_len).dimmed()
    );

    // Column headers
    let content_width = type_w + 1 + name_w + 1 + line_w + 1 + value_w;
    println!(
        "│ {:<type_w$} {:<name_w$} {:<line_w$} {:<value_w$} │",
        msg.header_type.bold().cyan(),
        msg.header_name.bold().cyan(),
        "LINE".bold().cyan(),
        msg.header_value.bold().cyan(),
        type_w = type_w,
        name_w = name_w,
        line_w = line_w,
        value_w = value_w
    );
    println!("│ {} │", "─".repeat(content_width).dimmed());

    // Print entries with TUI-style coloring
    for entry in &entries {
        let type_colored = color_entry_type_tui(entry.entry_type);

        // Format line info (single line or range for multi-line)
        let line_info = match (entry.line_number, entry.end_line) {
            (Some(start), Some(end)) if end > start => format!("{}-{}", start, end),
            (Some(line), _) => format!("{}", line),
            (None, _) => "-".to_string(),
        };

        // Truncate name and value based on calculated widths
        let name_display = truncate_value(&entry.name, name_w);
        let value_display = truncate_value(&entry.value, value_w);

        println!(
            "│ {:<type_w$} {:<name_w$} {:<line_w$} {:<value_w$} │",
            type_colored,
            name_display.white(),
            line_info.dimmed(),
            value_display.dimmed(),
            type_w = type_w,
            name_w = name_w,
            line_w = line_w,
            value_w = value_w
        );
    }

    println!("└{}┘", "─".repeat(content_width + 2).dimmed());
    println!();

    // Footer with total and hints
    println!(
        "{}",
        msg.total_entries
            .replace("{}", &entries.len().to_string())
            .dimmed()
    );
    println!("{}", "Run 'wenv tui' for interactive mode".dimmed());

    Ok(())
}

/// Color entry type using TUI color scheme
fn color_entry_type_tui(entry_type: EntryType) -> colored::ColoredString {
    let type_str = format!("{}", entry_type);
    match entry_type {
        EntryType::Alias => type_str.green().bold(),
        EntryType::Function => type_str.blue().bold(),
        EntryType::EnvVar => type_str.yellow().bold(),
        EntryType::Source => type_str.magenta().bold(),
        EntryType::Code => type_str.cyan().bold(),
        EntryType::Comment => type_str.white().bold(),
    }
}
