//! # Bash Parser
//!
//! Parses `.bashrc` and similar Bash configuration files.
//!
//! ## Supported Entry Types
//!
//! | Type | Pattern | Multi-line |
//! |------|---------|------------|
//! | Alias | `alias name='value'` | ✅ Single-quote boundary |
//! | EnvVar | `export VAR=value` | ✅ Single-quote boundary |
//! | Function | `func() { ... }` | ✅ Brace counting |
//! | Source | `source file` or `. file` | ❌ |
//! | Comment | `# text` | ✅ Adjacent merging |
//! | Code | Control structures, other | ✅ Keyword tracking |
//!
//! ## Module Structure
//!
//! - [`patterns`] - Regex definitions for syntax matching
//! - [`control`] - Control structure detection (`if`/`fi`, etc.)
//! - [`parsers`] - Individual entry parsing methods
//!
//! ## Parsing State Machine
//!
//! The parser uses a unified PendingBlock state machine. All entries pass
//! through the pending pattern:
//!
//! 1. Detect entry start and create PendingBlock with appropriate boundary type
//! 2. Accumulate lines until boundary condition is satisfied
//! 3. Build Entry from completed PendingBlock
//!
//! ## Multi-line Detection
//!
//! | Entry Type | Start Detection | End Detection |
//! |------------|-----------------|---------------|
//! | Function | `func() {` | brace_count = 0 |
//! | Code Block | `if`/`while`/`for`/`case` | `fi`/`done`/`esac` |
//! | Alias/Env | Odd single quotes | Even single quotes |
//! | Comment | Line starts with `#` | Non-`#` line |

pub mod control;
pub mod parsers;
pub mod patterns;

use crate::model::{Entry, EntryType, ParseResult, ShellType};
use crate::parser::builders::{count_braces_outside_quotes, CommentBlockBuilder};
use crate::parser::pending::{BoundaryType, MergeType, PendingBlock};
use crate::parser::Parser;

use control::{count_control_end, count_control_start};
use parsers::{detect_function_start, try_parse_alias, try_parse_env, try_parse_source};

use crate::parser::ParseEvent;

/// Bash configuration file parser.
///
/// Implements the [`Parser`] trait for parsing `.bashrc`, `.bash_profile`,
/// and similar Bash configuration files.
///
/// ## Example
///
/// ```rust,ignore
/// use wenv::parser::{Parser, BashParser};
///
/// let parser = BashParser::new();
/// let content = std::fs::read_to_string("~/.bashrc")?;
/// let result = parser.parse(&content);
///
/// for entry in result.entries {
///     println!("{}: {}", entry.entry_type, entry.name);
/// }
/// ```
pub struct BashParser;

impl BashParser {
    /// Create a new Bash parser instance.
    pub fn new() -> Self {
        Self
    }

    /// Build an Entry from a completed PendingBlock.
    fn build_entry_from_pending(&self, block: PendingBlock) -> Entry {
        let entry_type = block.entry_hint.unwrap_or(EntryType::Code);
        let raw_content = block.raw_content();

        // Determine name and value based on entry type
        let (name, value) = match entry_type {
            EntryType::Function => {
                let name = block
                    .name
                    .unwrap_or_else(|| format!("L{}", block.start_line));
                // Extract function body from raw content
                let body = self.extract_function_body(&raw_content);
                (name, body)
            }
            EntryType::Alias | EntryType::EnvVar => {
                let name = block
                    .name
                    .unwrap_or_else(|| format!("L{}", block.start_line));
                let value = block.value.unwrap_or_else(|| raw_content.clone());
                (name, value)
            }
            EntryType::Comment => {
                let prefix = if block.start_line == block.end_line {
                    format!("#L{}", block.start_line)
                } else {
                    format!("#L{}-L{}", block.start_line, block.end_line)
                };
                // Value is first line for display
                let first_line = block.lines.first().cloned().unwrap_or_default();
                (prefix, first_line)
            }
            EntryType::Code => {
                let prefix = if block.start_line == block.end_line {
                    format!("L{}", block.start_line)
                } else {
                    format!("L{}-L{}", block.start_line, block.end_line)
                };
                // Value is first non-blank line for display (for control blocks with absorbed blank lines)
                let first_non_blank = block
                    .lines
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| block.lines.first().cloned().unwrap_or_default());
                (prefix, first_non_blank)
            }
            EntryType::Source => {
                let name = format!("L{}", block.start_line);
                let value = block.value.unwrap_or_else(|| raw_content.clone());
                (name, value)
            }
        };

        Entry::new(entry_type, name, value)
            .with_line_number(block.start_line)
            .with_end_line(block.end_line)
            .with_raw_line(raw_content)
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

    /// Flush pending Comment/Code block and return Entry if exists.
    fn flush_pending_comment_code(&self, pending: &mut Option<PendingBlock>) -> Option<Entry> {
        pending
            .take()
            .map(|block| self.build_entry_from_pending(block))
    }
}

impl Default for BashParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for BashParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut result = ParseResult::new();

        // === Unified pending state ===

        // For multi-line structures: function, control block, multi-line alias/env
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
            // Handle active multi-line block (function, control, alias, env)
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
                    BoundaryType::QuoteCounting {
                        ref mut quote_count,
                    } => {
                        // Multi-line alias or env
                        *quote_count += line.matches('\'').count();

                        if *quote_count % 2 == 0 {
                            let mut completed = active_block.take().unwrap();
                            // Extract value from complete content
                            let raw = completed.raw_content();
                            completed.value = Some(self.extract_quoted_value(&raw));
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

                            // Make result pending for trailing blank absorption
                            let completed = active_block.take().unwrap();
                            pending_entry = Some(PendingBlock {
                                lines: completed.lines,
                                start_line: completed.start_line,
                                end_line: completed.end_line,
                                boundary: BoundaryType::AdjacentMerging {
                                    merge_type: MergeType::CodeWithBlanks,
                                },
                                entry_hint: Some(EntryType::Code),
                                name: None,
                                value: None,
                            });
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
            // Handle empty lines (part of pending entry state machine)
            // ------------------------------------------------------------------
            if trimmed.is_empty() {
                let blank = PendingBlock::blank_lines(line_number, line);

                match &mut pending_entry {
                    Some(pending) if pending.can_absorb_blank() => {
                        // Comment, BlankLines, or CodeWithBlanks absorbs blank
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
            match try_parse_alias(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    // Flush pending entry
                    if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(e);
                    }
                    result.add_entry(entry);
                    continue;
                }
                ParseEvent::Started {
                    entry_type,
                    name,
                    boundary,
                    first_line,
                } => {
                    // Flush pending entry
                    if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(e);
                    }
                    // Create PendingBlock directly from ParseEvent
                    active_block = Some(PendingBlock {
                        lines: vec![first_line],
                        start_line: line_number,
                        end_line: line_number,
                        boundary,
                        entry_hint: Some(entry_type),
                        name: Some(name),
                        value: None,
                    });
                    continue;
                }
                ParseEvent::None => {}
            }

            // Try export (environment variable)
            match try_parse_env(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    // Flush pending entry
                    if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(e);
                    }
                    result.add_entry(entry);
                    continue;
                }
                ParseEvent::Started {
                    entry_type,
                    name,
                    boundary,
                    first_line,
                } => {
                    // Flush pending entry
                    if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(e);
                    }
                    // Create PendingBlock directly from ParseEvent
                    active_block = Some(PendingBlock {
                        lines: vec![first_line],
                        start_line: line_number,
                        end_line: line_number,
                        boundary,
                        entry_hint: Some(entry_type),
                        name: Some(name),
                        value: None,
                    });
                    continue;
                }
                ParseEvent::None => {}
            }

            // Try source
            match try_parse_source(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    // Flush pending entry
                    if let Some(e) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(e);
                    }
                    result.add_entry(entry);
                    continue;
                }
                ParseEvent::Started { .. } => {
                    // Source statements are currently single-line only,
                    // but we handle this case for API consistency
                    unreachable!("Source statements should not return Started");
                }
                ParseEvent::None => {}
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
            // Fallback: capture as non-blank Code
            // ------------------------------------------------------------------
            match &mut pending_entry {
                Some(pending) if pending.entry_hint == Some(EntryType::Comment) => {
                    // Comment + non-blank Code → merge and upgrade to Code
                    pending.add_line(line, line_number);
                    pending.upgrade_to_code();
                }
                Some(pending) if pending.entry_hint == Some(EntryType::Code) => {
                    // Non-blank Code pending + new non-blank Code → flush pending, new pending
                    if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                        result.add_entry(entry);
                    }
                    pending_entry = Some(PendingBlock::code(line_number, line));
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
                Some(EntryType::Alias) => "Unclosed multi-line alias at end of file",
                Some(EntryType::EnvVar) => "Unclosed multi-line export at end of file",
                _ => "Unclosed block at end of file",
            };
            result.add_warning(crate::model::ParseWarning::new(block.start_line, msg, ""));
        }

        result
    }

    fn shell_type(&self) -> ShellType {
        ShellType::Bash
    }
}

impl BashParser {
    /// Extract quoted value from raw multi-line content.
    fn extract_quoted_value(&self, raw: &str) -> String {
        // Find the first quote and extract value
        if let Some(start) = raw.find('\'') {
            if let Some(end) = raw.rfind('\'') {
                if start < end {
                    return raw[start + 1..end].to_string();
                }
            }
        }
        // Fallback: try double quotes
        if let Some(start) = raw.find('"') {
            if let Some(end) = raw.rfind('"') {
                if start < end {
                    return raw[start + 1..end].to_string();
                }
            }
        }
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_alias_single_quote() {
        let parser = BashParser::new();
        let content = "alias ll='ls -la'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[0].name, "ll");
        assert_eq!(result.entries[0].value, "ls -la");
    }

    #[test]
    fn test_parse_alias_double_quote() {
        let parser = BashParser::new();
        let content = r#"alias gs="git status""#;
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "gs");
        assert_eq!(result.entries[0].value, "git status");
    }

    #[test]
    fn test_parse_multiline_alias() {
        let parser = BashParser::new();
        let content = "alias complex='echo line1\necho line2\necho line3'";
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();

        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].name, "complex");
        assert_eq!(aliases[0].line_number, Some(1));
        assert_eq!(aliases[0].end_line, Some(3));
        assert!(aliases[0].value.contains("line1"));
        assert!(aliases[0].value.contains("line2"));
        assert!(aliases[0].value.contains("line3"));
    }

    #[test]
    fn test_parse_multiline_export() {
        let parser = BashParser::new();
        let content = "export LONG='value1\nvalue2\nvalue3'";
        let result = parser.parse(content);

        let exports: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar)
            .collect();

        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].name, "LONG");
        assert_eq!(exports[0].line_number, Some(1));
        assert_eq!(exports[0].end_line, Some(3));
    }

    #[test]
    fn test_parse_export() {
        let parser = BashParser::new();
        let content = "export EDITOR=nvim";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::EnvVar);
        assert_eq!(result.entries[0].name, "EDITOR");
        assert_eq!(result.entries[0].value, "nvim");
    }

    #[test]
    fn test_parse_source() {
        let parser = BashParser::new();
        let content = "source ~/.aliases";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Source);
        assert_eq!(result.entries[0].name, ".aliases");
        assert_eq!(result.entries[0].value, "~/.aliases");
    }

    #[test]
    fn test_parse_function_with_end_line() {
        let parser = BashParser::new();
        let content = "greet() {\n    echo hello\n    echo world\n}";
        let result = parser.parse(content);

        let funcs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Function)
            .collect();

        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "greet");
        assert_eq!(funcs[0].line_number, Some(1));
        assert_eq!(funcs[0].end_line, Some(4));
    }

    #[test]
    fn test_adjacent_comments_merged() {
        let parser = BashParser::new();
        let content = "# Header comment\n# Second line\n# Third line\nalias test='value'";
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
    fn test_comments_separated_by_blank() {
        // With new merging logic: Comment + blank + Comment → first Comment absorbs blank
        // Then when second Comment comes, since pending is still Comment (absorbed blank),
        // the second Comment merges into it
        let parser = BashParser::new();
        let content = "# First block\n\n# Second block";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();

        // All merged into one Comment spanning L1-L3
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].line_number, Some(1));
        assert_eq!(comments[0].end_line, Some(3));
    }

    #[test]
    fn test_comments_separated_by_code() {
        let parser = BashParser::new();
        let content = "# First block\nalias a='a'\n# Second block";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();

        assert_eq!(comments.len(), 2);
    }

    #[test]
    fn test_control_structure_captured_as_code() {
        let parser = BashParser::new();
        let content = "if [ -f file ]; then\n    echo exists\nfi";
        let result = parser.parse(content);

        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("if"))
            .collect();

        assert_eq!(code_blocks.len(), 1);
        assert_eq!(code_blocks[0].line_number, Some(1));
        assert_eq!(code_blocks[0].end_line, Some(3));
        assert_eq!(code_blocks[0].name, "L1-L3");
    }

    #[test]
    fn test_empty_lines_grouped() {
        let parser = BashParser::new();
        let content = "alias a='a'\n\n\nalias b='b'";
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
    fn test_special_alias_names() {
        let parser = BashParser::new();
        let content = "alias ..='cd ..'\nalias ~='cd ~'";
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();

        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name, "..");
        assert_eq!(aliases[1].name, "~");
    }

    // === New tests for Comment/Code merging logic ===

    #[test]
    fn test_comment_absorbs_blank() {
        // # Header
        // (blank at EOF - no actual blank line character)
        // → 1 entry: Comment L1 with end_line=L1
        let parser = BashParser::new();
        let content = "# Header\n";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].line_number, Some(1));
        // Single line comment has end_line = line_number
        assert_eq!(comments[0].end_line, Some(1));
    }

    #[test]
    fn test_comment_plus_code_becomes_code() {
        // # Note
        // echo hello
        // → 1 entry: Code L1-L2, raw_line contains both lines
        let parser = BashParser::new();
        let content = "# Note\necho hello";
        let result = parser.parse(content);

        // Should be merged into a single Code entry
        assert_eq!(result.entries.len(), 1);

        let code = &result.entries[0];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.line_number, Some(1));
        assert_eq!(code.end_line, Some(2));
        // value preserves Comment's first line for list display
        assert_eq!(code.value, "# Note");
        // raw_line contains complete content (comment + code)
        assert_eq!(code.raw_line, Some("# Note\necho hello".to_string()));
        // comment field is no longer set - raw_line has complete content
    }

    #[test]
    fn test_comment_blank_code_all_merge() {
        // # Header
        // (blank)
        // echo hi
        // → 1 entry: Code L1-L3
        let parser = BashParser::new();
        let content = "# Header\n\necho hi";
        let result = parser.parse(content);

        // Should be merged into a single Code entry
        assert_eq!(result.entries.len(), 1);

        let code = &result.entries[0];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.line_number, Some(1));
        assert_eq!(code.end_line, Some(3));
        // value preserves Comment's first line for list display
        assert_eq!(code.value, "# Header");
        // raw_line contains complete content (comment + blank + code)
        assert_eq!(code.raw_line, Some("# Header\n\necho hi".to_string()));
        // comment field is no longer set - raw_line has complete content
    }

    #[test]
    fn test_blank_does_not_absorb_code() {
        // (blank)
        // echo hi
        // → 2 entries: Code L1 (空), Code L2
        let parser = BashParser::new();
        let content = "\necho hi";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);

        let blank = &result.entries[0];
        assert_eq!(blank.entry_type, EntryType::Code);
        assert_eq!(blank.is_blank(), true);
        assert_eq!(blank.line_number, Some(1));

        let code = &result.entries[1];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.value, "echo hi");
        assert_eq!(code.line_number, Some(2));
    }

    #[test]
    fn test_multiple_blanks_merge() {
        // (blank)
        // (blank)
        // (blank)
        // → 1 entry: Code L1-L3 (empty)
        let parser = BashParser::new();
        let content = "\n\n\n";
        let result = parser.parse(content);

        let blanks: Vec<_> = result.entries.iter().filter(|e| e.is_blank()).collect();

        assert_eq!(blanks.len(), 1);
        assert_eq!(blanks[0].line_number, Some(1));
        assert_eq!(blanks[0].end_line, Some(3));
    }

    #[test]
    fn test_comment_then_alias_not_merged() {
        // # Section header
        // alias ll='ls -la'
        // → Comment L1 flushed, Alias L2
        let parser = BashParser::new();
        let content = "# Section header\nalias ll='ls -la'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);

        let comment = &result.entries[0];
        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1));

        let alias = &result.entries[1];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.name, "ll");
    }

    #[test]
    fn test_comment_blank_alias_scenario() {
        // # Section header
        // # with description
        // (blank)
        // alias ll='ls -la'
        // Expected: Comment L1-L3, Alias L4
        let parser = BashParser::new();
        let content = "# Section header\n# with description\n\nalias ll='ls -la'";
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
    fn test_nonblank_code_absorbs_trailing_blanks() {
        // echo hi
        // (blank)
        // (blank)
        // → 1 entry: Code L1-L3
        let parser = BashParser::new();
        let content = "echo hi\n\n\n";
        let result = parser.parse(content);

        // Non-blank code absorbs trailing blank lines
        assert_eq!(result.entries.len(), 1);

        let code = &result.entries[0];
        assert_eq!(code.entry_type, EntryType::Code);
        assert_eq!(code.value, "echo hi");
        assert_eq!(code.line_number, Some(1));
        assert_eq!(code.end_line, Some(3));
    }

    #[test]
    fn test_nonblank_code_blank_then_another_code() {
        // echo first
        // (blank)
        // echo second
        // → 2 entries: Code L1-L2, Code L3
        let parser = BashParser::new();
        let content = "echo first\n\necho second";
        let result = parser.parse(content);

        // When second code comes, first code (with absorbed blank) is flushed
        assert_eq!(result.entries.len(), 2);

        let first = &result.entries[0];
        assert_eq!(first.entry_type, EntryType::Code);
        assert_eq!(first.value, "echo first");
        assert_eq!(first.line_number, Some(1));
        assert_eq!(first.end_line, Some(2)); // Absorbed one blank line

        let second = &result.entries[1];
        assert_eq!(second.entry_type, EntryType::Code);
        assert_eq!(second.value, "echo second");
        assert_eq!(second.line_number, Some(3));
    }

    #[test]
    fn test_trailing_blank_lines_preserved_in_raw_line() {
        // Test that trailing blank lines are preserved in raw_line
        // "#\n\n" should be 3 lines (comment, blank, blank), not 2
        let parser = BashParser::new();

        // Case 1: Comment with trailing blank lines (no file terminator)
        let content = "# comment\n\n";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.entry_type, EntryType::Comment);
        assert_eq!(entry.line_number, Some(1));
        assert_eq!(entry.end_line, Some(2)); // Comment absorbs the blank line
                                             // raw_line should contain "# comment\n" (2 lines: comment + blank)
        assert_eq!(entry.raw_line, Some("# comment\n".to_string()));
    }

    #[test]
    fn test_multiple_trailing_blanks_preserved() {
        // "# comment\n\n\n" (with file terminator) should be 3 lines
        let parser = BashParser::new();

        // Multiple trailing blank lines
        let content = "# comment\n\n\n";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.entry_type, EntryType::Comment);
        assert_eq!(entry.line_number, Some(1));
        assert_eq!(entry.end_line, Some(3)); // Comment absorbs 2 blank lines
                                             // raw_line should contain all 3 lines
        assert_eq!(entry.raw_line, Some("# comment\n\n".to_string()));
    }

    #[test]
    fn test_trailing_blank_not_confused_with_file_terminator() {
        // Important: "# comment\n" is 1 line with file terminator
        // "# comment\n\n" is 2 lines: comment + blank (with file terminator)
        let parser = BashParser::new();

        // Single line with proper file terminator
        let content1 = "# comment\n";
        let result1 = parser.parse(content1);
        assert_eq!(result1.entries.len(), 1);
        assert_eq!(result1.entries[0].end_line, Some(1)); // Just 1 line

        // Comment + blank line (with file terminator)
        let content2 = "# comment\n\n";
        let result2 = parser.parse(content2);
        assert_eq!(result2.entries.len(), 1);
        assert_eq!(result2.entries[0].end_line, Some(2)); // 2 lines: comment + blank
    }
}
