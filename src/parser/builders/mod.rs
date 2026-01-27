//! # Entry Builders
//!
//! Builders for accumulating multi-line entries during parsing.
//!
//! ## Module Structure
//!
//! ```text
//! builders/
//! ├── mod.rs          - This file: exports + utility functions
//! ├── quoted.rs       - QuotedValueBuilder for multi-line quoted values
//! └── comment.rs      - CommentBlockBuilder for adjacent comments
//! ```
//!
//! ## Builder Pattern
//!
//! All builders follow this lifecycle:
//!
//! 1. **`new(start_line, first_line)`** - Initialize with the first line
//! 2. **`add_line(line)`** - Accumulate subsequent lines
//! 3. **`is_complete()`** - Check if block is closed (optional, for some builders)
//! 4. **`build(entry_type)`** - Produce final `Entry` with line range
//!
//! ## Available Builders
//!
//! | Builder | Entry Types | Boundary Detection | Line Range |
//! |---------|-------------|-------------------|------------|
//! | [`QuotedValueBuilder`] | Alias, EnvVar | Quote counting | ✅ `end_line` |
//! | [`CommentBlockBuilder`] | Comment | Non-comment line | ✅ `end_line` |
//!
//! ## Example: Multi-line Alias
//!
//! ```rust,ignore
//! use crate::parser::builders::QuotedValueBuilder;
//!
//! // Check if line starts a multi-line value
//! if QuotedValueBuilder::has_unclosed_single_quote(line) {
//!     let mut builder = QuotedValueBuilder::new(name, line_num, line);
//!     
//!     while !builder.is_complete() {
//!         let next = lines.next();
//!         builder.add_line(next);
//!     }
//!     
//!     let entry = builder.build(EntryType::Alias);
//!     // entry.line_number = Some(start)
//!     // entry.end_line = Some(end)
//! }
//! ```
//!
//! ## Utility Functions
//!
//! This module also provides utility functions for quote-aware parsing:
//!
//! - [`count_braces_outside_quotes`] - Count `{` and `}` outside quoted strings
//! - [`extract_comment`] - Extract inline comments respecting quotes
//! - [`strip_quotes`] - Remove surrounding quotes from a value

mod comment;
mod quoted;

// Re-export builders
pub use comment::CommentBlockBuilder;
pub use quoted::QuotedValueBuilder;

/// Count braces `{` and `}` outside of quoted strings.
///
/// This is used for tracking function body boundaries. Braces inside
/// single or double quotes are not counted.
///
/// # Arguments
///
/// - `line`: The line to analyze
///
/// # Returns
///
/// A tuple `(open_count, close_count)` where:
/// - `open_count`: Number of `{` outside quotes
/// - `close_count`: Number of `}` outside quotes
///
/// # Example
///
/// ```rust,ignore
/// let (open, close) = count_braces_outside_quotes("func() { echo \"}\" }");
/// assert_eq!(open, 1);
/// assert_eq!(close, 1); // The } inside quotes is not counted
/// ```
pub fn count_braces_outside_quotes(line: &str) -> (usize, usize) {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut open_count = 0;
    let mut close_count = 0;

    for c in line.chars() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '{' if !in_single_quote && !in_double_quote => open_count += 1,
            '}' if !in_single_quote && !in_double_quote => close_count += 1,
            _ => {}
        }
    }

    (open_count, close_count)
}

/// Count opening `(` and closing `)` parentheses outside quoted strings.
///
/// This is used to track multi-line structures that use parentheses for grouping,
/// such as `plugins=(...)` in shell configs.
///
/// # Arguments
///
/// - `line`: The line to analyze
///
/// # Returns
///
/// A tuple `(open_count, close_count)` of parentheses counts outside quotes.
///
/// # Example
///
/// ```rust,ignore
/// let (open, close) = count_parens_outside_quotes("plugins=(");
/// assert_eq!(open, 1);
/// assert_eq!(close, 0);
/// ```
pub fn count_parens_outside_quotes(line: &str) -> (usize, usize) {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut open_count = 0;
    let mut close_count = 0;

    for c in line.chars() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '(' if !in_single_quote && !in_double_quote => open_count += 1,
            ')' if !in_single_quote && !in_double_quote => close_count += 1,
            _ => {}
        }
    }

    (open_count, close_count)
}

/// Extract inline comment from a line, respecting quoted strings.
///
/// The comment character (typically `#`) is only recognized outside
/// of single and double quotes.
///
/// # Arguments
///
/// - `line`: The line to analyze
/// - `comment_char`: The comment character (usually `#`)
///
/// # Returns
///
/// A tuple `(code, comment)` where:
/// - `code`: The part before the comment (trimmed)
/// - `comment`: The comment text without the `#`, or `None`
///
/// # Example
///
/// ```rust,ignore
/// let (code, comment) = extract_comment("alias ll='ls -la' # list files", '#');
/// assert_eq!(code, "alias ll='ls -la'");
/// assert_eq!(comment, Some("list files".to_string()));
///
/// // Comment inside quotes is not extracted
/// let (code, comment) = extract_comment("alias x='echo # not a comment'", '#');
/// assert_eq!(code, "alias x='echo # not a comment'");
/// assert_eq!(comment, None);
/// ```
pub fn extract_comment(line: &str, comment_char: char) -> (String, Option<String>) {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let chars: Vec<char> = line.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            c if c == comment_char && !in_single_quote && !in_double_quote => {
                let code: String = chars[..i].iter().collect();
                let comment: String = chars[i + 1..].iter().collect();
                return (
                    code.trim_end().to_string(),
                    Some(comment.trim().to_string()),
                );
            }
            _ => {}
        }
    }

    (line.to_string(), None)
}

/// Strip surrounding quotes from a value.
///
/// Removes matching single or double quotes from the beginning and end
/// of the value. If the quotes don't match or aren't present, returns
/// the original value (trimmed).
///
/// # Arguments
///
/// - `value`: The value to strip quotes from
///
/// # Returns
///
/// The value without surrounding quotes.
///
/// # Example
///
/// ```rust,ignore
/// assert_eq!(strip_quotes("'hello'"), "hello");
/// assert_eq!(strip_quotes("\"world\""), "world");
/// assert_eq!(strip_quotes("no quotes"), "no quotes");
/// ```
pub fn strip_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_braces_simple() {
        let (open, close) = count_braces_outside_quotes("function test() {");
        assert_eq!(open, 1);
        assert_eq!(close, 0);
    }

    #[test]
    fn test_count_braces_in_double_quotes() {
        let (open, close) = count_braces_outside_quotes(r#"echo "{ not counted }""#);
        assert_eq!(open, 0);
        assert_eq!(close, 0);
    }

    #[test]
    fn test_count_braces_in_single_quotes() {
        let (open, close) = count_braces_outside_quotes("echo '{ also ignored }'");
        assert_eq!(open, 0);
        assert_eq!(close, 0);
    }

    #[test]
    fn test_count_braces_mixed() {
        let (open, close) = count_braces_outside_quotes(r#"{ echo "}" }"#);
        assert_eq!(open, 1);
        assert_eq!(close, 1);
    }

    #[test]
    fn test_extract_comment_basic() {
        let (code, comment) = extract_comment("alias ll='ls -la' # list files", '#');
        assert_eq!(code, "alias ll='ls -la'");
        assert_eq!(comment, Some("list files".to_string()));
    }

    #[test]
    fn test_extract_comment_in_single_quotes() {
        let (code, comment) = extract_comment("alias x='echo # not a comment'", '#');
        assert_eq!(code, "alias x='echo # not a comment'");
        assert_eq!(comment, None);
    }

    #[test]
    fn test_extract_comment_in_double_quotes() {
        let (code, comment) = extract_comment("echo \"# not a comment\"", '#');
        assert_eq!(code, "echo \"# not a comment\"");
        assert_eq!(comment, None);
    }

    #[test]
    fn test_extract_comment_none() {
        let (code, comment) = extract_comment("echo hello", '#');
        assert_eq!(code, "echo hello");
        assert_eq!(comment, None);
    }

    #[test]
    fn test_strip_quotes_single() {
        assert_eq!(strip_quotes("'hello'"), "hello");
    }

    #[test]
    fn test_strip_quotes_double() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
    }

    #[test]
    fn test_strip_quotes_none() {
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn test_strip_quotes_mismatched() {
        assert_eq!(strip_quotes("'hello\""), "'hello\"");
    }

    #[test]
    fn test_strip_quotes_with_whitespace() {
        assert_eq!(strip_quotes("  'hello'  "), "hello");
    }
}
