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

    fn format_alias(&self, entry: &Entry) -> String {
        format!("Set-Alias {} {}", entry.name, entry.value)
    }

    fn format_env(&self, entry: &Entry) -> String {
        // PowerShell env vars are always quoted
        format!("$env:{} = \"{}\"", entry.name, entry.value)
    }

    fn format_source(&self, entry: &Entry) -> String {
        format!(". {}", entry.name)
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
