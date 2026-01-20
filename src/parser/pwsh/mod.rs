//! # PowerShell Parser
//!
//! Parses PowerShell profile files (`$PROFILE` and similar).
//!
//! ## Supported Entry Types
//!
//! | Type | Pattern | Multi-line |
//! |------|---------|------------|
//! | Alias | `Set-Alias`, `New-Alias` | ❌ |
//! | EnvVar | `$env:VAR = value` | ❌ |
//! | Function | `function Name { ... }` | ✅ Brace counting |
//! | Source | `. .\file.ps1` | ❌ |
//! | Comment | `# text` | ✅ Adjacent merging |
//! | Code | Control structures, other | ✅ Keyword tracking |
//!
//! ## Module Structure
//!
//! - [`patterns`] - Regex definitions for syntax matching
//! - [`control`] - Control structure detection (`if`, `foreach`, etc.)
//! - [`parsers`] - Individual entry parsing methods
//!
//! ## PowerShell-Specific Notes
//!
//! - Uses `#` for comments (same as Bash)
//! - Control structures end with `}` but may have continuations (`else`, `catch`)
//! - Function names can contain hyphens (e.g., `Get-ChildItem`)

pub mod control;
pub mod parsers;
pub mod patterns;

use crate::model::{Entry, EntryType, ParseResult, ShellType};
use crate::parser::builders::{count_braces_outside_quotes, CommentBlockBuilder};
use crate::parser::pending::{BoundaryType, PendingBlock};
use crate::parser::Parser;

use control::{count_control_end, count_control_start};
use parsers::{
    detect_env_heredoc_start, detect_function_start, is_heredoc_end, try_parse_alias,
    try_parse_env, try_parse_source,
};

/// PowerShell configuration file parser.
///
/// Implements the [`Parser`] trait for parsing PowerShell profiles.
pub struct PowerShellParser;

impl PowerShellParser {
    /// Create a new PowerShell parser instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PowerShellParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for PowerShellParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut result = ParseResult::new();

        // === Unified pending state ===

        // For multi-line structures: function, control block, Here-String
        let mut active_block: Option<PendingBlock> = None;

        // For Comment/Code merging (separate from active_block)
        let mut pending_entry: Option<PendingBlock> = None;

        // Track control structure depth (needed for control block detection)
        let mut control_depth: usize = 0;

        // === Main parsing loop ===
        // Use split('\n') instead of lines() to preserve trailing empty lines.
        // lines() treats '\n' as a line terminator, so "a\nb\n" → ["a", "b"]
        // But for raw_line format (where '\n' is a separator), we need "a\nb\n" → ["a", "b", ""]
        // However, a file ending with '\n' is just proper file termination, not an extra line.
        // So we strip the final empty element only if content ends with '\n'.
        let lines_vec: Vec<&str> = content.split('\n').collect();
        let lines_to_process: &[&str] = if lines_vec.last() == Some(&"") && content.ends_with('\n')
        {
            &lines_vec[..lines_vec.len() - 1]
        } else {
            &lines_vec[..]
        };

        for (line_num, line) in lines_to_process.iter().enumerate() {
            let line_number = line_num + 1;
            let trimmed = line.trim();

            // ------------------------------------------------------------------
            // Handle active multi-line block (function, control, Here-String)
            // ------------------------------------------------------------------
            if let Some(ref mut block) = active_block {
                block.add_line(line, line_number);

                match &mut block.boundary {
                    BoundaryType::BraceCounting {
                        ref mut brace_count,
                    } => {
                        // Function body
                        let (open, close) = count_braces_outside_quotes(trimmed);
                        *brace_count += open as i32;
                        *brace_count = (*brace_count).saturating_sub(close as i32);

                        if *brace_count == 0 {
                            let entry = self.build_entry_from_pending(active_block.take().unwrap());
                            result.add_entry(entry);
                        }
                    }
                    BoundaryType::QuoteCounting { quote_count: _ } => {
                        // Here-String - check for terminator
                        if is_heredoc_end(trimmed) {
                            let mut completed = active_block.take().unwrap();
                            // Value is collected lines (excluding start/end markers)
                            let value = completed
                                .lines
                                .iter()
                                .skip(1) // skip start line with @"
                                .take(completed.lines.len().saturating_sub(2)) // skip end line with "@
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n");
                            completed.value = Some(value);
                            let entry = self.build_entry_from_pending(completed);
                            result.add_entry(entry);
                        }
                    }
                    BoundaryType::KeywordTracking { ref mut depth } => {
                        // Control structure
                        let end_count = count_control_end(trimmed);
                        let start_count = count_control_start(trimmed);
                        *depth = (*depth).saturating_sub(end_count);
                        *depth += start_count;

                        if *depth == 0 {
                            // Reset external control_depth to sync state
                            control_depth = 0;

                            // Emit code block directly (PowerShell doesn't merge like Bash)
                            let entry = self.build_entry_from_pending(active_block.take().unwrap());
                            result.add_entry(entry);
                        }
                    }
                    _ => {}
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Check for control structure start/continuation
            // ------------------------------------------------------------------
            let prev_depth = control_depth;
            control_depth = control_depth.saturating_sub(count_control_end(trimmed));
            control_depth += count_control_start(trimmed);

            if control_depth > 0 || (prev_depth > 0 && control_depth == 0) {
                // Flush pending items before starting control block
                if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(e);
                }

                // Start control block
                if prev_depth == 0 && control_depth > 0 {
                    active_block = Some(PendingBlock::control(line_number, line, control_depth));
                }

                // Handle final line of control structure
                if prev_depth > 0 && control_depth == 0 {
                    // This shouldn't normally happen since we process inside active_block
                    // But handle it defensively
                }

                continue;
            }

            // ------------------------------------------------------------------
            // Handle empty lines (part of pending entry state machine)
            // ------------------------------------------------------------------
            if trimmed.is_empty() {
                let blank = PendingBlock::blank_lines(line_number, line);

                match &mut pending_entry {
                    Some(pending) if pending.can_absorb_blank() => {
                        // Comment or BlankLines absorbs blank
                        pending.add_line(line, line_number);
                    }
                    Some(_) => {
                        // Other pending types: flush and start new blank
                        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(blank);
                    }
                    None => {
                        pending_entry = Some(blank);
                    }
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Handle comment lines (part of pending entry state machine)
            // ------------------------------------------------------------------
            if CommentBlockBuilder::is_standalone_comment(trimmed) {
                match &mut pending_entry {
                    Some(pending) if pending.can_absorb_comment() => {
                        // Comment merges with Comment
                        pending.add_line(line, line_number);
                    }
                    Some(_) => {
                        // Non-Comment pending: flush and start new comment
                        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(PendingBlock::comment(line_number, line));
                    }
                    None => {
                        pending_entry = Some(PendingBlock::comment(line_number, line));
                    }
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Try to parse structured entry types (Alias, EnvVar, Source, Function)
            // ------------------------------------------------------------------

            // Try alias
            if let Some(entry) = try_parse_alias(trimmed, line_number) {
                // Flush pending entry
                if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(e);
                }
                result.add_entry(entry);
                continue;
            }

            // Try Here-String env var (only outside control structures)
            if let Some(var_name) = detect_env_heredoc_start(trimmed) {
                // Flush pending entry
                if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(e);
                }
                // Start Here-String block
                active_block = Some(PendingBlock {
                    lines: vec![line.to_string()],
                    start_line: line_number,
                    end_line: line_number,
                    boundary: BoundaryType::QuoteCounting { quote_count: 1 }, // Use odd count to indicate incomplete
                    entry_hint: Some(EntryType::EnvVar),
                    name: Some(var_name),
                    value: None,
                });
                continue;
            }

            // Try regular env var
            if let Some(entry) = try_parse_env(trimmed, line_number) {
                // Flush pending entry
                if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(e);
                }
                result.add_entry(entry);
                continue;
            }

            // Try source
            if let Some(entry) = try_parse_source(trimmed, line_number) {
                // Flush pending entry
                if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(e);
                }
                result.add_entry(entry);
                continue;
            }

            // Try function
            if let Some(func_name) = detect_function_start(trimmed) {
                // Flush pending entry
                if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(e);
                }

                let (open, close) = count_braces_outside_quotes(trimmed);
                let brace_count = (open as i32).saturating_sub(close as i32);

                let func_block =
                    PendingBlock::function(func_name.clone(), line_number, line, brace_count);

                // Single-line function check
                if brace_count == 0 && trimmed.contains('}') {
                    let entry = self.build_entry_from_pending(func_block);
                    result.add_entry(entry);
                } else {
                    active_block = Some(func_block);
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Fallback: capture as Code
            // ------------------------------------------------------------------
            // Flush pending and create new Code entry
            if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                result.add_entry(entry);
            }

            let entry = Entry::new(
                EntryType::Code,
                format!("L{}", line_number),
                trimmed.to_string(),
            )
            .with_line_number(line_number)
            .with_raw_line(line.to_string());
            result.add_entry(entry);
        }

        // === Flush remaining state ===

        // Flush remaining pending entry
        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
            result.add_entry(entry);
        }

        // Warn about unclosed active block
        if let Some(block) = active_block {
            let msg = match block.entry_hint {
                Some(EntryType::Function) => "Unclosed function definition at end of file",
                Some(EntryType::EnvVar) => {
                    "Unclosed environment variable Here-String at end of file"
                }
                _ => "Unclosed block at end of file",
            };
            result.add_warning(crate::model::ParseWarning::new(block.start_line, msg, ""));
        }

        result
    }

    fn shell_type(&self) -> ShellType {
        ShellType::PowerShell
    }
}

impl PowerShellParser {
    /// Build an Entry from a completed PendingBlock.
    fn build_entry_from_pending(&self, block: PendingBlock) -> Entry {
        let entry_type = block.entry_hint.unwrap_or(EntryType::Code);
        let raw_content = block.raw_content();

        match entry_type {
            EntryType::Function => {
                let name = block.name.unwrap_or_else(|| "anonymous".to_string());
                let body = self.extract_function_body(&raw_content);
                Entry::new(EntryType::Function, name, body)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
                    .with_raw_line(raw_content)
            }
            EntryType::EnvVar => {
                let name = block.name.unwrap_or_else(|| "UNKNOWN".to_string());
                let value = block.value.unwrap_or_default();
                Entry::new(EntryType::EnvVar, name, value)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
            }
            EntryType::Comment => {
                let name = if block.start_line == block.end_line {
                    format!("#L{}", block.start_line)
                } else {
                    format!("#L{}-L{}", block.start_line, block.end_line)
                };
                // First line's comment text as value
                let value = block
                    .lines
                    .first()
                    .map(|l| l.trim().to_string())
                    .unwrap_or_default();
                Entry::new(EntryType::Comment, name, value)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
                    .with_raw_line(raw_content)
            }
            EntryType::Code => {
                let name = if block.start_line == block.end_line {
                    format!("L{}", block.start_line)
                } else {
                    format!("L{}-L{}", block.start_line, block.end_line)
                };
                // First non-blank line as value, or empty for blank-only blocks
                let first_non_blank = block
                    .lines
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| block.lines.first().cloned().unwrap_or_default());
                let value = if first_non_blank.trim().is_empty() {
                    String::new()
                } else {
                    first_non_blank.trim().to_string()
                };
                Entry::new(EntryType::Code, name, value)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
                    .with_raw_line(raw_content)
            }
            _ => {
                // Shouldn't happen, but handle gracefully
                Entry::new(entry_type, "unknown".to_string(), raw_content.clone())
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
                    .with_raw_line(raw_content)
            }
        }
    }

    /// Extract function body from raw content.
    fn extract_function_body(&self, raw: &str) -> String {
        // Find opening brace and extract body
        if let Some(start) = raw.find('{') {
            if let Some(end) = raw.rfind('}') {
                if start < end {
                    return raw[start + 1..end].trim().to_string();
                }
            }
        }
        raw.to_string()
    }

    /// Flush pending Comment/Code entry.
    fn flush_pending_comment_code(&self, pending: &mut Option<PendingBlock>) -> Option<Entry> {
        pending
            .take()
            .map(|block| self.build_entry_from_pending(block))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_alias() {
        let parser = PowerShellParser::new();
        let content = "Set-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();

        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].name, "ll");
        assert_eq!(aliases[0].value, "Get-ChildItem");
    }

    #[test]
    fn test_parse_env() {
        let parser = PowerShellParser::new();
        let content = r#"$env:EDITOR = "code""#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "EDITOR");
        assert_eq!(envs[0].value, "code");
    }

    #[test]
    fn test_parse_function_with_end_line() {
        let parser = PowerShellParser::new();
        let content = "function Get-Greeting {\n    Write-Host 'Hello'\n}";
        let result = parser.parse(content);

        let funcs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Function)
            .collect();

        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "Get-Greeting");
        assert_eq!(funcs[0].line_number, Some(1));
        assert_eq!(funcs[0].end_line, Some(3));
    }

    #[test]
    fn test_adjacent_comments_merged() {
        let parser = PowerShellParser::new();
        let content = "# Comment 1\n# Comment 2\n# Comment 3\nSet-Alias test Get-ChildItem";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].name, "#L1-L3");
        assert_eq!(comments[0].line_number, Some(1));
        assert_eq!(comments[0].end_line, Some(3));
    }

    #[test]
    fn test_control_structure_captured_as_code() {
        let parser = PowerShellParser::new();
        let content = "if ($true) {\n    Write-Host 'yes'\n}";
        let result = parser.parse(content);

        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("if"))
            .collect();

        assert_eq!(code_blocks.len(), 1);
        assert_eq!(code_blocks[0].line_number, Some(1));
        assert_eq!(code_blocks[0].end_line, Some(3));
    }

    #[test]
    fn test_blank_lines_grouped() {
        let parser = PowerShellParser::new();
        let content = "Set-Alias a Get-ChildItem\n\n\nSet-Alias b Get-Location";
        let result = parser.parse(content);

        let blanks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.is_empty())
            .collect();

        assert_eq!(blanks.len(), 1);
        assert_eq!(blanks[0].name, "L2-L3");
    }

    #[test]
    fn test_env_heredoc_simple() {
        let parser = PowerShellParser::new();
        let content = r#"$env:PATH = @"
C:\Program Files\bin
D:\tools
"@"#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "PATH");
        assert_eq!(envs[0].value, "C:\\Program Files\\bin\nD:\\tools");
        assert_eq!(envs[0].line_number, Some(1));
        assert_eq!(envs[0].end_line, Some(4));
    }

    #[test]
    fn test_env_heredoc_with_spaces() {
        let parser = PowerShellParser::new();
        let content = r#"$env:CONFIG = @"
  line with leading spaces
    indented line
"@"#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "CONFIG");
        assert_eq!(
            envs[0].value,
            "  line with leading spaces\n    indented line"
        );
    }

    #[test]
    fn test_env_heredoc_single_line_backward_compat() {
        let parser = PowerShellParser::new();
        let content = r#"$env:EDITOR = "code""#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "EDITOR");
        assert_eq!(envs[0].value, "code");
        assert_eq!(envs[0].line_number, Some(1));
        assert!(envs[0].end_line.is_none());
    }

    #[test]
    fn test_env_heredoc_mixed_with_single_line() {
        let parser = PowerShellParser::new();
        let content = r#"$env:EDITOR = "code"
$env:PATH = @"
C:\bin
D:\tools
"@
$env:SHELL = "pwsh""#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 3);
        assert_eq!(envs[0].name, "EDITOR");
        assert_eq!(envs[0].value, "code");
        assert_eq!(envs[1].name, "PATH");
        assert_eq!(envs[1].value, "C:\\bin\nD:\\tools");
        assert_eq!(envs[2].name, "SHELL");
        assert_eq!(envs[2].value, "pwsh");
    }

    #[test]
    fn test_env_heredoc_empty_lines() {
        let parser = PowerShellParser::new();
        let content = r#"$env:DATA = @"
line1

line3
"@"#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "DATA");
        assert_eq!(envs[0].value, "line1\n\nline3");
    }

    #[test]
    fn test_env_heredoc_with_special_chars() {
        let parser = PowerShellParser::new();
        let content = r#"$env:NOTES = @"
Line with "quotes"
Line with $variable
Line with 'single quotes'
"@"#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "NOTES");
        assert!(envs[0].value.contains("\"quotes\""));
        assert!(envs[0].value.contains("$variable"));
    }

    #[test]
    fn test_env_heredoc_unclosed_warning() {
        let parser = PowerShellParser::new();
        let content = r#"$env:PATH = @"
C:\bin
D:\tools"#;
        let result = parser.parse(content);

        assert!(result.warnings.iter().any(|w| w
            .message
            .contains("Unclosed environment variable Here-String")));
    }

    #[test]
    fn test_env_heredoc_not_inside_function() {
        let parser = PowerShellParser::new();
        let content = r#"function Test {
    $env:PATH = @"
C:\bin
"@
}"#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(
            envs.len(),
            0,
            "Here-String inside function should not be parsed as EnvVar"
        );

        let funcs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Function)
            .collect();

        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].value.contains("$env:PATH"));
    }

    #[test]
    fn test_env_heredoc_not_inside_control_structure() {
        let parser = PowerShellParser::new();
        let content = r#"if ($true) {
    $env:PATH = @"
C:\bin
"@
}"#;
        let result = parser.parse(content);

        let envs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(
            envs.len(),
            0,
            "Here-String inside control structure should not be parsed as EnvVar"
        );

        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code)
            .collect();

        // Check raw_line since value only contains first line for multi-line Code blocks
        assert!(code_blocks.iter().any(|c| c
            .raw_line
            .as_ref()
            .map(|r| r.contains("$env:PATH"))
            .unwrap_or(false)));
    }
}
