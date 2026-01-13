//! # PowerShell Regex Patterns
//!
//! All regex patterns for parsing PowerShell configuration file syntax.
//!
//! ## Pattern Naming Convention
//!
//! - `*_RE` - Standard pattern
//! - `*_SIMPLE_RE` - Simplified variant
//! - `*_START_RE` - Start of multi-line construct
//!
//! ## PowerShell Syntax Notes
//!
//! - Aliases use `Set-Alias` or `New-Alias` cmdlets
//! - Environment variables use `$env:NAME` syntax
//! - Source uses dot-sourcing: `. .\file.ps1`
//! - Functions use `function Name { }` syntax

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    // =========================================================================
    // Alias Patterns
    // =========================================================================

    /// Matches complex alias format with optional `-Name` and `-Value` parameters.
    ///
    /// Examples:
    /// - `Set-Alias -Name ll -Value Get-ChildItem`
    /// - `New-Alias ll Get-ChildItem`
    ///
    /// Captures:
    /// - Group 1: Optional `-Name ` prefix
    /// - Group 2: alias name
    /// - Group 3: Optional `-Value ` prefix
    /// - Group 4: alias value
    pub static ref ALIAS_RE: Regex = Regex::new(
        r#"^(?:Set-Alias|New-Alias)\s+(-Name\s+)?(\w+)\s+(-Value\s+)?(.+)$"#
    ).unwrap();

    /// Matches simple alias format: `Set-Alias name value`
    ///
    /// Captures:
    /// - Group 1: alias name
    /// - Group 2: alias value (word characters and hyphens)
    pub static ref ALIAS_SIMPLE_RE: Regex = Regex::new(
        r#"^(?:Set-Alias|New-Alias)\s+(\w+)\s+(\w[\w-]*)$"#
    ).unwrap();

    // =========================================================================
    // Environment Variable Patterns
    // =========================================================================

    /// Matches environment variable assignment: `$env:VAR = value`
    ///
    /// Captures:
    /// - Group 1: variable name
    /// - Group 2: value (everything after `=`)
    pub static ref ENV_RE: Regex = Regex::new(
        r#"^\$env:(\w+)\s*=\s*(.+)$"#
    ).unwrap();

    // =========================================================================
    // Source Patterns
    // =========================================================================

    /// Matches dot-sourcing: `. .\file.ps1` or `. C:\path\file.ps1`
    ///
    /// Captures:
    /// - Group 1: file path (must end with `.ps1`)
    pub static ref SOURCE_RE: Regex = Regex::new(
        r#"^\.\s+(.+\.ps1)$"#
    ).unwrap();

    // =========================================================================
    // Function Patterns
    // =========================================================================

    /// Matches function definition: `function Name {` or `function Name() {`
    ///
    /// Captures:
    /// - Group 1: function name (allows hyphens)
    pub static ref FUNC_START_RE: Regex = Regex::new(
        r#"^function\s+(\w[\w-]*)\s*(?:\([^)]*\))?\s*\{"#
    ).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_simple_re() {
        let caps = ALIAS_SIMPLE_RE
            .captures("Set-Alias ll Get-ChildItem")
            .unwrap();
        assert_eq!(&caps[1], "ll");
        assert_eq!(&caps[2], "Get-ChildItem");
    }

    #[test]
    fn test_alias_re_with_params() {
        let caps = ALIAS_RE.captures("Set-Alias -Name gs -Value git").unwrap();
        assert_eq!(&caps[2], "gs");
        assert_eq!(&caps[4], "git");
    }

    #[test]
    fn test_env_re() {
        let caps = ENV_RE.captures(r#"$env:EDITOR = "code""#).unwrap();
        assert_eq!(&caps[1], "EDITOR");
        assert_eq!(&caps[2], r#""code""#);
    }

    #[test]
    fn test_source_re() {
        let caps = SOURCE_RE.captures(r#". .\aliases.ps1"#).unwrap();
        assert_eq!(&caps[1], r#".\aliases.ps1"#);
    }

    #[test]
    fn test_func_start_re() {
        let caps = FUNC_START_RE.captures("function Get-Greeting {").unwrap();
        assert_eq!(&caps[1], "Get-Greeting");
    }

    #[test]
    fn test_func_with_params() {
        let caps = FUNC_START_RE
            .captures("function Test-Func($param) {")
            .unwrap();
        assert_eq!(&caps[1], "Test-Func");
    }
}
