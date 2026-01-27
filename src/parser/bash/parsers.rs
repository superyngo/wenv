//! # Entry Parsing Methods
//!
//! Individual methods for parsing each entry type from Bash configuration files.
//!
//! ## Standard Signatures
//!
//! All `try_parse_*` functions follow the unified signature:
//! - `try_parse_alias(line, line_num) -> ParseEvent`
//! - `try_parse_env(line, line_num) -> ParseEvent`
//! - `try_parse_source(line, line_num) -> ParseEvent`
//!
//! Returns:
//! - `ParseEvent::Complete(entry)` for single-line entries
//! - `ParseEvent::Started { ... }` for multi-line entry starts
//! - `ParseEvent::None` if no match
//!
//! ## Function Detection
//!
//! - `detect_function_start(line) -> Option<String>` - Standard function for all shells
//!
//! ## Adding New Entry Types
//!
//! 1. Add regex pattern in `patterns.rs`
//! 2. Add `try_parse_*` method here returning `ParseEvent`
//! 3. Call from main loop in `mod.rs`

use super::patterns::*;
use crate::model::{Entry, EntryType};
use crate::parser::builders::{extract_comment, strip_quotes, QuotedValueBuilder};
use crate::parser::{BoundaryType, ParseEvent};

/// Try to parse a line as an alias.
///
/// This handles three cases:
/// 1. Complete single-quoted alias: `alias name='value'`
/// 2. Complete double-quoted alias: `alias name="value"`
/// 3. Start of multi-line alias: `alias name='unclosed...`
///
/// # Arguments
///
/// - `line`: The trimmed line to parse
/// - `line_num`: 1-based line number
///
/// # Returns
///
/// - `ParseEvent::Complete(entry)` for complete aliases
/// - `ParseEvent::Started { ... }` for multi-line alias start
/// - `ParseEvent::None` if line is not an alias
pub fn try_parse_alias(line: &str, line_num: usize) -> ParseEvent {
    // Try complete single-quoted alias first
    if let Some(caps) = ALIAS_SINGLE_RE.captures(line) {
        return ParseEvent::Complete(
            Entry::new(EntryType::Alias, caps[1].to_string(), line.to_string())
                .with_line_number(line_num),
        );
    }

    // Try complete double-quoted alias
    if let Some(caps) = ALIAS_DOUBLE_RE.captures(line) {
        return ParseEvent::Complete(
            Entry::new(EntryType::Alias, caps[1].to_string(), line.to_string())
                .with_line_number(line_num),
        );
    }

    // Check for multi-line alias start FIRST (before noquote)
    // This is critical: ALIAS_NOQUOTE_RE would incorrectly match `alias foo='123` as
    // a complete alias with value `'123`, preventing multi-line detection
    if let Some(caps) = ALIAS_MULTILINE_START_RE.captures(line) {
        // Verify it has an unclosed single quote
        if QuotedValueBuilder::has_unclosed_single_quote(line) {
            let name = caps[1].to_string();
            let quote_count = line.chars().filter(|&c| c == '\'').count();
            return ParseEvent::Started {
                entry_type: EntryType::Alias,
                name,
                boundary: BoundaryType::QuoteCounting { quote_count },
                first_line: line.to_string(),
            };
        }
    }

    // Try unquoted alias (LAST - after all quoted/multi-line checks)
    if let Some(caps) = ALIAS_NOQUOTE_RE.captures(line) {
        return ParseEvent::Complete(
            Entry::new(EntryType::Alias, caps[1].to_string(), line.to_string())
                .with_line_number(line_num),
        );
    }

    ParseEvent::None
}

/// Try to parse a line as an environment variable export.
///
/// This handles:
/// 1. Complete export: `export VAR=value` or `export VAR="value"`
/// 2. Start of multi-line export: `export VAR='unclosed...`
///
/// # Arguments
///
/// - `line`: The trimmed line to parse
/// - `line_num`: 1-based line number
///
/// # Returns
///
/// - `ParseEvent::Complete(entry)` for complete exports
/// - `ParseEvent::Started { ... }` for multi-line export start
/// - `ParseEvent::None` if line is not an export
pub fn try_parse_env(line: &str, line_num: usize) -> ParseEvent {
    // Check for multi-line export start FIRST
    // (before the general EXPORT_RE which would match but not handle multi-line)
    if let Some(caps) = EXPORT_MULTILINE_START_RE.captures(line) {
        if QuotedValueBuilder::has_unclosed_single_quote(line) {
            let name = caps[1].to_string();
            let quote_count = line.chars().filter(|&c| c == '\'').count();
            return ParseEvent::Started {
                entry_type: EntryType::EnvVar,
                name,
                boundary: BoundaryType::QuoteCounting { quote_count },
                first_line: line.to_string(),
            };
        }
    }

    // Try complete export
    if let Some(caps) = EXPORT_RE.captures(line) {
        let (value_clean, _inline_comment) = extract_comment(&caps[2], '#');
        let _value = strip_quotes(&value_clean);
        return ParseEvent::Complete(
            Entry::new(EntryType::EnvVar, caps[1].to_string(), line.to_string())
                .with_line_number(line_num),
        );
    }

    ParseEvent::None
}

/// Try to parse a line as a source statement.
///
/// Matches:
/// - `source file`
/// - `. file`
///
/// # Arguments
///
/// - `line`: The trimmed line to parse
/// - `line_num`: 1-based line number
///
/// # Returns
///
/// - `ParseEvent::Complete(entry)` if the line is a source statement
/// - `ParseEvent::None` otherwise
pub fn try_parse_source(line: &str, line_num: usize) -> ParseEvent {
    if let Some(caps) = SOURCE_RE.captures(line) {
        let (path_clean, _inline_comment) = extract_comment(&caps[1], '#');
        let path = strip_quotes(&path_clean);
        // Extract filename (without extension) as name for TUI identification
        let name = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&path)
            .to_string();
        return ParseEvent::Complete(
            Entry::new(EntryType::Source, name, line.to_string()).with_line_number(line_num),
        );
    }
    ParseEvent::None
}

/// Detect if a line starts a function definition.
///
/// Matches:
/// - `name() {` - Named function
/// - `function name() {` - Named function with keyword
/// - `function name {` - Named function without parentheses
/// - `() {` - Anonymous function
///
/// # Arguments
///
/// - `line`: The trimmed line to check
///
/// # Returns
///
/// `Some((function_name, is_anonymous))` where:
/// - `function_name`: The function name, or empty string for anonymous
/// - `is_anonymous`: true if this is an anonymous function
///
/// Returns `None` if not a function start.
pub fn detect_function_start(line: &str) -> Option<(String, bool)> {
    use super::patterns::{ANON_FUNC_RE, FUNC_KEYWORD_RE, FUNC_START_RE};

    // Check named functions first
    if let Some(caps) = FUNC_START_RE.captures(line) {
        return Some((caps[1].to_string(), false));
    }
    if let Some(caps) = FUNC_KEYWORD_RE.captures(line) {
        return Some((caps[1].to_string(), false));
    }
    // Check anonymous function
    if ANON_FUNC_RE.is_match(line) {
        return Some((String::new(), true));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_alias_single() {
        match try_parse_alias("alias ll='ls -la'", 1) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "ll");
                assert_eq!(entry.value, "alias ll=\'ls -la\'");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_alias_double() {
        match try_parse_alias(r#"alias gs="git status""#, 1) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "gs");
                assert_eq!(entry.value, r#"alias gs="git status""#);
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_alias_multiline_start() {
        match try_parse_alias("alias complex='echo line1", 5) {
            ParseEvent::Started {
                entry_type,
                name,
                boundary,
                ..
            } => {
                assert_eq!(entry_type, EntryType::Alias);
                assert_eq!(name, "complex");
                assert!(matches!(
                    boundary,
                    BoundaryType::QuoteCounting { quote_count: 1 }
                ));
            }
            _ => panic!("Expected Started"),
        }
    }

    #[test]
    fn test_try_parse_alias_multiline_precedence() {
        // This tests that multi-line detection takes precedence over noquote matching
        // Before the fix, `alias test1='123` would be incorrectly matched by ALIAS_NOQUOTE_RE
        // as a complete alias with value `'123`
        match try_parse_alias("alias test1='123", 1) {
            ParseEvent::Started { name, .. } => {
                assert_eq!(name, "test1");
            }
            other => panic!("Expected Started, got {:?}", other),
        }
    }

    #[test]
    fn test_try_parse_alias_not_alias() {
        match try_parse_alias("export VAR=value", 1) {
            ParseEvent::None => {}
            _ => panic!("Expected None"),
        }
    }

    #[test]
    fn test_try_parse_env_single() {
        match try_parse_env("export EDITOR=nvim", 1) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "EDITOR");
                assert_eq!(entry.value, "export EDITOR=nvim");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_env_quoted() {
        match try_parse_env(r#"export PATH="$HOME/bin:$PATH""#, 1) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "PATH");
                assert_eq!(entry.value, r#"export PATH="$HOME/bin:$PATH""#);
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_env_multiline_start() {
        match try_parse_env("export LONG='first line", 10) {
            ParseEvent::Started {
                entry_type, name, ..
            } => {
                assert_eq!(entry_type, EntryType::EnvVar);
                assert_eq!(name, "LONG");
            }
            _ => panic!("Expected Started"),
        }
    }

    #[test]
    fn test_try_parse_source() {
        match try_parse_source("source ~/.bashrc", 5) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.entry_type, EntryType::Source);
                assert_eq!(entry.name, ".bashrc");
                assert_eq!(entry.value, "source ~/.bashrc");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_source_dot() {
        match try_parse_source(". ~/.profile", 10) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.value, ". ~/.profile");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_detect_function_start() {
        assert_eq!(
            detect_function_start("greet() {"),
            Some(("greet".to_string(), false))
        );
        assert_eq!(
            detect_function_start("function hello() {"),
            Some(("hello".to_string(), false))
        );
        assert_eq!(
            detect_function_start("function test {"),
            Some(("test".to_string(), false))
        );
        assert_eq!(detect_function_start("() {"), Some((String::new(), true)));
        assert_eq!(detect_function_start("echo hello"), None);
    }
}
