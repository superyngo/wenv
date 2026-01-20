//! Formatter module for shell configuration files

mod bash;
pub mod indent;
mod pwsh;

pub use bash::BashFormatter;
pub use pwsh::PowerShellFormatter;

use crate::model::{Config, Entry, EntryType, ShellType};
use std::collections::HashMap;

/// Trait for shell configuration formatters
pub trait Formatter {
    /// Format entries into shell configuration format
    fn format(&self, entries: &[Entry], config: &Config) -> String;

    /// Format a single entry
    fn format_entry(&self, entry: &Entry) -> String;

    /// Get the shell type this formatter handles
    fn shell_type(&self) -> ShellType;
}

/// Get a formatter for the specified shell type
pub fn get_formatter(shell_type: ShellType) -> Box<dyn Formatter> {
    match shell_type {
        ShellType::Bash | ShellType::Zsh => Box::new(BashFormatter::new()),
        ShellType::PowerShell => Box::new(PowerShellFormatter::new()),
    }
}

/// Find comments that are attached to entries (comments immediately before an entry).
/// Returns a HashMap mapping entry line numbers to their associated comment entries.
///
/// This is a common utility used by both Bash and PowerShell formatters.
pub fn find_attached_comments(entries: &[Entry]) -> HashMap<usize, Vec<Entry>> {
    let mut attached_comments = HashMap::new();

    // Sort entries by line number
    let mut sorted_entries: Vec<_> = entries.iter().collect();
    sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

    for i in 0..sorted_entries.len() {
        let entry = sorted_entries[i];

        // Only process Comment entries
        if entry.entry_type != EntryType::Comment {
            continue;
        }

        // Check if there's a next entry
        if i + 1 >= sorted_entries.len() {
            continue;
        }

        let next_entry = sorted_entries[i + 1];

        // Skip if next entry is also a Comment or Code (these stay in place)
        if next_entry.entry_type == EntryType::Comment || next_entry.entry_type == EntryType::Code {
            continue;
        }

        // Check if comment is immediately before the next entry
        // Comment should end right before the next entry starts
        if let (Some(comment_end), Some(next_line)) = (entry.end_line, next_entry.line_number) {
            if comment_end + 1 == next_line {
                // This comment is attached to the next entry
                attached_comments
                    .entry(next_line)
                    .or_insert_with(Vec::new)
                    .push(entry.clone());
            }
        }
    }

    attached_comments
}
