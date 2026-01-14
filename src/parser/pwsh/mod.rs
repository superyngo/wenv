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
use crate::parser::builders::{
    count_braces_outside_quotes, create_blank_line_entry, CodeBlockBuilder, CommentBlockBuilder,
    FunctionBuilder,
};
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

        // Function parsing state
        let mut in_function = false;
        let mut brace_count = 0;
        let mut current_func: Option<FunctionBuilder> = None;

        // Control structure state
        let mut control_depth: usize = 0;
        let mut current_code_block: Option<CodeBlockBuilder> = None;

        // Comment block state
        let mut current_comment_block: Option<CommentBlockBuilder> = None;

        // Blank line tracking
        let mut blank_line_start: Option<usize> = None;

        // Pending comment for association
        let mut pending_comment: Option<String> = None;

        // Environment variable Here-String state
        let mut in_env_heredoc = false;
        let mut env_heredoc_var_name: Option<String> = None;
        let mut env_heredoc_lines: Vec<String> = Vec::new();
        let mut env_heredoc_start_line: usize = 0;

        for (line_num, line) in content.lines().enumerate() {
            let line_number = line_num + 1;
            let trimmed = line.trim();

            // Handle multi-line function body
            if in_function {
                let (open, close) = count_braces_outside_quotes(trimmed);
                brace_count += open;
                brace_count = brace_count.saturating_sub(close);

                if let Some(ref mut func) = current_func {
                    func.add_line(line);
                }

                if brace_count == 0 {
                    in_function = false;
                    if let Some(func) = current_func.take() {
                        let mut entry = func.build(EntryType::Function);
                        if let Some(comment) = pending_comment.take() {
                            entry = entry.with_comment(comment);
                        }
                        result.add_entry(entry);
                    }
                }
                continue;
            }

            // Handle environment variable Here-String content (before control structure check)
            if in_env_heredoc {
                if is_heredoc_end(trimmed) {
                    // End of Here-String
                    in_env_heredoc = false;
                    let value = env_heredoc_lines.join("\n");
                    let mut entry = Entry::new(
                        EntryType::EnvVar,
                        env_heredoc_var_name.take().unwrap(),
                        value,
                    );
                    entry = entry.with_line_number(env_heredoc_start_line);
                    entry = entry.with_end_line(line_number);
                    result.add_entry(entry);
                } else {
                    // Collect Here-String content line
                    env_heredoc_lines.push(line.to_string());
                }
                continue;
            }

            // Handle control structure blocks
            let prev_depth = control_depth;
            control_depth = control_depth.saturating_sub(count_control_end(trimmed));
            control_depth += count_control_start(trimmed);

            if control_depth > 0 || (prev_depth > 0 && control_depth == 0) {
                // Flush pending items
                flush_pending_items(
                    &mut result,
                    &mut current_comment_block,
                    &mut blank_line_start,
                    line_number,
                );

                if current_code_block.is_none() && prev_depth == 0 && control_depth > 0 {
                    current_code_block = Some(CodeBlockBuilder::new(line_number));
                }

                if let Some(ref mut block) = current_code_block {
                    block.add_line(line);
                }

                if prev_depth > 0 && control_depth == 0 {
                    if let Some(block) = current_code_block.take() {
                        result.add_entry(block.build());
                    }
                }

                pending_comment = None;
                continue;
            }

            // Handle empty lines
            if trimmed.is_empty() {
                // Flush comment block on blank line
                if let Some(block) = current_comment_block.take() {
                    result.add_entry(block.build());
                }

                if blank_line_start.is_none() {
                    blank_line_start = Some(line_number);
                }
                pending_comment = None;
                continue;
            } else {
                // Non-empty line, flush blank lines
                if let Some(start) = blank_line_start.take() {
                    let end = line_number - 1;
                    result.add_entry(create_blank_line_entry(start, end));
                }
            }

            // Handle comment lines (adjacent merging)
            if CommentBlockBuilder::is_standalone_comment(trimmed) {
                if let Some(ref mut block) = current_comment_block {
                    block.add_line(line);
                } else {
                    current_comment_block = Some(CommentBlockBuilder::new(line_number, line));
                }

                if let Some(stripped) = trimmed.strip_prefix('#') {
                    pending_comment = Some(stripped.trim().to_string());
                }
                continue;
            } else {
                // Non-comment line, flush comment block
                if let Some(block) = current_comment_block.take() {
                    result.add_entry(block.build());
                }
            }

            // Try to parse entry types
            if let Some(mut entry) = try_parse_alias(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
                continue;
            }

            // Check for Here-String environment variable start (only outside control structures)
            if control_depth == 0 {
                if let Some(var_name) = detect_env_heredoc_start(trimmed) {
                    in_env_heredoc = true;
                    env_heredoc_var_name = Some(var_name);
                    env_heredoc_lines.clear();
                    env_heredoc_start_line = line_number;
                    pending_comment = None;
                    continue;
                }
            }

            if let Some(mut entry) = try_parse_env(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
                continue;
            }

            if let Some(mut entry) = try_parse_source(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
                continue;
            }

            if let Some(func_name) = detect_function_start(trimmed) {
                in_function = true;
                let (open, close) = count_braces_outside_quotes(trimmed);
                brace_count = open.saturating_sub(close);

                current_func = Some(FunctionBuilder::new(func_name, line_number));
                if let Some(ref mut func) = current_func {
                    func.add_line(line);
                }

                // Single-line function
                if brace_count == 0 && trimmed.contains('}') {
                    in_function = false;
                    if let Some(func) = current_func.take() {
                        let mut entry = func.build(EntryType::Function);
                        if let Some(comment) = pending_comment.take() {
                            entry = entry.with_comment(comment);
                        }
                        result.add_entry(entry);
                    }
                }
                continue;
            }

            // Fallback: capture as Code
            let entry = Entry::new(
                EntryType::Code,
                format!("L{}", line_number),
                trimmed.to_string(),
            )
            .with_line_number(line_number)
            .with_raw_line(line.to_string());
            result.add_entry(entry);
            pending_comment = None;
        }

        // Flush remaining state
        if let Some(block) = current_comment_block.take() {
            result.add_entry(block.build());
        }

        if let Some(start) = blank_line_start.take() {
            let end = content.lines().count();
            result.add_entry(create_blank_line_entry(start, end));
        }

        if in_function {
            result.add_warning(crate::model::ParseWarning::new(
                current_func.as_ref().map(|f| f.start_line).unwrap_or(0),
                "Unclosed function definition at end of file",
                "",
            ));
        }

        if in_env_heredoc {
            result.add_warning(crate::model::ParseWarning::new(
                env_heredoc_start_line,
                "Unclosed environment variable Here-String at end of file",
                "",
            ));
        }

        result
    }

    fn shell_type(&self) -> ShellType {
        ShellType::PowerShell
    }
}

/// Helper to flush pending comment blocks and blank lines.
fn flush_pending_items(
    result: &mut ParseResult,
    comment_block: &mut Option<CommentBlockBuilder>,
    blank_line_start: &mut Option<usize>,
    current_line: usize,
) {
    if let Some(block) = comment_block.take() {
        result.add_entry(block.build());
    }
    if let Some(start) = blank_line_start.take() {
        let end = current_line - 1;
        result.add_entry(create_blank_line_entry(start, end));
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
    fn test_comment_association() {
        let parser = PowerShellParser::new();
        let content = "# List files\nSet-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        let alias = result
            .entries
            .iter()
            .find(|e| e.entry_type == EntryType::Alias)
            .unwrap();

        assert_eq!(alias.comment, Some("List files".to_string()));
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

        assert!(code_blocks.iter().any(|c| c.value.contains("$env:PATH")));
    }
}
