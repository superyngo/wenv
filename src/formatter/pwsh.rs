//! PowerShell configuration file formatter

use crate::formatter::find_attached_comments;
use crate::model::{Config, Entry, EntryType, ShellType};
use crate::utils::dependency;

use super::Formatter;

/// PowerShell configuration file formatter
pub struct PowerShellFormatter;

impl PowerShellFormatter {
    pub fn new() -> Self {
        Self
    }

    #[allow(dead_code)]
    fn format_alias(&self, entry: &Entry) -> String {
        format!("Set-Alias {} '{}'", entry.name, entry.value)
    }

    #[allow(dead_code)]
    fn format_env(&self, entry: &Entry) -> String {
        // Use Here-String format for multi-line values
        if entry.value.contains('\n') {
            format!("$env:{} = @\"\n{}\n\"@", entry.name, entry.value)
        } else {
            // Single-line env vars are always quoted
            format!("$env:{} = \"{}\"", entry.name, entry.value)
        }
    }

    #[allow(dead_code)]
    fn format_source(&self, entry: &Entry) -> String {
        format!(". {}", entry.value)
    }

    #[allow(dead_code)]
    fn format_function(&self, entry: &Entry) -> String {
        // With the new architecture, value already contains complete syntax
        entry.value.clone()
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
            // Group entries by type (for format command with grouping enabled)
            // Strategy: Output types in configured order, keep Comment/Code in original positions

            // Find comments attached to entries
            let attached_comments = find_attached_comments(entries);

            // Group parseable entries by type
            let mut grouped: std::collections::HashMap<EntryType, Vec<&Entry>> =
                std::collections::HashMap::new();

            for entry in entries {
                match entry.entry_type {
                    EntryType::Alias
                    | EntryType::EnvVar
                    | EntryType::Source
                    | EntryType::Function => {
                        grouped.entry(entry.entry_type).or_default().push(entry);
                    }
                    _ => {}
                }
            }

            // Sort grouped entries
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

            // Build type order from config
            let type_order: Vec<EntryType> = config
                .format
                .order
                .types
                .iter()
                .filter_map(|s| s.parse::<EntryType>().ok())
                .collect();

            // Collect Comment/Code entries for output in original order
            let mut code_comments: Vec<&Entry> = entries
                .iter()
                .filter(|e| e.entry_type == EntryType::Code || e.entry_type == EntryType::Comment)
                .filter(|e| {
                    // Skip comments attached to other entries
                    if e.entry_type == EntryType::Comment {
                        let entry_line = e.line_number.unwrap_or(0);
                        !attached_comments.values().any(|comments| {
                            comments.iter().any(|c| c.line_number == Some(entry_line))
                        })
                    } else {
                        true
                    }
                })
                .collect();
            code_comments.sort_by_key(|e| e.line_number.unwrap_or(0));

            // Output Code/Comment entries that appear before any structured entries
            let first_structured_line = entries
                .iter()
                .filter(|e| {
                    matches!(
                        e.entry_type,
                        EntryType::Alias
                            | EntryType::EnvVar
                            | EntryType::Source
                            | EntryType::Function
                    )
                })
                .filter_map(|e| e.line_number)
                .min()
                .unwrap_or(usize::MAX);

            for entry in &code_comments {
                let line = entry.line_number.unwrap_or(0);
                if line < first_structured_line {
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
            }

            // Output structured entries by configured type order
            let blank_lines = config.format.blank_lines_between_groups;
            let mut first_group = true;

            for entry_type in &type_order {
                if let Some(type_entries) = grouped.get(entry_type) {
                    if !type_entries.is_empty() {
                        // Add blank lines between groups
                        if !first_group {
                            for _ in 0..blank_lines {
                                output.push('\n');
                            }
                        }
                        first_group = false;

                        for grouped_entry in type_entries {
                            // Output attached comments before the entry
                            if let Some(comments) =
                                attached_comments.get(&grouped_entry.line_number.unwrap_or(0))
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

            // Output remaining Code/Comment entries (those not before first structured entry)
            // These are output after all structured entries have been grouped
            for entry in &code_comments {
                let line = entry.line_number.unwrap_or(0);
                if line >= first_structured_line {
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
            }
        }

        output
    }

    fn format_entry(&self, entry: &Entry) -> String {
        // With the new architecture, value already contains complete raw syntax
        entry.value.clone()
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
        let entry = Entry::new(
            EntryType::Alias,
            "ll".into(),
            "Set-Alias ll 'Get-ChildItem'".into(),
        );
        assert_eq!(
            formatter.format_entry(&entry),
            "Set-Alias ll 'Get-ChildItem'"
        );
    }

    #[test]
    fn test_format_env() {
        let formatter = PowerShellFormatter::new();
        let entry = Entry::new(
            EntryType::EnvVar,
            "EDITOR".into(),
            "$env:EDITOR = \"code\"".into(),
        );
        assert_eq!(formatter.format_entry(&entry), "$env:EDITOR = \"code\"");
    }

    #[test]
    fn test_format_env_multiline() {
        let formatter = PowerShellFormatter::new();
        // Entry with complete syntax (Raw Value Architecture)
        let value = r#"$env:PATH = @"
C:\Program Files\bin
D:\tools
E:\bin
"@"#;
        let entry = Entry::new(EntryType::EnvVar, "PATH".into(), value.into());
        // Formatter returns value directly
        assert_eq!(formatter.format_entry(&entry), value);
    }

    #[test]
    fn test_format_source() {
        let formatter = PowerShellFormatter::new();
        // Entry with complete syntax (Raw Value Architecture)
        let entry = Entry::new(EntryType::Source, "L10".into(), ". .\\aliases.ps1".into());
        assert_eq!(formatter.format_entry(&entry), ". .\\aliases.ps1");
    }

    #[test]
    fn test_format_source_with_name() {
        let formatter = PowerShellFormatter::new();
        // Entry with complete syntax (Raw Value Architecture)
        let entry = Entry::new(
            EntryType::Source,
            "aliases".into(),
            ". .\\aliases.ps1".into(),
        );
        assert_eq!(formatter.format_entry(&entry), ". .\\aliases.ps1");
    }
}
