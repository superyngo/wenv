//! # Entry Parsing Methods
//!
//! Individual methods for parsing each entry type from Bash configuration files.
//!
//! ## Method Naming Convention
//!
//! - `try_parse_*` - Returns `Option<Entry>`, non-consuming
//! - `parse_*_start` - Detects the start of a multi-line construct
//!
//! ## Adding New Entry Types
//!
//! 1. Add regex pattern in `patterns.rs`
//! 2. Add `try_parse_*` method here
//! 3. Call from main loop in `mod.rs`
//!
//! ## Multi-line Handling
//!
//! Some entry types support multi-line values:
//! - Alias: Single-quote boundary detection
//! - EnvVar: Single-quote boundary detection
//! - Function: Brace counting (handled in mod.rs)

use super::patterns::*;
use crate::model::{Entry, EntryType};
use crate::parser::builders::{extract_comment, strip_quotes, QuotedValueBuilder};

/// Result of attempting to parse an alias line.
///
/// This enum handles both single-line aliases and the start of multi-line aliases.
#[derive(Debug)]
pub enum AliasParseResult {
    /// Single-line alias parsed successfully
    SingleLine(Entry),
    /// Multi-line alias detected, builder started
    MultiLineStart {
        /// Builder accumulating the multi-line content
        builder: QuotedValueBuilder,
    },
    /// Line is not an alias
    NotAlias,
}

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
/// - `AliasParseResult::SingleLine(entry)` for complete aliases
/// - `AliasParseResult::MultiLineStart { builder }` for multi-line start
/// - `AliasParseResult::NotAlias` if line is not an alias
pub fn try_parse_alias(line: &str, line_num: usize) -> AliasParseResult {
    // Try complete single-quoted alias first
    if let Some(caps) = ALIAS_SINGLE_RE.captures(line) {
        return AliasParseResult::SingleLine(
            Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }

    // Try complete double-quoted alias
    if let Some(caps) = ALIAS_DOUBLE_RE.captures(line) {
        return AliasParseResult::SingleLine(
            Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }

    // Check for multi-line alias start FIRST (before noquote)
    // This is critical: ALIAS_NOQUOTE_RE would incorrectly match `alias foo='123` as
    // a complete alias with value `'123`, preventing multi-line detection
    if let Some(caps) = ALIAS_MULTILINE_START_RE.captures(line) {
        // Verify it has an unclosed single quote
        if QuotedValueBuilder::has_unclosed_single_quote(line) {
            let name = caps[1].to_string();
            let builder = QuotedValueBuilder::new(name, line_num, line);
            return AliasParseResult::MultiLineStart { builder };
        }
    }

    // Try unquoted alias (LAST - after all quoted/multi-line checks)
    if let Some(caps) = ALIAS_NOQUOTE_RE.captures(line) {
        return AliasParseResult::SingleLine(
            Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }

    AliasParseResult::NotAlias
}

/// Result of attempting to parse an export line.
#[derive(Debug)]
pub enum ExportParseResult {
    /// Single-line export parsed successfully
    SingleLine(Entry),
    /// Multi-line export detected, builder started
    MultiLineStart {
        /// Builder accumulating the multi-line content
        builder: QuotedValueBuilder,
    },
    /// Line is not an export
    NotExport,
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
/// - `ExportParseResult::SingleLine(entry)` for complete exports
/// - `ExportParseResult::MultiLineStart { builder }` for multi-line start
/// - `ExportParseResult::NotExport` if line is not an export
pub fn try_parse_export(line: &str, line_num: usize) -> ExportParseResult {
    // Check for multi-line export start FIRST
    // (before the general EXPORT_RE which would match but not handle multi-line)
    if let Some(caps) = EXPORT_MULTILINE_START_RE.captures(line) {
        if QuotedValueBuilder::has_unclosed_single_quote(line) {
            let name = caps[1].to_string();
            let builder = QuotedValueBuilder::new(name, line_num, line);
            return ExportParseResult::MultiLineStart { builder };
        }
    }

    // Try complete export
    if let Some(caps) = EXPORT_RE.captures(line) {
        let (value_clean, _inline_comment) = extract_comment(&caps[2], '#');
        let value = strip_quotes(&value_clean);
        return ExportParseResult::SingleLine(
            Entry::new(EntryType::EnvVar, caps[1].to_string(), value)
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }

    ExportParseResult::NotExport
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
/// `Some(Entry)` if the line is a source statement, `None` otherwise.
pub fn try_parse_source(line: &str, line_num: usize) -> Option<Entry> {
    if let Some(caps) = SOURCE_RE.captures(line) {
        let (path_clean, _inline_comment) = extract_comment(&caps[1], '#');
        let path = strip_quotes(&path_clean);
        // Use line number as name for consistent identification
        let name = format!("L{}", line_num);
        return Some(
            Entry::new(EntryType::Source, name, path)
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }
    None
}

/// Detect if a line starts a function definition.
///
/// Matches:
/// - `name() {`
/// - `function name() {`
/// - `function name {`
///
/// # Arguments
///
/// - `line`: The trimmed line to check
///
/// # Returns
///
/// `Some(function_name)` if this is a function start, `None` otherwise.
pub fn detect_function_start(line: &str) -> Option<String> {
    if let Some(caps) = FUNC_START_RE.captures(line) {
        return Some(caps[1].to_string());
    }
    if let Some(caps) = FUNC_KEYWORD_RE.captures(line) {
        return Some(caps[1].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_alias_single() {
        match try_parse_alias("alias ll='ls -la'", 1) {
            AliasParseResult::SingleLine(entry) => {
                assert_eq!(entry.name, "ll");
                assert_eq!(entry.value, "ls -la");
            }
            _ => panic!("Expected SingleLine"),
        }
    }

    #[test]
    fn test_try_parse_alias_double() {
        match try_parse_alias(r#"alias gs="git status""#, 1) {
            AliasParseResult::SingleLine(entry) => {
                assert_eq!(entry.name, "gs");
                assert_eq!(entry.value, "git status");
            }
            _ => panic!("Expected SingleLine"),
        }
    }

    #[test]
    fn test_try_parse_alias_multiline_start() {
        match try_parse_alias("alias complex='echo line1", 5) {
            AliasParseResult::MultiLineStart { builder } => {
                assert_eq!(builder.name, "complex");
                assert_eq!(builder.start_line, 5);
                assert!(!builder.is_complete());
            }
            _ => panic!("Expected MultiLineStart"),
        }
    }

    #[test]
    fn test_try_parse_alias_multiline_precedence() {
        // This tests that multi-line detection takes precedence over noquote matching
        // Before the fix, `alias test1='123` would be incorrectly matched by ALIAS_NOQUOTE_RE
        // as a complete alias with value `'123`
        match try_parse_alias("alias test1='123", 1) {
            AliasParseResult::MultiLineStart { builder } => {
                assert_eq!(builder.name, "test1");
            }
            other => panic!("Expected MultiLineStart, got {:?}", other),
        }
    }

    #[test]
    fn test_try_parse_alias_not_alias() {
        match try_parse_alias("export VAR=value", 1) {
            AliasParseResult::NotAlias => {}
            _ => panic!("Expected NotAlias"),
        }
    }

    #[test]
    fn test_try_parse_export_single() {
        match try_parse_export("export EDITOR=nvim", 1) {
            ExportParseResult::SingleLine(entry) => {
                assert_eq!(entry.name, "EDITOR");
                assert_eq!(entry.value, "nvim");
            }
            _ => panic!("Expected SingleLine"),
        }
    }

    #[test]
    fn test_try_parse_export_quoted() {
        match try_parse_export(r#"export PATH="$HOME/bin:$PATH""#, 1) {
            ExportParseResult::SingleLine(entry) => {
                assert_eq!(entry.name, "PATH");
                assert_eq!(entry.value, "$HOME/bin:$PATH");
            }
            _ => panic!("Expected SingleLine"),
        }
    }

    #[test]
    fn test_try_parse_export_multiline_start() {
        match try_parse_export("export LONG='first line", 10) {
            ExportParseResult::MultiLineStart { builder } => {
                assert_eq!(builder.name, "LONG");
                assert_eq!(builder.start_line, 10);
            }
            _ => panic!("Expected MultiLineStart"),
        }
    }

    #[test]
    fn test_try_parse_source() {
        let entry = try_parse_source("source ~/.bashrc", 5).unwrap();
        assert_eq!(entry.entry_type, EntryType::Source);
        assert_eq!(entry.name, "L5");
        assert_eq!(entry.value, "~/.bashrc");
    }

    #[test]
    fn test_try_parse_source_dot() {
        let entry = try_parse_source(". ~/.profile", 10).unwrap();
        assert_eq!(entry.value, "~/.profile");
    }

    #[test]
    fn test_detect_function_start() {
        assert_eq!(detect_function_start("greet() {"), Some("greet".into()));
        assert_eq!(
            detect_function_start("function hello() {"),
            Some("hello".into())
        );
        assert_eq!(
            detect_function_start("function test {"),
            Some("test".into())
        );
        assert_eq!(detect_function_start("echo hello"), None);
    }
}
