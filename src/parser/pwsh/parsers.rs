//! # Entry Parsing Methods (PowerShell)
//!
//! Individual methods for parsing each entry type from PowerShell configuration files.
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
//! - `ParseEvent::Started { ... }` for multi-line entry starts (e.g., Here-Strings)
//! - `ParseEvent::None` if no match
//!
//! ## Function Detection
//!
//! - `detect_function_start(line) -> Option<String>` - Standard function for all shells
//!
//! ## Supported Entry Types
//!
//! - Alias: `Set-Alias`, `New-Alias`
//! - EnvVar: `$env:NAME = value` (single-line or Here-String)
//! - Source: `. .\file.ps1`
//! - Function: `function Name { }`

use super::patterns::*;
use crate::model::{Entry, EntryType};
use crate::parser::builders::{extract_comment, strip_quotes};
use crate::parser::{BoundaryType, ParseEvent};

/// Try to parse a line as a PowerShell alias.
///
/// Matches:
/// - `Set-Alias name value`
/// - `New-Alias -Name name -Value value`
///
/// # Arguments
///
/// - `line`: The trimmed line to parse
/// - `line_num`: 1-based line number
///
/// # Returns
///
/// - `ParseEvent::Complete(entry)` if the line is an alias
/// - `ParseEvent::None` otherwise
pub fn try_parse_alias(line: &str, line_num: usize) -> ParseEvent {
    // Try simple format first
    if let Some(caps) = ALIAS_SIMPLE_RE.captures(line) {
        return ParseEvent::Complete(
            Entry::new(EntryType::Alias, caps[1].to_string(), line.to_string())
                .with_line_number(line_num),
        );
    }

    // Try complex format with -Name and -Value
    if let Some(caps) = ALIAS_RE.captures(line) {
        let name = caps[2].to_string();
        return ParseEvent::Complete(
            Entry::new(EntryType::Alias, name, line.to_string()).with_line_number(line_num),
        );
    }

    ParseEvent::None
}

/// Try to parse a line as a PowerShell environment variable.
///
/// Matches:
/// - `$env:VAR = value` (single-line)
/// - `$env:VAR = @"` (Here-String start)
///
/// # Arguments
///
/// - `line`: The trimmed line to parse
/// - `line_num`: 1-based line number
///
/// # Returns
///
/// - `ParseEvent::Complete(entry)` for single-line env vars
/// - `ParseEvent::Started { ... }` for Here-String start
/// - `ParseEvent::None` otherwise
pub fn try_parse_env(line: &str, line_num: usize) -> ParseEvent {
    // Check for Here-String start FIRST (multi-line)
    if let Some(caps) = ENV_HEREDOC_START_RE.captures(line) {
        let name = caps[1].to_string();
        return ParseEvent::Started {
            entry_type: EntryType::EnvVar,
            name,
            // Use QuoteCounting as a marker for Here-String
            // The actual end detection is done via is_heredoc_end() in the main loop
            boundary: BoundaryType::QuoteCounting { quote_count: 1 },
            first_line: line.to_string(),
        };
    }

    // Try single-line env var
    if let Some(caps) = ENV_RE.captures(line) {
        let (value_clean, _inline_comment) = extract_comment(&caps[2], '#');
        let _value = strip_quotes(&value_clean);
        return ParseEvent::Complete(
            Entry::new(EntryType::EnvVar, caps[1].to_string(), line.to_string())
                .with_line_number(line_num),
        );
    }

    ParseEvent::None
}

/// Try to parse a line as a PowerShell source statement.
///
/// Matches: `. .\file.ps1`
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
/// Matches: `function Name {` or `function Name($param) {`
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
    None
}

/// Check if a line is a Here-String end marker.
///
/// Matches: `"@`
///
/// # Arguments
///
/// - `line`: The trimmed line to check
///
/// # Returns
///
/// `true` if this is a Here-String end marker.
pub fn is_heredoc_end(line: &str) -> bool {
    line == r#""@"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_alias_simple() {
        match try_parse_alias("Set-Alias ll Get-ChildItem", 1) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "ll");
                assert_eq!(entry.value, "Set-Alias ll Get-ChildItem");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_alias_with_params() {
        match try_parse_alias("Set-Alias -Name gs -Value git", 5) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "gs");
                assert_eq!(entry.value, "Set-Alias -Name gs -Value git");
                assert_eq!(entry.line_number, Some(5));
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_env() {
        match try_parse_env(r#"$env:EDITOR = "code""#, 10) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.name, "EDITOR");
                assert_eq!(entry.value, "$env:EDITOR = \"code\"");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_try_parse_env_heredoc_start() {
        match try_parse_env(r#"$env:LONG = @""#, 5) {
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
        match try_parse_source(r#". .\aliases.ps1"#, 15) {
            ParseEvent::Complete(entry) => {
                assert_eq!(entry.entry_type, EntryType::Source);
                assert_eq!(entry.name, ".\\aliases");
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_detect_function_start() {
        assert_eq!(
            detect_function_start("function Get-Greeting {"),
            Some("Get-Greeting".into())
        );
        assert_eq!(
            detect_function_start("function Test-Func($x) {"),
            Some("Test-Func".into())
        );
        assert_eq!(detect_function_start("Write-Host 'hello'"), None);
    }

    #[test]
    fn test_is_heredoc_end() {
        assert!(is_heredoc_end(r#""@"#));
        assert!(!is_heredoc_end(r#"  "@"#)); // Trimmed before calling
        assert!(!is_heredoc_end("other line"));
    }
}
