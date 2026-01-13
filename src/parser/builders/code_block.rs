//! # CodeBlockBuilder
//!
//! Accumulates lines for code blocks (control structures, etc.).
//!
//! ## Supported Structures
//!
//! - `if` / `fi`
//! - `while` / `done`
//! - `for` / `done`
//! - `case` / `esac`
//! - Other control flow constructs
//!
//! ## Example Input
//!
//! ```bash
//! if [ -f "$file" ]; then
//!     echo "File exists"
//! else
//!     echo "File not found"
//! fi
//! ```
//!
//! ## Naming Convention
//!
//! The entry name is automatically generated as:
//! - Single line: `L5`
//! - Multiple lines: `L5-L10`
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut builder = CodeBlockBuilder::new(5);
//! builder.add_line("if [ -f file ]; then");
//! builder.add_line("    echo exists");
//! builder.add_line("fi");
//! let entry = builder.build();
//! // entry.name = "L5-L7"
//! // entry.line_number = Some(5)
//! // entry.end_line = Some(7)
//! ```

use crate::model::{Entry, EntryType};

/// Builder for accumulating code block lines.
///
/// Code blocks include control structures (`if`, `while`, `for`, etc.)
/// that span multiple lines and should be treated as a single unit.
#[derive(Debug)]
pub struct CodeBlockBuilder {
    /// Line number where the block starts (1-based)
    pub start_line: usize,
    /// Accumulated lines of the block
    pub lines: Vec<String>,
}

impl CodeBlockBuilder {
    /// Create a new builder starting at the given line.
    pub fn new(start_line: usize) -> Self {
        Self {
            start_line,
            lines: Vec::new(),
        }
    }

    /// Add a line to the code block.
    pub fn add_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    /// Build the final Code entry with line range.
    ///
    /// The entry name is formatted as:
    /// - `L{start}` for single-line blocks
    /// - `L{start}-L{end}` for multi-line blocks
    pub fn build(self) -> Entry {
        let end_line = self.start_line + self.lines.len().saturating_sub(1);
        let name = if self.lines.len() <= 1 {
            format!("L{}", self.start_line)
        } else {
            format!("L{}-L{}", self.start_line, end_line)
        };
        let body = self.lines.join("\n");
        Entry::new(EntryType::Code, name, body.clone())
            .with_line_number(self.start_line)
            .with_end_line(end_line)
            .with_raw_line(body)
    }
}

/// Create an entry for consecutive blank lines.
///
/// # Arguments
///
/// - `start`: First blank line number (1-based)
/// - `end`: Last blank line number (1-based)
///
/// # Returns
///
/// A Code entry representing the blank line range.
pub fn create_blank_line_entry(start: usize, end: usize) -> Entry {
    let name = if start == end {
        format!("L{}", start)
    } else {
        format!("L{}-L{}", start, end)
    };
    Entry::new(EntryType::Code, name, String::new())
        .with_line_number(start)
        .with_end_line(end)
        .with_raw_line(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_block_single_line() {
        let mut builder = CodeBlockBuilder::new(10);
        builder.add_line("echo hello");
        let entry = builder.build();

        assert_eq!(entry.name, "L10");
        assert_eq!(entry.line_number, Some(10));
        assert_eq!(entry.end_line, Some(10));
    }

    #[test]
    fn test_code_block_multi_line() {
        let mut builder = CodeBlockBuilder::new(5);
        builder.add_line("if [ -f file ]; then");
        builder.add_line("    echo exists");
        builder.add_line("fi");
        let entry = builder.build();

        assert_eq!(entry.name, "L5-L7");
        assert_eq!(entry.line_number, Some(5));
        assert_eq!(entry.end_line, Some(7));
        assert_eq!(entry.entry_type, EntryType::Code);
    }

    #[test]
    fn test_blank_line_entry_single() {
        let entry = create_blank_line_entry(15, 15);
        assert_eq!(entry.name, "L15");
        assert_eq!(entry.line_number, Some(15));
        assert_eq!(entry.end_line, Some(15));
    }

    #[test]
    fn test_blank_line_entry_range() {
        let entry = create_blank_line_entry(20, 25);
        assert_eq!(entry.name, "L20-L25");
        assert_eq!(entry.line_number, Some(20));
        assert_eq!(entry.end_line, Some(25));
    }
}
