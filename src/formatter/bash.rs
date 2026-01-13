//! Bash configuration file formatter

use super::Formatter;
use crate::model::{Config, Entry, EntryType, ShellType};
use crate::utils::dependency;

/// Bash configuration file formatter
pub struct BashFormatter {
    /// Indentation style (e.g., "    " for 4 spaces, "\t" for tab)
    indent_style: String,
}

impl BashFormatter {
    pub fn new() -> Self {
        Self {
            indent_style: "    ".to_string(), // Default to 4 spaces
        }
    }

    /// Create a formatter with a specific indent style
    pub fn with_indent_style(indent_style: String) -> Self {
        Self { indent_style }
    }

    fn format_alias(&self, entry: &Entry) -> String {
        // Check if value needs quotes
        let value = &entry.value;
        if value.contains(' ') || value.contains('$') || value.contains('"') {
            if value.contains('\'') {
                format!("alias {}=\"{}\"", entry.name, value.replace('"', "\\\""))
            } else {
                format!("alias {}='{}'", entry.name, value)
            }
        } else {
            format!("alias {}='{}'", entry.name, value)
        }
    }

    fn format_export(&self, entry: &Entry) -> String {
        let value = &entry.value;
        if value.contains(' ') || value.contains('$') {
            format!("export {}=\"{}\"", entry.name, value)
        } else {
            format!("export {}={}", entry.name, value)
        }
    }

    fn format_source(&self, entry: &Entry) -> String {
        // Value contains the path (name is now line number based like "L10")
        format!("source {}", entry.value)
    }

    fn format_function(&self, entry: &Entry, indent_style: &str) -> String {
        // If we have raw_line, use it but apply formatting to the body
        if let Some(ref raw) = entry.raw_line {
            return self.format_raw_function(raw, indent_style);
        }

        // Build from value (function body only)
        // Apply indentation to body and wrap in function declaration
        let body = super::indent::format_body_preserve_relative(&entry.value, indent_style);
        format!("{}() {{\n{}\n}}", entry.name, body)
    }

    /// Format a raw function definition, applying indentation to the body
    fn format_raw_function(&self, raw: &str, indent_style: &str) -> String {
        let lines: Vec<&str> = raw.lines().collect();

        if lines.len() <= 2 {
            // Single line or minimal function, return as-is
            return raw.to_string();
        }

        // Extract body (lines between first and last)
        let body = lines[1..lines.len() - 1].join("\n");
        let formatted_body = super::indent::format_body_preserve_relative(&body, indent_style);

        // Reconstruct with formatted body
        format!(
            "{}\n{}\n{}",
            lines[0],
            formatted_body,
            lines.last().unwrap()
        )
    }
}

impl Default for BashFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for BashFormatter {
    fn format(&self, entries: &[Entry], config: &Config) -> String {
        let mut output = String::new();

        if !config.format.group_by_type {
            // Output in original order (by line number)
            let mut sorted_entries: Vec<_> = entries.iter().collect();
            sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

            for entry in sorted_entries {
                if entry.entry_type == EntryType::Code && entry.value.is_empty() {
                    // Handle grouped blank lines
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
            // Strategy: Keep Comment/Code in original positions, only reorder Alias/EnvVar/Source/Function

            // Group parseable entries by type
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

            // Sort all entries by line number
            let mut sorted_entries: Vec<_> = entries.iter().collect();
            sorted_entries.sort_by_key(|e| e.line_number.unwrap_or(0));

            let mut output_types: std::collections::HashSet<EntryType> =
                std::collections::HashSet::new();

            // Iterate through entries in line number order
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
            EntryType::EnvVar => self.format_export(entry),
            EntryType::Source => self.format_source(entry),
            EntryType::Function => self.format_function(entry, &self.indent_style),
            EntryType::Code | EntryType::Comment => entry
                .raw_line
                .clone()
                .unwrap_or_else(|| entry.value.clone()),
        }
    }

    fn shell_type(&self) -> ShellType {
        ShellType::Bash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_alias() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(EntryType::Alias, "ll".into(), "ls -la".into());
        assert_eq!(formatter.format_entry(&entry), "alias ll='ls -la'");
    }

    #[test]
    fn test_format_export() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(EntryType::EnvVar, "EDITOR".into(), "nvim".into());
        assert_eq!(formatter.format_entry(&entry), "export EDITOR=nvim");
    }

    #[test]
    fn test_format_export_with_spaces() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into());
        assert_eq!(
            formatter.format_entry(&entry),
            "export PATH=\"$HOME/bin:$PATH\""
        );
    }

    #[test]
    fn test_format_source() {
        let formatter = BashFormatter::new();
        // Source now uses line number as name, path as value
        let entry = Entry::new(EntryType::Source, "L10".into(), "~/.aliases".into());
        assert_eq!(formatter.format_entry(&entry), "source ~/.aliases");
    }

    #[test]
    fn test_complete_file_restoration() {
        use crate::model::ShellType;

        let original_content = r#"# Git aliases
alias gs='git status'
alias gd='git diff'

# Environment
export EDITOR=nvim

if [ -f ~/.bashrc.local ]; then
    source ~/.bashrc.local
fi

greet() {
    echo "Hello"
}
"#;

        // Parse the content using the parser factory
        let parser = crate::parser::get_parser(ShellType::Bash);
        let result = parser.parse(original_content);

        // Format it back
        let formatter = BashFormatter::new();
        let config = Config::default();
        let formatted = formatter.format(&result.entries, &config);

        // The formatted output should preserve all content
        // Check that key elements are present
        assert!(formatted.contains("# Git aliases"));
        assert!(formatted.contains("alias gs='git status'"));
        assert!(formatted.contains("alias gd='git diff'"));
        assert!(formatted.contains("# Environment"));
        assert!(formatted.contains("export EDITOR=nvim"));
        assert!(formatted.contains("if [ -f ~/.bashrc.local ]; then"));
        assert!(formatted.contains("source ~/.bashrc.local"));
        assert!(formatted.contains("fi"));
        assert!(formatted.contains("greet() {"));
        assert!(formatted.contains("echo \"Hello\""));
    }

    #[test]
    fn test_format_code_entry() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(
            EntryType::Code,
            "L10-L12".into(),
            "if true; then\n    echo hi\nfi".into(),
        )
        .with_line_number(10)
        .with_end_line(12)
        .with_raw_line("if true; then\n    echo hi\nfi".into());

        let formatted = formatter.format_entry(&entry);
        assert_eq!(formatted, "if true; then\n    echo hi\nfi");
    }

    #[test]
    fn test_format_comment_entry() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(EntryType::Comment, "L5".into(), "This is a comment".into())
            .with_line_number(5)
            .with_raw_line("# This is a comment".into());

        let formatted = formatter.format_entry(&entry);
        assert_eq!(formatted, "# This is a comment");
    }
}
