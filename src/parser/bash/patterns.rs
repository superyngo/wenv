//! # Bash Regex Patterns
//!
//! All regex patterns for parsing Bash configuration file syntax.
//!
//! ## Pattern Naming Convention
//!
//! - `*_SINGLE_RE` - Single-quoted version
//! - `*_DOUBLE_RE` - Double-quoted version
//! - `*_NOQUOTE_RE` - Unquoted version
//! - `*_START_RE` - Start of multi-line construct
//!
//! ## Adding New Patterns
//!
//! 1. Add pattern to `lazy_static!` block below
//! 2. Add corresponding parse method in `parsers.rs`
//! 3. Integrate into main loop in `mod.rs`
//!
//! ## Regex Notes
//!
//! Rust's `regex` crate does not support backreferences, so we use
//! separate patterns for single-quoted and double-quoted variants.

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // =========================================================================
    // Alias Patterns
    // =========================================================================

    /// Matches single-quoted alias: `alias name='value'`
    ///
    /// Captures:
    /// - Group 1: alias name (allows special chars like `.`, `~`, `-`)
    /// - Group 2: alias value (content between single quotes)
    ///
    /// Note: This only matches complete single-line aliases.
    /// Multi-line aliases are handled by `QuotedValueBuilder`.
    pub static ref ALIAS_SINGLE_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)='([^']*)'(?:\s*(?:#.*)?)?$"#
    ).unwrap();

    /// Matches double-quoted alias: `alias name="value"`
    ///
    /// Captures:
    /// - Group 1: alias name
    /// - Group 2: alias value
    pub static ref ALIAS_DOUBLE_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)="([^"]*)"(?:\s*(?:#.*)?)?$"#
    ).unwrap();

    /// Matches unquoted alias: `alias name=value`
    ///
    /// Captures:
    /// - Group 1: alias name
    /// - Group 2: alias value (single word, no spaces)
    pub static ref ALIAS_NOQUOTE_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)=(\S+)(?:\s*(?:#.*)?)?$"#
    ).unwrap();

    /// Matches the start of a potentially multi-line single-quoted alias.
    ///
    /// Captures:
    /// - Group 1: alias name
    ///
    /// This pattern matches `alias name='...` where the quote is not closed.
    pub static ref ALIAS_MULTILINE_START_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)='"#
    ).unwrap();

    // =========================================================================
    // Export (Environment Variable) Patterns
    // =========================================================================

    /// Matches export statement: `export VAR=value` or `export VAR="value"`
    ///
    /// Captures:
    /// - Group 1: variable name (word characters only)
    /// - Group 2: value (everything after `=`)
    pub static ref EXPORT_RE: Regex = Regex::new(
        r#"^export\s+(\w+)=(.*)$"#
    ).unwrap();

    /// Matches the start of a potentially multi-line export.
    ///
    /// Captures:
    /// - Group 1: variable name
    pub static ref EXPORT_MULTILINE_START_RE: Regex = Regex::new(
        r#"^export\s+(\w+)='"#
    ).unwrap();

    // =========================================================================
    // Source Patterns
    // =========================================================================

    /// Matches source statement: `source file` or `. file`
    ///
    /// Captures:
    /// - Group 1: file path (everything after source/.)
    pub static ref SOURCE_RE: Regex = Regex::new(
        r#"^(?:source|\.)\s+(.+)$"#
    ).unwrap();

    // =========================================================================
    // Function Patterns
    // =========================================================================

    /// Matches function with parentheses: `name() {` or `function name() {`
    ///
    /// Captures:
    /// - Group 1: function name
    pub static ref FUNC_START_RE: Regex = Regex::new(
        r#"^(?:function\s+)?(\w+)\s*\(\s*\)\s*\{?"#
    ).unwrap();

    /// Matches function with `function` keyword (no parentheses): `function name {`
    ///
    /// Captures:
    /// - Group 1: function name
    pub static ref FUNC_KEYWORD_RE: Regex = Regex::new(
        r#"^function\s+(\w+)\s*\{"#
    ).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_single_re() {
        let caps = ALIAS_SINGLE_RE.captures("alias ll='ls -la'").unwrap();
        assert_eq!(&caps[1], "ll");
        assert_eq!(&caps[2], "ls -la");
    }

    #[test]
    fn test_alias_double_re() {
        let caps = ALIAS_DOUBLE_RE
            .captures(r#"alias gs="git status""#)
            .unwrap();
        assert_eq!(&caps[1], "gs");
        assert_eq!(&caps[2], "git status");
    }

    #[test]
    fn test_alias_special_names() {
        assert!(ALIAS_SINGLE_RE.captures("alias ..='cd ..'").is_some());
        assert!(ALIAS_SINGLE_RE.captures("alias ~='cd ~'").is_some());
        assert!(ALIAS_SINGLE_RE.captures("alias ...='cd ../..'").is_some());
    }

    #[test]
    fn test_alias_multiline_start() {
        let caps = ALIAS_MULTILINE_START_RE
            .captures("alias complex='echo")
            .unwrap();
        assert_eq!(&caps[1], "complex");
    }

    #[test]
    fn test_export_re() {
        let caps = EXPORT_RE.captures("export EDITOR=nvim").unwrap();
        assert_eq!(&caps[1], "EDITOR");
        assert_eq!(&caps[2], "nvim");
    }

    #[test]
    fn test_export_multiline_start() {
        let caps = EXPORT_MULTILINE_START_RE
            .captures("export LONG='first line")
            .unwrap();
        assert_eq!(&caps[1], "LONG");
    }

    #[test]
    fn test_source_re() {
        let caps = SOURCE_RE.captures("source ~/.bashrc").unwrap();
        assert_eq!(&caps[1], "~/.bashrc");

        let caps = SOURCE_RE.captures(". ~/.profile").unwrap();
        assert_eq!(&caps[1], "~/.profile");
    }

    #[test]
    fn test_func_start_re() {
        let caps = FUNC_START_RE.captures("greet() {").unwrap();
        assert_eq!(&caps[1], "greet");

        let caps = FUNC_START_RE.captures("function hello() {").unwrap();
        assert_eq!(&caps[1], "hello");
    }

    #[test]
    fn test_func_keyword_re() {
        let caps = FUNC_KEYWORD_RE.captures("function test {").unwrap();
        assert_eq!(&caps[1], "test");
    }
}
