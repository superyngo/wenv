//! # FunctionBuilder
//!
//! Accumulates lines for multi-line shell functions.
//!
//! ## Example Input
//!
//! ```bash
//! my_func() {
//!     echo "hello"
//!     echo "world"
//! }
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut builder = FunctionBuilder::new("my_func".into(), 1);
//! builder.add_line("my_func() {");
//! builder.add_line("    echo \"hello\"");
//! builder.add_line("}");
//! let entry = builder.build(EntryType::Function);
//! // entry.line_number = Some(1)
//! // entry.end_line = Some(3)
//! ```
//!
//! ## Line Range
//!
//! The builder automatically calculates `end_line` based on the number of
//! accumulated lines, enabling proper display of line ranges (e.g., "5-15").

use crate::model::{Entry, EntryType};

/// Builder for accumulating multi-line function definitions.
///
/// # Fields
///
/// - `name`: Function name extracted from the definition
/// - `start_line`: Line number where the function starts (1-based)
/// - `lines`: Accumulated raw lines including the function body
#[derive(Debug)]
pub struct FunctionBuilder {
    pub name: String,
    pub start_line: usize,
    pub lines: Vec<String>,
}

impl FunctionBuilder {
    /// Create a new builder for a function starting at the given line.
    ///
    /// # Arguments
    ///
    /// - `name`: The function name
    /// - `start_line`: 1-based line number where the function starts
    pub fn new(name: String, start_line: usize) -> Self {
        Self {
            name,
            start_line,
            lines: Vec::new(),
        }
    }

    /// Add a line to the function body.
    ///
    /// Lines are accumulated in order and will be joined with newlines
    /// when building the final entry.
    pub fn add_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    /// Build the final Entry with proper line range.
    ///
    /// # Arguments
    ///
    /// - `entry_type`: The type of entry (typically `EntryType::Function`)
    ///
    /// # Returns
    ///
    /// An `Entry` with:
    /// - `line_number`: Set to `start_line`
    /// - `end_line`: Set to `start_line + lines.len() - 1`
    /// - `value`: Function body only (without opening/closing lines)
    /// - `raw_line`: Complete function definition
    pub fn build(self, entry_type: EntryType) -> Entry {
        let end_line = self.start_line + self.lines.len().saturating_sub(1);
        let raw = self.lines.join("\n");

        // Extract function body (lines between opening { and closing })
        let body = self.extract_body();

        Entry::new(entry_type, self.name, body)
            .with_line_number(self.start_line)
            .with_end_line(end_line)
            .with_raw_line(raw)
    }

    /// Extract the function body, excluding the opening and closing lines
    fn extract_body(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }

        if self.lines.len() == 1 {
            // Single-line function: extract content between { and }
            let line = &self.lines[0];
            if let Some(start) = line.find('{') {
                if let Some(end) = line.rfind('}') {
                    if end > start + 1 {
                        return line[start + 1..end].trim().to_string();
                    }
                }
            }
            return String::new();
        }

        // Multi-line function: take lines between first and last (body lines)
        if self.lines.len() > 2 {
            self.lines[1..self.lines.len() - 1].join("\n")
        } else {
            // Only 2 lines (opening and closing), empty body
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_builder_single_line() {
        let mut builder = FunctionBuilder::new("test".into(), 5);
        builder.add_line("test() { echo hello; }");
        let entry = builder.build(EntryType::Function);

        assert_eq!(entry.name, "test");
        assert_eq!(entry.line_number, Some(5));
        assert_eq!(entry.end_line, Some(5));
        // value should be the body only
        assert_eq!(entry.value, "echo hello;");
        // raw_line should be the complete function
        assert!(entry.raw_line.as_ref().unwrap().contains("test() {"));
    }

    #[test]
    fn test_function_builder_multi_line() {
        let mut builder = FunctionBuilder::new("greet".into(), 10);
        builder.add_line("greet() {");
        builder.add_line("    echo \"Hello\"");
        builder.add_line("    echo \"World\"");
        builder.add_line("}");
        let entry = builder.build(EntryType::Function);

        assert_eq!(entry.name, "greet");
        assert_eq!(entry.line_number, Some(10));
        assert_eq!(entry.end_line, Some(13));
        // value should be the body only (middle lines)
        assert!(entry.value.contains("echo \"Hello\""));
        assert!(entry.value.contains("echo \"World\""));
        assert!(!entry.value.contains("greet() {")); // Should NOT contain opening
        assert!(!entry.value.contains("}")); // Should NOT contain closing as separate line
                                             // raw_line should have complete function
        assert!(entry.raw_line.as_ref().unwrap().contains("greet() {"));
        assert!(entry.raw_line.as_ref().unwrap().ends_with("}"));
    }

    #[test]
    fn test_function_builder_empty() {
        let builder = FunctionBuilder::new("empty".into(), 1);
        let entry = builder.build(EntryType::Function);

        assert_eq!(entry.line_number, Some(1));
        // Empty builder: start_line + 0.saturating_sub(1) = start_line + 0 = 1
        // But with no lines, end_line equals start_line
        assert_eq!(entry.end_line, Some(1));
        assert_eq!(entry.value, "");
    }
}
