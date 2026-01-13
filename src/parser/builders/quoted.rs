//! # QuotedValueBuilder
//!
//! Handles multi-line values enclosed in single quotes for aliases and environment variables.
//!
//! ## Supported Entry Types
//!
//! - `Alias`: `alias name='multi\nline\nvalue'`
//! - `EnvVar`: `export VAR='multi\nline\nvalue'`
//!
//! ## Detection Logic
//!
//! A multi-line quoted value is detected when:
//! 1. The line contains an **odd** number of unescaped single quotes
//! 2. Subsequent lines are accumulated until quote count becomes **even**
//!
//! ## Example
//!
//! ```bash
//! alias complex='echo "line1"
//! echo "line2"
//! echo "line3"'
//! ```
//!
//! This spans lines 1-3 and will be parsed as a single Alias entry.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // First, check if line starts a multi-line value
//! if QuotedValueBuilder::has_unclosed_single_quote(line) {
//!     let mut builder = QuotedValueBuilder::new("myalias".into(), 5, line);
//!     
//!     // Continue adding lines until complete
//!     while !builder.is_complete() {
//!         builder.add_line(next_line);
//!     }
//!     
//!     let entry = builder.build(EntryType::Alias);
//! }
//! ```
//!
//! ## Note on Double Quotes
//!
//! This builder only handles single-quote boundaries. Double quotes inside
//! single-quoted strings are treated as literal characters.

use crate::model::{Entry, EntryType};

/// Builder for multi-line quoted values (alias, env var).
///
/// Accumulates lines until the single-quote boundary is closed.
#[derive(Debug)]
pub struct QuotedValueBuilder {
    /// Entry name (alias name or variable name)
    pub name: String,
    /// Line number where the entry starts (1-based)
    pub start_line: usize,
    /// Accumulated raw lines
    pub lines: Vec<String>,
}

impl QuotedValueBuilder {
    /// Count single quotes in a line that are outside double-quoted strings.
    ///
    /// This correctly handles cases like:
    /// - `alias x='hello'` → 2 quotes
    /// - `alias x='say "hi"'` → 2 quotes (quotes inside are literal)
    /// - `alias x='it` → 1 quote (unclosed)
    ///
    /// # Arguments
    ///
    /// - `line`: The line to analyze
    ///
    /// # Returns
    ///
    /// Number of single quotes outside double-quoted regions.
    pub fn count_single_quotes(line: &str) -> usize {
        let mut count = 0;
        let mut in_double = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                // Handle escape sequences
                '\\' => {
                    // Skip the next character (it's escaped)
                    chars.next();
                }
                // Toggle double-quote state (but not inside single quotes)
                '"' if !in_double => {
                    // We're entering a double-quoted region
                    in_double = true;
                }
                '"' if in_double => {
                    // We're exiting a double-quoted region
                    in_double = false;
                }
                // Count single quotes only outside double-quoted regions
                '\'' if !in_double => {
                    count += 1;
                }
                _ => {}
            }
        }

        count
    }

    /// Check if a line has an unclosed single quote.
    ///
    /// Returns `true` if the number of single quotes (outside double quotes)
    /// is odd, indicating the quote spans to the next line.
    pub fn has_unclosed_single_quote(line: &str) -> bool {
        Self::count_single_quotes(line) % 2 == 1
    }

    /// Create a new builder with the first line.
    ///
    /// # Arguments
    ///
    /// - `name`: The entry name (alias or variable name)
    /// - `start_line`: 1-based line number
    /// - `first_line`: The first line of content
    pub fn new(name: String, start_line: usize, first_line: &str) -> Self {
        Self {
            name,
            start_line,
            lines: vec![first_line.to_string()],
        }
    }

    /// Add a continuation line.
    pub fn add_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    /// Check if the accumulated content has balanced quotes.
    ///
    /// Returns `true` when the total number of single quotes
    /// (across all lines) is even, meaning the quoted value is complete.
    pub fn is_complete(&self) -> bool {
        let total_quotes: usize = self
            .lines
            .iter()
            .map(|l| Self::count_single_quotes(l))
            .sum();
        total_quotes.is_multiple_of(2)
    }

    /// Get the current line count.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Extract the value portion from the accumulated lines.
    ///
    /// This finds the content between the first `='` and the final `'`.
    pub fn extract_value(&self) -> String {
        let full_content = self.lines.join("\n");

        // Find the start of the value (after =' or =")
        if let Some(eq_pos) = full_content.find("='") {
            let start = eq_pos + 2;
            // Find the last single quote
            if let Some(end) = full_content.rfind('\'') {
                if end > start {
                    return full_content[start..end].to_string();
                }
            }
            // If no closing quote found, return everything after ='
            return full_content[start..].to_string();
        }

        // Fallback: return the full content
        full_content
    }

    /// Build the final Entry with line range.
    ///
    /// # Arguments
    ///
    /// - `entry_type`: `EntryType::Alias` or `EntryType::EnvVar`
    ///
    /// # Returns
    ///
    /// An `Entry` with proper `line_number` and `end_line` set.
    pub fn build(self, entry_type: EntryType) -> Entry {
        let end_line = self.start_line + self.lines.len().saturating_sub(1);
        let raw = self.lines.join("\n");
        let value = self.extract_value();

        Entry::new(entry_type, self.name, value)
            .with_line_number(self.start_line)
            .with_end_line(end_line)
            .with_raw_line(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_single_quotes_basic() {
        assert_eq!(
            QuotedValueBuilder::count_single_quotes("alias x='hello'"),
            2
        );
        assert_eq!(QuotedValueBuilder::count_single_quotes("alias x='hello"), 1);
        assert_eq!(QuotedValueBuilder::count_single_quotes("echo hello"), 0);
    }

    #[test]
    fn test_count_single_quotes_with_double() {
        // Single quotes inside double quotes should still be counted
        // because we're looking for single-quote boundaries
        assert_eq!(
            QuotedValueBuilder::count_single_quotes(r#"alias x='say "hi"'"#),
            2
        );
    }

    #[test]
    fn test_has_unclosed_quote() {
        assert!(QuotedValueBuilder::has_unclosed_single_quote(
            "alias x='hello"
        ));
        assert!(!QuotedValueBuilder::has_unclosed_single_quote(
            "alias x='hello'"
        ));
        assert!(QuotedValueBuilder::has_unclosed_single_quote(
            "alias x='it'\\''s"
        ));
    }

    #[test]
    fn test_multiline_alias() {
        let mut builder = QuotedValueBuilder::new("complex".into(), 5, "alias complex='echo line1");
        assert!(!builder.is_complete());

        builder.add_line("echo line2");
        assert!(!builder.is_complete());

        builder.add_line("echo line3'");
        assert!(builder.is_complete());

        let entry = builder.build(EntryType::Alias);
        assert_eq!(entry.name, "complex");
        assert_eq!(entry.line_number, Some(5));
        assert_eq!(entry.end_line, Some(7));
        assert!(entry.value.contains("line1"));
        assert!(entry.value.contains("line2"));
        assert!(entry.value.contains("line3"));
    }

    #[test]
    fn test_single_line_complete() {
        let builder = QuotedValueBuilder::new("simple".into(), 10, "alias simple='value'");
        assert!(builder.is_complete());

        let entry = builder.build(EntryType::Alias);
        assert_eq!(entry.line_number, Some(10));
        assert_eq!(entry.end_line, Some(10));
    }

    #[test]
    fn test_extract_value() {
        let builder = QuotedValueBuilder::new("test".into(), 1, "alias test='hello world'");
        assert_eq!(builder.extract_value(), "hello world");
    }

    #[test]
    fn test_extract_value_multiline() {
        let mut builder = QuotedValueBuilder::new("multi".into(), 1, "alias multi='line1");
        builder.add_line("line2");
        builder.add_line("line3'");

        let value = builder.extract_value();
        assert!(value.contains("line1"));
        assert!(value.contains("line2"));
        assert!(value.contains("line3"));
    }
}
