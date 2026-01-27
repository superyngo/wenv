//! # CommentBlockBuilder
//!
//! Merges adjacent comment lines into a single Comment entry.
//!
//! ## Boundary Rule
//!
//! Adjacent comments are grouped until a **non-comment line** is encountered.
//! Any line that does NOT start with `#` (after trimming whitespace) breaks the block.
//!
//! ## Example
//!
//! ```bash
//! # This is a header comment
//! # that spans multiple lines
//! # with detailed explanation
//!
//! alias my_alias='value'
//! ```
//!
//! Result: One Comment entry spanning lines 1-3, then a blank line, then an alias.
//!
//! ## Naming Convention
//!
//! The entry name shows the line range:
//! - Single line: `#L5`
//! - Multiple lines: `#L5-L8`
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut builder = CommentBlockBuilder::new(1, "# First line");
//! builder.add_line("# Second line");
//! builder.add_line("# Third line");
//!
//! // Non-comment line encountered, flush the builder
//! let entry = builder.build();
//! // entry.name = "#L1-L3"
//! // entry.line_number = Some(1)
//! // entry.end_line = Some(3)
//! ```
//!
//! ## Integration with Parser
//!
//! The parser should:
//! 1. Start a new `CommentBlockBuilder` when encountering a `#` line
//! 2. Continue adding lines while they start with `#`
//! 3. Call `build()` when a non-`#` line is encountered
//! 4. Process the non-`#` line normally

use crate::model::{Entry, EntryType};

/// Builder for accumulating adjacent comment lines.
#[derive(Debug)]
pub struct CommentBlockBuilder {
    /// Line number where the comment block starts (1-based)
    pub start_line: usize,
    /// Accumulated comment lines (including the `#` prefix)
    pub lines: Vec<String>,
}

impl CommentBlockBuilder {
    /// Check if a line is a comment (starts with `#` after trimming).
    ///
    /// # Arguments
    ///
    /// - `line`: The line to check
    ///
    /// # Returns
    ///
    /// `true` if the trimmed line starts with `#`
    pub fn is_comment_line(line: &str) -> bool {
        line.trim().starts_with('#')
    }

    /// Check if a line is a standalone comment (not an inline comment).
    ///
    /// A standalone comment is a line where the first non-whitespace
    /// character is `#`. This excludes lines like `alias x='y' # comment`.
    pub fn is_standalone_comment(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with('#') && !trimmed.is_empty()
    }

    /// Create a new builder with the first comment line.
    ///
    /// # Arguments
    ///
    /// - `start_line`: 1-based line number
    /// - `first_line`: The first comment line (should start with `#`)
    pub fn new(start_line: usize, first_line: &str) -> Self {
        Self {
            start_line,
            lines: vec![first_line.to_string()],
        }
    }

    /// Add another comment line to the block.
    pub fn add_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    /// Get the number of lines in this comment block.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Extract the comment text (without `#` prefixes).
    ///
    /// Each line has its leading `#` and whitespace stripped,
    /// then all lines are joined with newlines.
    pub fn extract_text(&self) -> String {
        self.lines
            .iter()
            .map(|line| {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix('#') {
                    rest.trim_start().to_string()
                } else {
                    trimmed.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build the final Comment entry with line range.
    ///
    /// # Returns
    ///
    /// A `Comment` entry with:
    /// - `name`: `#L{start}` or `#L{start}-L{end}`
    /// - `value`: The raw comment lines (preserving # prefix and whitespace)
    /// - `line_number` and `end_line`: The line range
    /// - `raw_line`: Original lines joined with newlines (same as value)
    pub fn build(self) -> Entry {
        let end_line = self.start_line + self.lines.len().saturating_sub(1);
        let name = if self.lines.len() <= 1 {
            format!("#L{}", self.start_line)
        } else {
            format!("#L{}-L{}", self.start_line, end_line)
        };

        let raw = self.lines.join("\n");
        // Changed: value now preserves original format with # prefix
        let value = raw.clone();

        Entry::new(EntryType::Comment, name, value)
            .with_line_number(self.start_line)
            .with_end_line(end_line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_comment_line() {
        assert!(CommentBlockBuilder::is_comment_line("# comment"));
        assert!(CommentBlockBuilder::is_comment_line("  # indented comment"));
        assert!(CommentBlockBuilder::is_comment_line("#"));
        assert!(!CommentBlockBuilder::is_comment_line("alias x='y'"));
        assert!(!CommentBlockBuilder::is_comment_line(
            "alias x='y' # inline"
        ));
        assert!(!CommentBlockBuilder::is_comment_line(""));
    }

    #[test]
    fn test_is_standalone_comment() {
        assert!(CommentBlockBuilder::is_standalone_comment("# comment"));
        assert!(CommentBlockBuilder::is_standalone_comment("  # indented"));
        assert!(!CommentBlockBuilder::is_standalone_comment(
            "alias x='y' # inline"
        ));
        assert!(!CommentBlockBuilder::is_standalone_comment(""));
    }

    #[test]
    fn test_single_line_comment() {
        let builder = CommentBlockBuilder::new(5, "# This is a comment");
        let entry = builder.build();

        assert_eq!(entry.name, "#L5");
        // Changed: value now includes # prefix
        assert_eq!(entry.value, "# This is a comment");
        assert_eq!(entry.line_number, Some(5));
        assert_eq!(entry.end_line, Some(5));
        assert_eq!(entry.entry_type, EntryType::Comment);
    }

    #[test]
    fn test_multi_line_comment() {
        let mut builder = CommentBlockBuilder::new(10, "# Header comment");
        builder.add_line("# Second line");
        builder.add_line("# Third line");
        let entry = builder.build();

        assert_eq!(entry.name, "#L10-L12");
        assert_eq!(entry.line_number, Some(10));
        assert_eq!(entry.end_line, Some(12));
        // Changed: value now includes # prefix
        assert!(entry.value.contains("# Header comment"));
        assert!(entry.value.contains("# Second line"));
        assert!(entry.value.contains("# Third line"));
    }

    #[test]
    fn test_extract_text() {
        let mut builder = CommentBlockBuilder::new(1, "# Line 1");
        builder.add_line("#Line 2");
        builder.add_line("#  Line 3");

        let text = builder.extract_text();
        assert_eq!(text, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_raw_line_preserved() {
        let mut builder = CommentBlockBuilder::new(1, "  # Indented");
        builder.add_line("# Normal");
        let entry = builder.build();

        assert!(&entry.value.contains("  # Indented"));
        assert!(&entry.value.contains("# Normal"));
    }
}
