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
use crate::parser::pending::{
    entry_to_trailing_pending, merge_pending_with_structured, BoundaryType, MergeType, PendingBlock,
};
use crate::parser::Parser;

use control::{count_control_end, count_control_start};
use parsers::{
    detect_function_start, detect_scriptblock_start, is_block_comment_end, is_block_comment_start,
    is_heredoc_end, try_parse_alias, try_parse_env, try_parse_source,
};

use crate::parser::ParseEvent;

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
                            let completed = active_block.take().unwrap();
                            pending_entry = Some(entry_to_trailing_pending(
                                self.build_entry_from_pending(completed),
                            ));
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
                            pending_entry = Some(entry_to_trailing_pending(entry));
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

                            // Make result pending for trailing blank absorption
                            let completed = active_block.take().unwrap();
                            pending_entry = Some(PendingBlock {
                                lines: completed.lines,
                                start_line: completed.start_line,
                                end_line: completed.end_line,
                                boundary: BoundaryType::AdjacentMerging {
                                    merge_type: crate::parser::pending::MergeType::CodeWithBlanks,
                                },
                                entry_hint: Some(EntryType::Code),
                                name: None,
                                value: None,
                                comment_count: 0,
                                has_absorbed_blanks: false,
                            });
                        }
                    }
                    BoundaryType::BlockComment => {
                        if is_block_comment_end(trimmed) {
                            let completed = active_block.take().unwrap();
                            let entry = self.build_entry_from_pending(completed);
                            pending_entry = Some(entry_to_trailing_pending(entry));
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
                // Start control block - merge pending Comment/Code if present
                if active_block.is_none() && prev_depth == 0 && control_depth > 0 {
                    if let Some(pending) = pending_entry.take() {
                        if matches!(
                            pending.entry_hint,
                            Some(EntryType::Comment) | Some(EntryType::Code)
                        ) {
                            // Seed block with pending content
                            let mut lines = pending.lines;
                            lines.push(line.to_string());
                            active_block = Some(PendingBlock {
                                lines,
                                start_line: pending.start_line,
                                end_line: line_number,
                                boundary: BoundaryType::KeywordTracking {
                                    depth: control_depth,
                                },
                                entry_hint: Some(EntryType::Code),
                                name: None,
                                value: None,
                                comment_count: pending.comment_count,
                                has_absorbed_blanks: false,
                            });
                        } else {
                            // Flush non-mergeable pending
                            result.add_entry(self.build_entry_from_pending(pending));
                            active_block =
                                Some(PendingBlock::control(line_number, line, control_depth));
                        }
                    } else {
                        active_block =
                            Some(PendingBlock::control(line_number, line, control_depth));
                    }
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Check for block comment start <#
            // ------------------------------------------------------------------
            if is_block_comment_start(trimmed) {
                if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                    result.add_entry(entry);
                }
                active_block = Some(PendingBlock {
                    lines: vec![line.to_string()],
                    start_line: line_number,
                    end_line: line_number,
                    boundary: BoundaryType::BlockComment,
                    entry_hint: Some(EntryType::Comment),
                    name: None,
                    value: None,
                    comment_count: 0,
                    has_absorbed_blanks: false,
                });
                continue;
            }

            // ------------------------------------------------------------------
            // Handle empty lines (part of pending entry state machine)
            // ------------------------------------------------------------------
            if trimmed.is_empty() {
                let blank = PendingBlock::blank_lines(line_number, line);

                match &mut pending_entry {
                    Some(pending) if pending.can_absorb_blank() => {
                        pending.add_line(line, line_number);
                        if matches!(
                            pending.merge_type(),
                            Some(MergeType::CodeWithBlanks) | Some(MergeType::Comment)
                        ) {
                            pending.has_absorbed_blanks = true;
                        }
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
                        pending.add_line(line, line_number);
                        pending.increment_comment_count();
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
            match try_parse_alias(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    let (pending_entry_to_add, merged) =
                        merge_pending_with_structured(pending_entry.take(), entry, |b| {
                            self.build_entry_from_pending(b)
                        });
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    pending_entry = Some(entry_to_trailing_pending(merged));
                    continue;
                }
                ParseEvent::Started { .. } => {
                    unreachable!("PowerShell aliases should not return Started");
                }
                ParseEvent::None => {}
            }

            // Try env var (handles both single-line and Here-String start)
            match try_parse_env(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    let (pending_entry_to_add, merged) =
                        merge_pending_with_structured(pending_entry.take(), entry, |b| {
                            self.build_entry_from_pending(b)
                        });
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    pending_entry = Some(entry_to_trailing_pending(merged));
                    continue;
                }
                ParseEvent::Started {
                    entry_type,
                    name,
                    boundary,
                    first_line,
                } => {
                    let (merged_first_line, start_line) =
                        if let Some(pending) = pending_entry.take() {
                            if pending.can_merge_down() {
                                let merged = format!("{}\n{}", pending.raw_content(), first_line);
                                (merged, pending.start_line)
                            } else {
                                result.add_entry(self.build_entry_from_pending(pending));
                                (first_line.to_string(), line_number)
                            }
                        } else {
                            (first_line.to_string(), line_number)
                        };
                    active_block = Some(PendingBlock {
                        lines: vec![merged_first_line],
                        start_line,
                        end_line: line_number,
                        boundary,
                        entry_hint: Some(entry_type),
                        name: Some(name),
                        value: None,
                        comment_count: 0,
                        has_absorbed_blanks: false,
                    });
                    continue;
                }
                ParseEvent::None => {}
            }

            // Try source
            match try_parse_source(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    let (pending_entry_to_add, merged) =
                        merge_pending_with_structured(pending_entry.take(), entry, |b| {
                            self.build_entry_from_pending(b)
                        });
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    pending_entry = Some(entry_to_trailing_pending(merged));
                    continue;
                }
                ParseEvent::Started { .. } => {
                    unreachable!("Source statements should not return Started");
                }
                ParseEvent::None => {}
            }

            // Try function
            if let Some(func_name) = detect_function_start(trimmed) {
                let (open, close) = count_braces_outside_quotes(trimmed);
                let brace_count = (open as i32).saturating_sub(close as i32);
                let is_single_line = brace_count == 0 && trimmed.contains('}');

                if is_single_line {
                    let entry = Entry::new(EntryType::Function, func_name, line.to_string())
                        .with_line_number(line_number)
                        .with_end_line(line_number);
                    let (pending_entry_to_add, merged) =
                        merge_pending_with_structured(pending_entry.take(), entry, |b| {
                            self.build_entry_from_pending(b)
                        });
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    pending_entry = Some(entry_to_trailing_pending(merged));
                } else {
                    let (merged_first_line, start_line) =
                        if let Some(pending) = pending_entry.take() {
                            if pending.can_merge_down() {
                                let merged = format!("{}\n{}", pending.raw_content(), line);
                                (merged, pending.start_line)
                            } else {
                                result.add_entry(self.build_entry_from_pending(pending));
                                (line.to_string(), line_number)
                            }
                        } else {
                            (line.to_string(), line_number)
                        };
                    active_block = Some(PendingBlock::function(
                        func_name,
                        start_line,
                        &merged_first_line,
                        brace_count,
                    ));
                }
                continue;
            }

            // Try pipeline/scriptblock
            if let Some(block_name) = detect_scriptblock_start(trimmed) {
                let (open, close) = count_braces_outside_quotes(trimmed);
                let brace_count = (open as i32).saturating_sub(close as i32);
                let is_single_line = brace_count == 0 && trimmed.contains('}');

                if is_single_line {
                    let entry = Entry::new(EntryType::Code, block_name, line.to_string())
                        .with_line_number(line_number)
                        .with_end_line(line_number);
                    let (pending_entry_to_add, merged) =
                        merge_pending_with_structured(pending_entry.take(), entry, |b| {
                            self.build_entry_from_pending(b)
                        });
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    pending_entry = Some(entry_to_trailing_pending(merged));
                } else {
                    let (merged_first_line, start_line) =
                        if let Some(pending) = pending_entry.take() {
                            if matches!(
                                pending.entry_hint,
                                Some(EntryType::Comment) | Some(EntryType::Code)
                            ) && !pending.has_absorbed_blanks
                            {
                                let mut lines = pending.lines;
                                lines.push(line.to_string());
                                (lines, pending.start_line)
                            } else {
                                result.add_entry(self.build_entry_from_pending(pending));
                                (vec![line.to_string()], line_number)
                            }
                        } else {
                            (vec![line.to_string()], line_number)
                        };

                    active_block = Some(PendingBlock {
                        lines: merged_first_line,
                        start_line,
                        end_line: line_number,
                        boundary: BoundaryType::BraceCounting { brace_count },
                        entry_hint: Some(EntryType::Code),
                        name: Some(block_name),
                        value: None,
                        comment_count: 0,
                        has_absorbed_blanks: false,
                    });
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Fallback: capture as non-blank Code
            // ------------------------------------------------------------------
            match &mut pending_entry {
                Some(pending) if pending.entry_hint == Some(EntryType::Comment) => {
                    if pending.can_merge_down() {
                        pending.add_line(line, line_number);
                        pending.upgrade_to_code();
                    } else {
                        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(PendingBlock::code(line_number, line));
                    }
                }
                Some(pending) if pending.entry_hint == Some(EntryType::Code) => {
                    if pending.merge_type() == Some(MergeType::CodeWithBlanks)
                        && !pending.has_absorbed_blanks
                    {
                        pending.add_line(line, line_number);
                    } else {
                        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(PendingBlock::code(line_number, line));
                    }
                }
                Some(_) => {
                    // Flush pending, start new pending Code
                    if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(entry);
                    }
                    pending_entry = Some(PendingBlock::code(line_number, line));
                }
                None => {
                    pending_entry = Some(PendingBlock::code(line_number, line));
                }
            }
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
                // Store complete function definition (Raw Value Architecture)
                Entry::new(EntryType::Function, name, raw_content)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
            }
            EntryType::EnvVar => {
                let name = block.name.unwrap_or_else(|| "UNKNOWN".to_string());
                // Always use raw_content to preserve complete syntax (Raw Value Architecture)
                Entry::new(EntryType::EnvVar, name, raw_content)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
            }
            EntryType::Comment => {
                let name = if block.start_line == block.end_line {
                    format!("#L{}", block.start_line)
                } else {
                    format!("#L{}-L{}", block.start_line, block.end_line)
                };
                // Store complete comment content (Raw Value Architecture)
                Entry::new(EntryType::Comment, name, raw_content)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
            }
            EntryType::Code => {
                let name = block.name.unwrap_or_else(|| {
                    if block.start_line == block.end_line {
                        format!("L{}", block.start_line)
                    } else {
                        format!("L{}-L{}", block.start_line, block.end_line)
                    }
                });
                // Store complete code content (Raw Value Architecture)
                Entry::new(EntryType::Code, name, raw_content)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
            }
            EntryType::Alias | EntryType::Source => {
                let name = block.name.unwrap_or_else(|| "unknown".to_string());
                Entry::new(entry_type, name, raw_content)
                    .with_line_number(block.start_line)
                    .with_end_line(block.end_line)
            }
        }
    }

    /// Extract function body from raw content.
    #[allow(dead_code)]
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
        assert_eq!(aliases[0].value, "Set-Alias ll Get-ChildItem");
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
        assert_eq!(envs[0].value, r#"$env:EDITOR = "code""#);
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

        assert_eq!(blanks.len(), 0);
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
        assert_eq!(
            envs[0].value,
            r#"$env:PATH = @"
C:\Program Files\bin
D:\tools
"@"#
        );
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
            r#"$env:CONFIG = @"
  line with leading spaces
    indented line
"@"#
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
        assert_eq!(envs[0].value, r#"$env:EDITOR = "code""#);
        assert_eq!(envs[0].line_number, Some(1));
        assert_eq!(envs[0].end_line, Some(1));
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
        assert_eq!(envs[0].value, r#"$env:EDITOR = "code""#);
        assert_eq!(envs[1].name, "PATH");
        assert_eq!(
            envs[1].value,
            r#"$env:PATH = @"
C:\bin
D:\tools
"@"#
        );
        assert_eq!(envs[2].name, "SHELL");
        assert_eq!(envs[2].value, r#"$env:SHELL = "pwsh""#);
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
        assert_eq!(
            envs[0].value,
            r#"$env:DATA = @"
line1

line3
"@"#
        );
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

        // Check value (contains complete content)
        assert!(code_blocks.iter().any(|c| c.value.contains("$env:PATH")));
    }

    // === Tests for Comment/Code merging logic ===

    #[test]
    fn test_comment_absorbs_blank() {
        let parser = PowerShellParser::new();
        let content = "# Header\n";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].line_number, Some(1));
        assert_eq!(comments[0].end_line, Some(1));
    }

    #[test]
    fn test_comment_plus_code_becomes_code() {
        let parser = PowerShellParser::new();
        let content = "# Note\nWrite-Host 'hello'";
        let result = parser.parse(content);

        // Should be merged into a single Code entry
        assert_eq!(result.entries.len(), 1);

        let code = &result.entries[0];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.line_number, Some(1));
        assert_eq!(code.end_line, Some(2));
        // value contains complete content (comment + code)
        assert_eq!(code.value, "# Note\nWrite-Host 'hello'");
    }

    #[test]
    fn test_comment_blank_code_separate() {
        let parser = PowerShellParser::new();
        let content = "# Header\n\nWrite-Host 'hi'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);

        let comment = &result.entries[0];
        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(2)); // Absorbs blank line

        let code = &result.entries[1];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.line_number, Some(3));
        assert_eq!(code.end_line, Some(3));
        assert_eq!(code.value, "Write-Host 'hi'");
    }

    #[test]
    fn test_blank_does_not_absorb_code() {
        let parser = PowerShellParser::new();
        let content = "\nWrite-Host 'hi'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);

        let blank = &result.entries[0];
        assert_eq!(blank.entry_type, EntryType::Code);
        assert_eq!(blank.is_blank(), true);
        assert_eq!(blank.line_number, Some(1));

        let code = &result.entries[1];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.value, "Write-Host 'hi'");
        assert_eq!(code.line_number, Some(2));
    }

    #[test]
    fn test_nonblank_code_absorbs_trailing_blanks() {
        let parser = PowerShellParser::new();
        let content = "Write-Host 'hi'\n\n\n";
        let result = parser.parse(content);

        // Non-blank code absorbs trailing blank lines
        assert_eq!(result.entries.len(), 1);

        let code = &result.entries[0];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.value, "Write-Host 'hi'\n\n");
        assert_eq!(code.line_number, Some(1));
        assert_eq!(code.end_line, Some(3));
    }

    #[test]
    fn test_nonblank_code_blank_then_another_code() {
        let parser = PowerShellParser::new();
        let content = "Write-Host 'first'\n\nWrite-Host 'second'";
        let result = parser.parse(content);

        // When second code comes, first code (with absorbed blank) is flushed
        assert_eq!(result.entries.len(), 2);

        let first = &result.entries[0];
        assert_eq!(first.entry_type, EntryType::Code);
        assert_eq!(first.value, "Write-Host 'first'\n");
        assert_eq!(first.line_number, Some(1));
        assert_eq!(first.end_line, Some(2)); // Absorbed one blank line

        let second = &result.entries[1];
        assert_eq!(second.entry_type, EntryType::Code);
        assert_eq!(second.value, "Write-Host 'second'");
        assert_eq!(second.line_number, Some(3));
    }

    #[test]
    fn test_comment_blank_then_code_separate() {
        let parser = PowerShellParser::new();
        let content = "# note\n\nWrite-Host 'hi'\n# another\nWrite-Host 'bye'";
        let result = parser.parse(content);

        // #note + blank → Comment(L1-L2) sealed
        // Write-Host hi → Code(L3)
        // #another + Write-Host bye → Code(L4-L5) merged (single comment, no blank gap)
        assert_eq!(result.entries.len(), 3);

        assert_eq!(result.entries[0].entry_type, EntryType::Comment);
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(2));

        assert_eq!(result.entries[1].entry_type, EntryType::Code);
        assert_eq!(result.entries[1].line_number, Some(3));

        assert_eq!(result.entries[2].entry_type, EntryType::Code);
        assert_eq!(result.entries[2].line_number, Some(4));
        assert_eq!(result.entries[2].end_line, Some(5));
    }

    #[test]
    fn test_comment_then_alias_not_merged() {
        let parser = PowerShellParser::new();
        let content = "# Section header\nSet-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.entry_type, EntryType::Alias);
        assert_eq!(entry.name, "ll");
        assert_eq!(entry.line_number, Some(1));
        assert!(entry.value.starts_with("# Section header"));
    }

    #[test]
    fn test_comment_blank_alias_scenario() {
        let parser = PowerShellParser::new();
        let content = "# Section header\n# with description\n\nSet-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);

        let comment = &result.entries[0];
        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(3)); // Absorbed blank line

        let alias = &result.entries[1];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.line_number, Some(4));
    }

    #[test]
    fn test_control_structure_absorbs_trailing_blanks() {
        let parser = PowerShellParser::new();
        let content = "if ($true) {\n    Write-Host 'yes'\n}\n\n";
        let result = parser.parse(content);

        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code)
            .collect();

        assert_eq!(code_blocks.len(), 1);
        assert_eq!(code_blocks[0].line_number, Some(1));
        assert_eq!(code_blocks[0].end_line, Some(4)); // Absorbed trailing blank
    }

    #[test]
    fn test_comment_merges_into_control_structure() {
        let parser = PowerShellParser::new();
        let content = "# This is a conditional\nif ($true) {\n    Write-Host 'yes'\n}";
        let result = parser.parse(content);

        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code)
            .collect();

        assert_eq!(code_blocks.len(), 1);
        assert_eq!(code_blocks[0].line_number, Some(1)); // Starts from comment
        assert_eq!(code_blocks[0].end_line, Some(4));
        // value should contain comment + control structure
        assert!(code_blocks[0].value.contains("# This is a conditional"));
        assert!(code_blocks[0].value.contains("if ($true)"));
    }

    #[test]
    fn test_single_comment_merges_into_alias() {
        let parser = PowerShellParser::new();
        let content = "# My alias\nSet-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[0].name, "ll");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(2));
    }

    #[test]
    fn test_multi_comment_does_not_merge_into_alias() {
        let parser = PowerShellParser::new();
        let content = "# Header\n# Description\nSet-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].entry_type, EntryType::Comment);
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
    }

    #[test]
    fn test_alias_absorbs_trailing_blanks() {
        let parser = PowerShellParser::new();
        let content = "Set-Alias ll Get-ChildItem\n\n\nSet-Alias gs git";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[0].name, "ll");
        assert_eq!(result.entries[0].end_line, Some(3));
    }

    #[test]
    fn test_adjacent_code_lines_merge() {
        let parser = PowerShellParser::new();
        let content = "Write-Host 'a'\nWrite-Host 'b'\nWrite-Host 'c'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Code);
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(3));
    }

    #[test]
    fn test_code_with_blank_gap_splits() {
        let parser = PowerShellParser::new();
        let content = "Write-Host 'a'\n\nWrite-Host 'b'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].end_line, Some(2));
        assert_eq!(result.entries[1].line_number, Some(3));
    }

    #[test]
    fn test_function_absorbs_trailing_blanks() {
        let parser = PowerShellParser::new();
        let content = "function Get-Name {\n    'test'\n}\n\nSet-Alias ll Get-ChildItem";
        let result = parser.parse(content);

        let funcs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Function)
            .collect();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].end_line, Some(4));
    }

    #[test]
    fn test_single_comment_merges_into_function() {
        let parser = PowerShellParser::new();
        let content = "# Helper\nfunction Get-Name {\n    'test'\n}";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Function);
        assert_eq!(result.entries[0].name, "Get-Name");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert!(result.entries[0].value.starts_with("# Helper"));
    }

    #[test]
    fn test_env_absorbs_trailing_blanks() {
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
    }

    #[test]
    fn test_pipeline_multiline_block() {
        let parser = PowerShellParser::new();
        let content = "1..9 | ForEach-Object {\n    $num = $_\n    Write-Host $num\n}";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Code);
        assert_eq!(result.entries[0].name, "ForEach-Object");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(4));
    }

    #[test]
    fn test_pipeline_block_with_internal_blanks() {
        let parser = PowerShellParser::new();
        let content = "1..9 | ForEach-Object {\n    $num = $_\n\n    Write-Host $num\n}";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Code);
        assert!(result.entries[0].value.contains("\n\n"));
    }

    #[test]
    fn test_pipeline_with_preceding_comment() {
        let parser = PowerShellParser::new();
        let content = "# Build numbers\n1..9 | ForEach-Object {\n    $_\n}";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].line_number, Some(1));
        assert!(result.entries[0].value.starts_with("# Build numbers"));
    }

    #[test]
    fn test_pipeline_absorbs_trailing_blanks() {
        let parser = PowerShellParser::new();
        let content = "1..9 | ForEach-Object {\n    $_\n}\n\n\nWrite-Host 'done'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].end_line, Some(5));
        assert_eq!(result.entries[1].line_number, Some(6));
    }

    #[test]
    fn test_block_comment_single() {
        let parser = PowerShellParser::new();
        let content = "<#\nA comment\n#>";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].line_number, Some(1));
        assert_eq!(comments[0].end_line, Some(3));
    }

    #[test]
    fn test_block_comment_does_not_merge_with_hash_comment() {
        let parser = PowerShellParser::new();
        let content = "<#\nBlock\n#>\n# Regular\nWrite-Host 'hi'";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
        assert!(comments[0].value.contains("<#"));
        assert!(!comments[0].value.contains("# Regular"));
    }

    #[test]
    fn test_block_comment_with_surrounding_entries() {
        let parser = PowerShellParser::new();
        let content = "Set-Alias ll Get-ChildItem\n\n<#\nA note\n#>\n\nSet-Alias gs git";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 3);
        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn test_scriptblock_assign_multiline() {
        let parser = PowerShellParser::new();
        let content = "$block = {\n    Write-Host 'hi'\n}";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Code);
        assert_eq!(result.entries[0].name, "$block");
    }
}
