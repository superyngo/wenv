//! Bash configuration file formatter

use crate::model::{Config, Entry, ShellType};
#[cfg(test)]
use crate::model::EntryType;

use super::Formatter;

/// Bash configuration file formatter
pub struct BashFormatter;

impl BashFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BashFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for BashFormatter {
    fn format(&self, entries: &[Entry], _config: &Config) -> String {
        entries.iter()
            .map(|e| self.format_entry(e))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_entry(&self, entry: &Entry) -> String {
        // With the new architecture, value already contains complete raw syntax
        // (including leading comments, keywords, options, quotes)
        entry.value.clone()
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
        let entry = Entry::new(EntryType::Alias, "ll".into(), "alias ll='ls -la'".into());
        assert_eq!(formatter.format_entry(&entry), "alias ll='ls -la'");
    }

    #[test]
    fn test_format_export() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(
            EntryType::EnvVar,
            "EDITOR".into(),
            "export EDITOR=nvim".into(),
        );
        assert_eq!(formatter.format_entry(&entry), "export EDITOR=nvim");
    }

    #[test]
    fn test_format_export_with_spaces() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(
            EntryType::EnvVar,
            "PATH".into(),
            "export PATH=\"$HOME/bin:$PATH\"".into(),
        );
        assert_eq!(
            formatter.format_entry(&entry),
            "export PATH=\"$HOME/bin:$PATH\""
        );
    }

    #[test]
    fn test_format_export_empty() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(EntryType::EnvVar, "EMPTY".into(), "export EMPTY=''".into());
        assert_eq!(formatter.format_entry(&entry), "export EMPTY=''");
    }

    #[test]
    fn test_format_source() {
        let formatter = BashFormatter::new();
        // Source with line number pattern as name (should not append comment)
        let entry = Entry::new(EntryType::Source, "L10".into(), "source ~/.aliases".into());
        assert_eq!(formatter.format_entry(&entry), "source ~/.aliases");
    }

    #[test]
    fn test_format_source_with_name() {
        let formatter = BashFormatter::new();
        // Source with custom name (name is for TUI identification only, not in output)
        let entry = Entry::new(
            EntryType::Source,
            "aliases".into(),
            "source ~/.aliases".into(),
        );
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
        .with_end_line(12);

        let formatted = formatter.format_entry(&entry);
        assert_eq!(formatted, "if true; then\n    echo hi\nfi");
    }

    #[test]
    fn test_format_comment_entry() {
        let formatter = BashFormatter::new();
        let entry = Entry::new(
            EntryType::Comment,
            "L5".into(),
            "# This is a comment".into(),
        )
        .with_line_number(5);

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
            "alias multi='line1
line2
line3'"
                .into(),
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
        let entry = Entry::new(
            EntryType::Alias,
            "multi".into(),
            "alias multi=\"it's line1
line2\""
                .into(),
        );
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
        let entry = Entry::new(
            EntryType::EnvVar,
            "MULTI".into(),
            "export MULTI='line1
line2'"
                .into(),
        );
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
            "export MULTI=\"it's line1
line2\""
                .into(),
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
        // Entry with complete syntax in value (as parsed)
        let entry = Entry::new(EntryType::Alias, "ll".into(), "alias ll='ls -la'".into());
        // Should return value directly
        assert_eq!(formatter.format_entry(&entry), "alias ll='ls -la'");
    }

    #[test]
    fn test_multiline_alias_preserves_raw_line() {
        let formatter = BashFormatter::new();
        // Original multiline alias with complete syntax
        let value = "alias multi='line1
line2
line3'";
        let entry = Entry::new(EntryType::Alias, "multi".into(), value.into());
        // Should return value directly
        assert_eq!(formatter.format_entry(&entry), value);
    }

    #[test]
    fn test_export_preserves_raw_line_when_unedited() {
        let formatter = BashFormatter::new();
        // Entry with complete syntax in value (as parsed)
        let entry = Entry::new(
            EntryType::EnvVar,
            "EDITOR".into(),
            "export EDITOR=nvim".into(),
        );
        // Should return value directly
        assert_eq!(formatter.format_entry(&entry), "export EDITOR=nvim");
    }

    #[test]
    fn test_source_preserves_raw_line_when_unedited() {
        let formatter = BashFormatter::new();
        // Entry with complete syntax in value (as parsed)
        let entry = Entry::new(EntryType::Source, "L10".into(), "source ~/.aliases".into());
        // Should return value directly
        assert_eq!(formatter.format_entry(&entry), "source ~/.aliases");
    }
}
