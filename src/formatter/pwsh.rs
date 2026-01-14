//! PowerShell configuration file formatter

use super::Formatter;
use crate::model::{Config, Entry, EntryType, ShellType};
use crate::utils::dependency;

/// PowerShell configuration file formatter
pub struct PowerShellFormatter;

impl PowerShellFormatter {
    pub fn new() -> Self {
        Self
    }

    /// Find comments that are attached to entries (comments immediately before an entry)
    /// Returns a HashMap mapping entry line numbers to their associated comment entries
    fn find_attached_comments(
        &self,
        entries: &[Entry],
    ) -> std::collections::HashMap<usize, Vec<Entry>> {
        let mut attached_comments = std::collections::HashMap::new();

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
            if next_entry.entry_type == EntryType::Comment
                || next_entry.entry_type == EntryType::Code
            {
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

    fn format_alias(&self, entry: &Entry) -> String {
        format!("Set-Alias {} {}", entry.name, entry.value)
    }

    fn format_env(&self, entry: &Entry) -> String {
        // Use Here-String format for multi-line values
        if entry.value.contains('\n') {
            format!("$env:{} = @\"\n{}\n\"@", entry.name, entry.value)
        } else {
            // Single-line env vars are always quoted
            format!("$env:{} = \"{}\"", entry.name, entry.value)
        }
    }

    fn format_source(&self, entry: &Entry) -> String {
        format!(". {}", entry.value)
    }

    fn format_function(&self, entry: &Entry) -> String {
        // If we have raw_line, preserve original format
        if let Some(ref raw) = entry.raw_line {
            return raw.clone();
        }

        // Otherwise, format as standard function
        format!("function {} {{\n{}\n}}", entry.name, entry.value)
    }
}

impl Default for PowerShellFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for PowerShellFormatter {
    fn format(&self, entries: &[Entry], config: &Config) -> String {
        let mut output = String::new();

        if !config.format.group_by_type {
            let mut sorted_entries: Vec<_> = entries.iter().collect();
            sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

            for entry in sorted_entries {
                if entry.entry_type == EntryType::Code && entry.value.is_empty() {
                    if let (Some(start), Some(end)) = (entry.line_number, entry.end_line) {
                        for _ in 0..(end - start + 1) {
                            output.push('\n');
                        }
                    } else {
                        output.push('\n');
                    }
                } else {
                    output.push_str(&self.format_entry(entry));
                    output.push('\n');
                }
            }
        } else {
            // Find comments attached to entries
            let attached_comments = self.find_attached_comments(entries);

            let mut grouped: std::collections::HashMap<EntryType, Vec<&Entry>> =
                std::collections::HashMap::new();
            let mut type_first_line: std::collections::HashMap<EntryType, usize> =
                std::collections::HashMap::new();

            for entry in entries {
                match entry.entry_type {
                    EntryType::Alias
                    | EntryType::EnvVar
                    | EntryType::Source
                    | EntryType::Function => {
                        grouped.entry(entry.entry_type).or_default().push(entry);
                        let line = entry.line_number.unwrap_or(0);
                        type_first_line
                            .entry(entry.entry_type)
                            .and_modify(|min_line| {
                                if line < *min_line {
                                    *min_line = line;
                                }
                            })
                            .or_insert(line);
                    }
                    _ => {}
                }
            }

            for (entry_type, type_entries) in grouped.iter_mut() {
                if config.format.sort_alphabetically {
                    if *entry_type == EntryType::EnvVar {
                        // Use topological sort for environment variables to respect dependencies
                        let sorted = dependency::topological_sort(type_entries, true);
                        *type_entries = sorted;
                    } else {
                        // Simple alphabetical sort for other types
                        type_entries.sort_by(|a, b| a.name.cmp(&b.name));
                    }
                } else if *entry_type == EntryType::EnvVar {
                    // Even without alphabetical sorting, preserve dependency order
                    let sorted = dependency::topological_sort(type_entries, false);
                    *type_entries = sorted;
                }
            }

            let mut sorted_entries: Vec<_> = entries.iter().collect();
            sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

            let mut output_types: std::collections::HashSet<EntryType> =
                std::collections::HashSet::new();

            for entry in sorted_entries {
                match entry.entry_type {
                    EntryType::Code | EntryType::Comment => {
                        // Skip comments that are attached to other entries (they'll be output with those entries)
                        if entry.entry_type == EntryType::Comment {
                            let entry_line = entry.line_number.unwrap_or(0);
                            let is_attached = attached_comments.values().any(|comments| {
                                comments.iter().any(|c| c.line_number == Some(entry_line))
                            });
                            if is_attached {
                                continue;
                            }
                        }

                        if entry.entry_type == EntryType::Code && entry.value.is_empty() {
                            if let (Some(start), Some(end)) = (entry.line_number, entry.end_line) {
                                for _ in 0..(end - start + 1) {
                                    output.push('\n');
                                }
                            } else {
                                output.push('\n');
                            }
                        } else {
                            output.push_str(&self.format_entry(entry));
                            output.push('\n');
                        }
                    }
                    entry_type @ (EntryType::Alias
                    | EntryType::EnvVar
                    | EntryType::Source
                    | EntryType::Function) => {
                        let current_line = entry.line_number.unwrap_or(0);
                        let first_line = type_first_line.get(&entry_type).copied().unwrap_or(0);

                        if current_line == first_line && !output_types.contains(&entry_type) {
                            output_types.insert(entry_type);

                            if let Some(type_entries) = grouped.get(&entry_type) {
                                for grouped_entry in type_entries {
                                    // Output attached comments before the entry
                                    if let Some(comments) = attached_comments
                                        .get(&grouped_entry.line_number.unwrap_or(0))
                                    {
                                        for comment in comments {
                                            output.push_str(&self.format_entry(comment));
                                            output.push('\n');
                                        }
                                    }

                                    output.push_str(&self.format_entry(grouped_entry));
                                    output.push('\n');
                                }
                            }
                        }
                    }
                }
            }
        }

        output
    }

    fn format_entry(&self, entry: &Entry) -> String {
        match entry.entry_type {
            EntryType::Alias => self.format_alias(entry),
            EntryType::EnvVar => self.format_env(entry),
            EntryType::Source => self.format_source(entry),
            EntryType::Function => self.format_function(entry),
            EntryType::Code | EntryType::Comment => entry
                .raw_line
                .clone()
                .unwrap_or_else(|| entry.value.clone()),
        }
    }

    fn shell_type(&self) -> ShellType {
        ShellType::PowerShell
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_alias() {
        let formatter = PowerShellFormatter::new();
        let entry = Entry::new(EntryType::Alias, "ll".into(), "Get-ChildItem".into());
        assert_eq!(formatter.format_entry(&entry), "Set-Alias ll Get-ChildItem");
    }

    #[test]
    fn test_format_env() {
        let formatter = PowerShellFormatter::new();
        let entry = Entry::new(EntryType::EnvVar, "EDITOR".into(), "code".into());
        assert_eq!(formatter.format_entry(&entry), "$env:EDITOR = \"code\"");
    }

    #[test]
    fn test_format_env_multiline() {
        let formatter = PowerShellFormatter::new();
        let value = "C:\\Program Files\\bin\nD:\\tools\nE:\\bin";
        let entry = Entry::new(EntryType::EnvVar, "PATH".into(), value.into());
        let expected = "$env:PATH = @\"\nC:\\Program Files\\bin\nD:\\tools\nE:\\bin\n\"@";
        assert_eq!(formatter.format_entry(&entry), expected);
    }

    #[test]
    fn test_format_source() {
        let formatter = PowerShellFormatter::new();
        let entry = Entry::new(
            EntryType::Source,
            ".\\aliases.ps1".into(),
            ".\\aliases.ps1".into(),
        );
        assert_eq!(formatter.format_entry(&entry), ". .\\aliases.ps1");
    }
}
