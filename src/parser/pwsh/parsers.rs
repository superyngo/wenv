//! # Entry Parsing Methods (PowerShell)
//!
//! Individual methods for parsing each entry type from PowerShell configuration files.
//!
//! ## Supported Entry Types
//!
//! - Alias: `Set-Alias`, `New-Alias`
//! - EnvVar: `$env:NAME = value`
//! - Source: `. .\file.ps1`
//! - Function: `function Name { }`

use super::patterns::*;
use crate::model::{Entry, EntryType};
use crate::parser::builders::{extract_comment, strip_quotes};

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
/// `Some(Entry)` if the line is an alias, `None` otherwise.
pub fn try_parse_alias(line: &str, line_num: usize) -> Option<Entry> {
    // Try simple format first
    if let Some(caps) = ALIAS_SIMPLE_RE.captures(line) {
        return Some(
            Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }

    // Try complex format with -Name and -Value
    if let Some(caps) = ALIAS_RE.captures(line) {
        let name = caps[2].to_string();
        let value = strip_quotes(&caps[4]);
        return Some(
            Entry::new(EntryType::Alias, name, value)
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }

    None
}

/// Try to parse a line as a PowerShell environment variable.
///
/// Matches: `$env:VAR = value`
///
/// # Arguments
///
/// - `line`: The trimmed line to parse
/// - `line_num`: 1-based line number
///
/// # Returns
///
/// `Some(Entry)` if the line is an env var, `None` otherwise.
pub fn try_parse_env(line: &str, line_num: usize) -> Option<Entry> {
    if let Some(caps) = ENV_RE.captures(line) {
        let (value_clean, _inline_comment) = extract_comment(&caps[2], '#');
        let value = strip_quotes(&value_clean);
        return Some(
            Entry::new(EntryType::EnvVar, caps[1].to_string(), value)
                .with_line_number(line_num)
                .with_raw_line(line.to_string()),
        );
    }
    None
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
/// `Some(Entry)` if the line is a source statement, `None` otherwise.
pub fn try_parse_source(line: &str, line_num: usize) -> Option<Entry> {
    if let Some(caps) = SOURCE_RE.captures(line) {
        let (path_clean, _inline_comment) = extract_comment(&caps[1], '#');
        let path = strip_quotes(&path_clean);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_alias_simple() {
        let entry = try_parse_alias("Set-Alias ll Get-ChildItem", 1).unwrap();
        assert_eq!(entry.name, "ll");
        assert_eq!(entry.value, "Get-ChildItem");
    }

    #[test]
    fn test_try_parse_alias_with_params() {
        let entry = try_parse_alias("Set-Alias -Name gs -Value git", 5).unwrap();
        assert_eq!(entry.name, "gs");
        assert_eq!(entry.value, "git");
        assert_eq!(entry.line_number, Some(5));
    }

    #[test]
    fn test_try_parse_env() {
        let entry = try_parse_env(r#"$env:EDITOR = "code""#, 10).unwrap();
        assert_eq!(entry.name, "EDITOR");
        assert_eq!(entry.value, "code");
    }

    #[test]
    fn test_try_parse_source() {
        let entry = try_parse_source(r#". .\aliases.ps1"#, 15).unwrap();
        assert_eq!(entry.entry_type, EntryType::Source);
        assert_eq!(entry.name, "L15");
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
}
