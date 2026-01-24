//! Bash configuration file formatter

use crate::formatter::find_attached_comments;
use crate::model::{Config, Entry, EntryType, ShellType};
use crate::utils::dependency;

use super::Formatter;

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
        let value = &entry.value;

        // Multi-line values: prefer single quotes (safest Bash multi-line syntax)
        if value.contains('\n') {
            if !value.contains('\'') {
                // No single quotes in value, safe to use single quotes
                return format!("alias {}='{}'", entry.name, value);
            }
            // Value contains single quotes, use double quotes with escaping
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`");
            return format!("alias {}=\"{}\"", entry.name, escaped);
        }

        // Single-line values: check if quotes are needed
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

        // Multi-line values: prefer single quotes to match parser (which only detects single-quote multi-line)
        if value.contains('\n') {
            if !value.contains('\'') {
                // No single quotes in value, safe to use single quotes
                return format!("export {}='{}'", entry.name, value);
            }
            // Value contains single quotes, use double quotes with escaping
            let escaped = value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('`', "\\`")
                .replace('$', "\\$"); // Escape $ in multi-line double-quoted strings
            return format!("export {}=\"{}\"", entry.name, escaped);
        }

        // Single-line values
        if value.is_empty() {
            // Empty value: use single quotes for clarity
            format!("export {}=''", entry.name)
        } else if value.contains(' ') || value.contains('$') {
            format!("export {}=\"{}\"", entry.name, value)
        } else {
            format!("export {}={}", entry.name, value)
        }
    }

    fn format_source(&self, entry: &Entry) -> String {
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
        match entry.entry_type {
            // For Alias/EnvVar/Source: prioritize raw_line if available (unedited entries)
            // This preserves original formatting for entries that haven't been modified
            EntryType::Alias => {
                if let Some(ref raw) = entry.raw_line {
                    return raw.clone();
                }
                self.format_alias(entry)
            }
            EntryType::EnvVar => {
                if let Some(ref raw) = entry.raw_line {
                    return raw.clone();
                }
                self.format_export(entry)
            }
            EntryType::Source => {
                if let Some(ref raw) = entry.raw_line {
                    return raw.clone();
                }
                self.format_source(entry)
            }
            // Function: continues to use format_function which handles raw_line internally
            // (applies indentation formatting to body)
            EntryType::Function => self.format_function(entry, &self.indent_style),
            // Code/Comment: always use raw_line if available
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
    fn test_format_export_empty() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(EntryType::EnvVar, "EMPTY".into(), "".into());
        assert_eq!(formatter.format_entry(&entry), "export EMPTY=''");
    }

    #[test]
    fn test_format_source() {
        let formatter = BashFormatter::new();
        // Source with line number pattern as name (should not append comment)
        let entry = Entry::new(EntryType::Source, "L10".into(), "~/.aliases".into());
        assert_eq!(formatter.format_entry(&entry), "source ~/.aliases");
    }

    #[test]
    fn test_format_source_with_name() {
        let formatter = BashFormatter::new();
        // Source with custom name (name is for TUI identification only, not in output)
        let entry = Entry::new(EntryType::Source, "aliases".into(), "~/.aliases".into());
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

    #[test]
    fn test_comment_follows_entry_when_grouped() {
        use crate::model::ShellType;

        let original_content = r#"# Git shortcuts
alias gs='git status'
# Directory shortcuts
alias ll='ls -la'

# Environment variables
export EDITOR=nvim
"#;

        // Parse the content
        let parser = crate::parser::get_parser(ShellType::Bash);
        let result = parser.parse(original_content);

        // Format with grouping enabled (default)
        let formatter = BashFormatter::new();
        let config = Config::default();
        let formatted = formatter.format(&result.entries, &config);

        // Debug: print the formatted output
        println!("Formatted output:\n{}", formatted);

        // Comments should follow their entries when sorted alphabetically
        // The order should be: gs (with "Git shortcuts"), ll (with "Directory shortcuts")
        // followed by env vars

        // Check that comments appear before their respective entries
        let ll_pos = formatted
            .find("alias ll=")
            .expect("ll alias should be present");
        let gs_pos = formatted
            .find("alias gs=")
            .expect("gs alias should be present");
        let dir_comment_pos = formatted
            .find("# Directory shortcuts")
            .expect("Directory comment should be present");
        let git_comment_pos = formatted
            .find("# Git shortcuts")
            .expect("Git comment should be present");

        // Directory comment should be right before ll
        assert!(dir_comment_pos < ll_pos);
        // Git comment should be right before gs
        assert!(git_comment_pos < gs_pos);

        // Since alphabetically gs comes before ll, check ordering
        assert!(gs_pos < ll_pos, "gs should come before ll alphabetically");
        assert!(
            git_comment_pos < dir_comment_pos,
            "Git comment should come before Directory comment"
        );
    }

    #[test]
    fn test_standalone_comments_stay_in_place() {
        use crate::model::ShellType;

        let original_content = r#"# This is a standalone comment

alias test='echo test'
"#;

        let parser = crate::parser::get_parser(ShellType::Bash);
        let result = parser.parse(original_content);

        let formatter = BashFormatter::new();
        let config = Config::default();
        let formatted = formatter.format(&result.entries, &config);

        // Standalone comment (with blank line after) should stay in its original position
        assert!(formatted.contains("# This is a standalone comment"));
    }

    #[test]
    fn test_format_multiline_alias_without_single_quotes() {
        let formatter = BashFormatter::new();
        // Entry without raw_line (edited entry) - should use format_alias
        let entry = Entry::new(
            EntryType::Alias,
            "multi".into(),
            "line1\nline2\nline3".into(),
        );
        // Should use single quotes for multiline without single quotes in value
        assert_eq!(
            formatter.format_entry(&entry),
            "alias multi='line1\nline2\nline3'"
        );
    }

    #[test]
    fn test_format_multiline_alias_with_single_quotes() {
        let formatter = BashFormatter::new();
        // Entry without raw_line (edited entry) with single quotes in value
        let entry = Entry::new(EntryType::Alias, "multi".into(), "it's line1\nline2".into());
        // Should use double quotes with escaping when value contains single quotes
        assert_eq!(
            formatter.format_entry(&entry),
            "alias multi=\"it's line1\nline2\""
        );
    }

    #[test]
    fn test_format_multiline_export() {
        let formatter = BashFormatter::new();
        // Entry without raw_line (edited entry)
        let entry = Entry::new(EntryType::EnvVar, "MULTI".into(), "line1\nline2".into());
        // Should use single quotes for multiline export (matches parser)
        assert_eq!(
            formatter.format_entry(&entry),
            "export MULTI='line1\nline2'"
        );
    }

    #[test]
    fn test_format_multiline_export_with_single_quotes() {
        let formatter = BashFormatter::new();
        // Entry with single quotes in value
        let entry = Entry::new(
            EntryType::EnvVar,
            "MULTI".into(),
            "it's line1\nline2".into(),
        );
        // Should use double quotes with escaping when value contains single quotes
        assert_eq!(
            formatter.format_entry(&entry),
            "export MULTI=\"it's line1\nline2\""
        );
    }

    #[test]
    fn test_alias_preserves_raw_line_when_unedited() {
        let formatter = BashFormatter::new();
        // Entry with raw_line (unedited entry from parser)
        let entry = Entry::new(EntryType::Alias, "ll".into(), "ls -la".into())
            .with_raw_line("alias ll='ls -la'".into());
        // Should use raw_line directly, not format_alias
        assert_eq!(formatter.format_entry(&entry), "alias ll='ls -la'");
    }

    #[test]
    fn test_multiline_alias_preserves_raw_line() {
        let formatter = BashFormatter::new();
        // Original multiline alias with custom formatting
        let raw = "alias multi='line1\nline2\nline3'";
        let entry = Entry::new(
            EntryType::Alias,
            "multi".into(),
            "line1\nline2\nline3".into(),
        )
        .with_raw_line(raw.into());
        // Should preserve the original raw_line exactly
        assert_eq!(formatter.format_entry(&entry), raw);
    }

    #[test]
    fn test_export_preserves_raw_line_when_unedited() {
        let formatter = BashFormatter::new();
        // Entry with raw_line (unedited entry from parser)
        let entry = Entry::new(EntryType::EnvVar, "EDITOR".into(), "nvim".into())
            .with_raw_line("export EDITOR=nvim".into());
        // Should use raw_line directly
        assert_eq!(formatter.format_entry(&entry), "export EDITOR=nvim");
    }

    #[test]
    fn test_source_preserves_raw_line_when_unedited() {
        let formatter = BashFormatter::new();
        // Entry with raw_line (unedited entry from parser)
        let entry = Entry::new(EntryType::Source, "L10".into(), "~/.aliases".into())
            .with_raw_line("source ~/.aliases".into());
        // Should use raw_line directly
        assert_eq!(formatter.format_entry(&entry), "source ~/.aliases");
    }

    #[test]
    fn test_format_order_from_config() {
        use crate::model::{ShellType, TypeOrder};

        let original_content = r#"alias test='echo test'
export EDITOR=nvim
source ~/.profile
greet() { echo hello; }
"#;

        let parser = crate::parser::get_parser(ShellType::Bash);
        let result = parser.parse(original_content);

        let formatter = BashFormatter::new();

        // Test with custom order: env, alias, func, source (default order)
        let mut config = Config::default();
        config.format.order = TypeOrder {
            types: vec!["env".into(), "alias".into(), "func".into(), "source".into()],
        };
        let formatted = formatter.format(&result.entries, &config);

        let env_pos = formatted
            .find("export EDITOR")
            .expect("env should be present");
        let alias_pos = formatted
            .find("alias test")
            .expect("alias should be present");
        let func_pos = formatted.find("greet()").expect("func should be present");
        let source_pos = formatted
            .find("source ~/.profile")
            .expect("source should be present");

        assert!(
            env_pos < alias_pos,
            "env should come before alias (order: env, alias, func, source)"
        );
        assert!(
            alias_pos < func_pos,
            "alias should come before func (order: env, alias, func, source)"
        );
        assert!(
            func_pos < source_pos,
            "func should come before source (order: env, alias, func, source)"
        );

        // Test with reversed order: source, func, alias, env
        config.format.order = TypeOrder {
            types: vec!["source".into(), "func".into(), "alias".into(), "env".into()],
        };
        let formatted = formatter.format(&result.entries, &config);

        let env_pos = formatted
            .find("export EDITOR")
            .expect("env should be present");
        let alias_pos = formatted
            .find("alias test")
            .expect("alias should be present");
        let func_pos = formatted.find("greet()").expect("func should be present");
        let source_pos = formatted
            .find("source ~/.profile")
            .expect("source should be present");

        assert!(
            source_pos < func_pos,
            "source should come before func (order: source, func, alias, env)"
        );
        assert!(
            func_pos < alias_pos,
            "func should come before alias (order: source, func, alias, env)"
        );
        assert!(
            alias_pos < env_pos,
            "alias should come before env (order: source, func, alias, env)"
        );
    }
}
