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
use crate::parser::builders::{
    count_braces_outside_quotes, count_parens_outside_quotes, CommentBlockBuilder,
};
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

    /// Convert an entry to a pending block that can absorb trailing blanks.
    fn entry_to_trailing_pending(entry: Entry) -> PendingBlock {
        PendingBlock {
            lines: entry.value.split('\n').map(|s| s.to_string()).collect(),
            start_line: entry.line_number.unwrap_or(1),
            end_line: entry.end_line.unwrap_or(entry.line_number.unwrap_or(1)),
            boundary: BoundaryType::AdjacentMerging {
                merge_type: MergeType::CodeWithBlanks,
            },
            entry_hint: Some(entry.entry_type),
            name: Some(entry.name),
            value: None, // Don't set value - let build_entry_from_pending use raw_content
            comment_count: 0,
        }
    }

    /// Merge pending entry (Comment/Code/blank lines) with a structured entry.
    /// The pending content becomes a prefix to the structured entry's value.
    ///
    /// New merge rules:
    /// - Single comment (comment_count == 1): merge downward to structured entry
    /// - Multiple comments (comment_count > 1): don't merge, return as separate entries
    /// - Blank lines only (comment_count == 0): don't merge, return as separate entry
    /// - Structured entry pending (has stored value): flush and don't merge
    fn merge_pending_with_structured(
        pending: Option<PendingBlock>,
        entry: Entry,
        parser: &BashParser,
    ) -> (Option<Entry>, Entry) {
        if let Some(pending) = pending {
            // If pending has a stored value, it's a structured entry absorbing trailing blanks
            // Flush it using build_entry_from_pending to preserve proper name extraction
            if pending.is_structured_entry() {
                let flushed = parser.build_entry_from_pending(pending);
                return (Some(flushed), entry);
            }

            if pending.comment_count == 0 {
                // All blank lines → return as separate Code entry
                (
                    Some(
                        Entry::new(
                            EntryType::Code,
                            format!("L{}-L{}", pending.start_line, pending.end_line),
                            pending.raw_content(),
                        )
                        .with_line_number(pending.start_line)
                        .with_end_line(pending.end_line),
                    ),
                    entry,
                )
            } else if pending.comment_count == 1 {
                // Single comment → merge downward
                let pending_content = pending.raw_content();
                let merged_value = format!("{}\n{}", pending_content, entry.value);
                let end_line = entry
                    .end_line
                    .or(entry.line_number)
                    .unwrap_or(pending.start_line);

                (
                    None,
                    Entry::new(entry.entry_type, entry.name, merged_value)
                        .with_line_number(pending.start_line)
                        .with_end_line(end_line),
                )
            } else {
                // Multiple comments → return as separate Comment entry
                (
                    Some(
                        Entry::new(
                            EntryType::Comment,
                            format!("#L{}-L{}", pending.start_line, pending.end_line),
                            pending.raw_content(),
                        )
                        .with_line_number(pending.start_line)
                        .with_end_line(pending.end_line),
                    ),
                    entry,
                )
            }
        } else {
            (None, entry)
        }
    }

    /// Build an Entry from a completed PendingBlock.
    fn build_entry_from_pending(&self, block: PendingBlock) -> Entry {
        let entry_type = block.entry_hint.unwrap_or(EntryType::Code);
        let raw_content = block.raw_content();

        // Determine name and value based on entry type
        let (name, value) = match entry_type {
            EntryType::Function => {
                let mut name = block
                    .name
                    .unwrap_or_else(|| format!("L{}", block.start_line));

                // Update anonymous function name with end_line if it was a multi-line function
                if name.starts_with("(fL") && block.start_line != block.end_line {
                    name = format!("(fL{}-L{})", block.start_line, block.end_line);
                }

                // Store complete function definition in value (Raw Value Architecture)
                (name, raw_content)
            }
            EntryType::Alias | EntryType::EnvVar => {
                let name = block
                    .name
                    .unwrap_or_else(|| format!("L{}", block.start_line));
                // Use raw_content (includes all absorbed trailing blanks)
                (name, raw_content)
            }
            EntryType::Comment => {
                let prefix = if block.start_line == block.end_line {
                    format!("#L{}", block.start_line)
                } else {
                    format!("#L{}-L{}", block.start_line, block.end_line)
                };
                // Store complete comment content (Raw Value Architecture)
                (prefix, raw_content)
            }
            EntryType::Code => {
                let prefix = if block.start_line == block.end_line {
                    format!("L{}", block.start_line)
                } else {
                    format!("L{}-L{}", block.start_line, block.end_line)
                };
                // Store complete code content (Raw Value Architecture)
                (prefix, raw_content)
            }
            EntryType::Source => {
                // Use stored name if available (from trailing pending), otherwise generate from line
                let name = block
                    .name
                    .unwrap_or_else(|| format!("L{}", block.start_line));
                // Use raw_content (includes all absorbed trailing blanks)
                (name, raw_content)
            }
        };

        Entry::new(entry_type, name, value)
            .with_line_number(block.start_line)
            .with_end_line(block.end_line)
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
                            // Set as pending to absorb trailing blanks
                            pending_entry = Some(Self::entry_to_trailing_pending(entry));
                        }
                    }
                    BoundaryType::QuoteCounting {
                        ref mut quote_count,
                    } => {
                        // Multi-line alias or env
                        *quote_count += line.matches('\'').count();

                        if *quote_count % 2 == 0 {
                            let completed = active_block.take().unwrap();
                            // Don't set completed.value - let build_entry_from_pending use raw_content
                            let entry = self.build_entry_from_pending(completed);
                            // Set as pending to absorb trailing blanks
                            pending_entry = Some(Self::entry_to_trailing_pending(entry));
                        }
                    }
                    BoundaryType::ParenthesisCounting {
                        ref mut parenthesis_count,
                    } => {
                        // Multi-line parenthesis structure (e.g., plugins=(...))
                        let (open, close) = count_parens_outside_quotes(trimmed);
                        *parenthesis_count += open as i32;
                        *parenthesis_count = (*parenthesis_count).saturating_sub(close as i32);

                        if *parenthesis_count == 0 {
                            let entry = self.build_entry_from_pending(active_block.take().unwrap());
                            // Set as pending to absorb trailing blanks
                            pending_entry = Some(Self::entry_to_trailing_pending(entry));
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
                                comment_count: 0,
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
                                comment_count: pending.comment_count,
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
                    // Merge pending entry (if exists) with this structured entry
                    let (pending_entry_to_add, merged) =
                        Self::merge_pending_with_structured(pending_entry.take(), entry, self);
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    // Set merged entry as pending to absorb trailing blanks
                    pending_entry = Some(Self::entry_to_trailing_pending(merged));
                    continue;
                }
                ParseEvent::Started {
                    entry_type,
                    name,
                    boundary,
                    first_line,
                } => {
                    // For multi-line alias, check pending merge rules
                    let (merged_first_line, start_line) =
                        if let Some(pending) = pending_entry.take() {
                            // If pending has stored value, it's a trailing structured entry - flush it
                            if pending.is_structured_entry() {
                                result.add_entry(self.build_entry_from_pending(pending));
                                (first_line, line_number)
                            } else if pending.comment_count == 1 {
                                // Single comment can merge down
                                let merged = format!("{}\n{}", pending.raw_content(), first_line);
                                (merged, pending.start_line)
                            } else {
                                // Multiple comments or pure blank - don't merge
                                result.add_entry(self.build_entry_from_pending(pending));
                                (first_line, line_number)
                            }
                        } else {
                            (first_line, line_number)
                        };

                    // Create PendingBlock directly from ParseEvent
                    active_block = Some(PendingBlock {
                        lines: vec![merged_first_line],
                        start_line,
                        end_line: line_number,
                        boundary,
                        entry_hint: Some(entry_type),
                        name: Some(name),
                        value: None,
                        comment_count: 0,
                    });
                    continue;
                }
                ParseEvent::None => {}
            }

            // Try export (environment variable)
            match try_parse_env(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    // Merge pending entry (if exists) with this structured entry
                    let (pending_entry_to_add, merged) =
                        Self::merge_pending_with_structured(pending_entry.take(), entry, self);
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    // Set merged entry as pending to absorb trailing blanks
                    pending_entry = Some(Self::entry_to_trailing_pending(merged));
                    continue;
                }
                ParseEvent::Started {
                    entry_type,
                    name,
                    boundary,
                    first_line,
                } => {
                    // For multi-line env, check pending merge rules
                    let (merged_first_line, start_line) =
                        if let Some(pending) = pending_entry.take() {
                            // If pending has stored value, it's a trailing structured entry - flush it
                            if pending.is_structured_entry() {
                                result.add_entry(self.build_entry_from_pending(pending));
                                (first_line, line_number)
                            } else if pending.comment_count == 1 {
                                // Single comment can merge down
                                let merged = format!("{}\n{}", pending.raw_content(), first_line);
                                (merged, pending.start_line)
                            } else {
                                // Multiple comments or pure blank - don't merge
                                result.add_entry(self.build_entry_from_pending(pending));
                                (first_line, line_number)
                            }
                        } else {
                            (first_line, line_number)
                        };

                    // Create PendingBlock directly from ParseEvent
                    active_block = Some(PendingBlock {
                        lines: vec![merged_first_line],
                        start_line,
                        end_line: line_number,
                        boundary,
                        entry_hint: Some(entry_type),
                        name: Some(name),
                        value: None,
                        comment_count: 0,
                    });
                    continue;
                }
                ParseEvent::None => {}
            }

            // Try source
            match try_parse_source(trimmed, line_number) {
                ParseEvent::Complete(entry) => {
                    // Merge pending entry (if exists) with this structured entry
                    let (pending_entry_to_add, merged) =
                        Self::merge_pending_with_structured(pending_entry.take(), entry, self);
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    // Set merged entry as pending to absorb trailing blanks
                    pending_entry = Some(Self::entry_to_trailing_pending(merged));
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
            if let Some((func_name, is_anonymous)) = detect_function_start(trimmed) {
                let (open, close) = count_braces_outside_quotes(trimmed);
                let brace_count = (open as i32).saturating_sub(close as i32);

                // Check if single-line function
                let is_single_line = brace_count == 0 && trimmed.contains('}');

                // Generate name for anonymous function if needed
                let name = if is_anonymous {
                    if is_single_line {
                        format!("(fL{})", line_number)
                    } else {
                        // Will update with end_line later
                        format!("(fL{})", line_number)
                    }
                } else {
                    func_name
                };

                if is_single_line {
                    // Single-line function: build entry directly
                    let entry = Entry::new(EntryType::Function, name, line.to_string())
                        .with_line_number(line_number)
                        .with_end_line(line_number);
                    // Merge with pending
                    let (pending_entry_to_add, merged) =
                        Self::merge_pending_with_structured(pending_entry.take(), entry, self);
                    if let Some(pending_e) = pending_entry_to_add {
                        result.add_entry(pending_e);
                    }
                    // Set merged entry as pending to absorb trailing blanks
                    pending_entry = Some(Self::entry_to_trailing_pending(merged));
                } else {
                    // Multi-line function: check pending merge rules
                    let (merged_first_line, start_line) =
                        if let Some(pending) = pending_entry.take() {
                            // If pending has stored value, it's a trailing structured entry - flush it
                            if pending.is_structured_entry() {
                                result.add_entry(self.build_entry_from_pending(pending));
                                (line.to_string(), line_number)
                            } else if pending.comment_count == 1 {
                                // Single comment can merge down
                                let merged = format!("{}\n{}", pending.raw_content(), line);
                                (merged, pending.start_line)
                            } else {
                                // Multiple comments or pure blank - don't merge
                                result.add_entry(self.build_entry_from_pending(pending));
                                (line.to_string(), line_number)
                            }
                        } else {
                            (line.to_string(), line_number)
                        };

                    let func_block = PendingBlock {
                        lines: vec![merged_first_line],
                        start_line,
                        end_line: line_number,
                        boundary: BoundaryType::BraceCounting { brace_count },
                        entry_hint: Some(EntryType::Function),
                        name: Some(name),
                        value: None,
                        comment_count: 0,
                    };
                    active_block = Some(func_block);
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Check for multi-line parenthesis structure (e.g., plugins=(...))
            // ------------------------------------------------------------------
            let (open_paren, close_paren) = count_parens_outside_quotes(trimmed);
            if open_paren > close_paren {
                // Start multi-line parenthesis structure
                let parenthesis_count = (open_paren - close_paren) as i32;

                let (merged_first_line, start_line) = if let Some(pending) = pending_entry.take() {
                    // If pending has stored value, it's a trailing structured entry - flush it
                    if pending.is_structured_entry() {
                        result.add_entry(self.build_entry_from_pending(pending));
                        (line.to_string(), line_number)
                    } else if pending.comment_count == 1 {
                        // Single comment can merge down
                        let merged = format!("{}\n{}", pending.raw_content(), line);
                        (merged, pending.start_line)
                    } else {
                        // Multiple comments or pure blank - don't merge
                        result.add_entry(self.build_entry_from_pending(pending));
                        (line.to_string(), line_number)
                    }
                } else {
                    (line.to_string(), line_number)
                };

                let paren_block = PendingBlock {
                    lines: vec![merged_first_line],
                    start_line,
                    end_line: line_number,
                    boundary: BoundaryType::ParenthesisCounting { parenthesis_count },
                    entry_hint: Some(EntryType::Code),
                    name: None,
                    value: None,
                    comment_count: 0,
                };
                active_block = Some(paren_block);
                continue;
            }

            // ------------------------------------------------------------------
            // Fallback: capture as non-blank Code
            // ------------------------------------------------------------------
            match &mut pending_entry {
                Some(pending) if pending.entry_hint == Some(EntryType::Comment) => {
                    // Only single comment can merge down to code
                    if pending.comment_count == 1 {
                        // Comment + non-blank Code → merge and upgrade to Code
                        pending.add_line(line, line_number);
                        pending.upgrade_to_code();
                    } else {
                        // Multiple comments don't merge - flush and start new code
                        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(PendingBlock::code(line_number, line));
                    }
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

impl BashParser {}

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
        assert_eq!(result.entries[0].value, "alias ll='ls -la'");
    }

    #[test]
    fn test_parse_alias_double_quote() {
        let parser = BashParser::new();
        let content = r#"alias gs="git status""#;
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "gs");
        assert_eq!(result.entries[0].value, r#"alias gs="git status""#);
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
        // Value should contain complete syntax, not just quoted content
        assert_eq!(
            aliases[0].value,
            "alias complex='echo line1\necho line2\necho line3'"
        );
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
        // Value should contain complete syntax
        assert_eq!(exports[0].value, "export LONG='value1\nvalue2\nvalue3'");
    }

    #[test]
    fn test_parse_export() {
        let parser = BashParser::new();
        let content = "export EDITOR=nvim";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::EnvVar);
        assert_eq!(result.entries[0].name, "EDITOR");
        assert_eq!(result.entries[0].value, "export EDITOR=nvim");
    }

    #[test]
    fn test_parse_source() {
        let parser = BashParser::new();
        let content = "source ~/.aliases";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Source);
        assert_eq!(result.entries[0].name, ".aliases");
        assert_eq!(result.entries[0].value, "source ~/.aliases");
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
    fn test_adjacent_comments_merged_with_alias() {
        // NEW BEHAVIOR: Multiple comments don't merge with alias
        // Comments followed by alias should NOT merge (comment_count > 1)
        let parser = BashParser::new();
        let content = "# Header comment\n# Second line\n# Third line\nalias test='value'";
        let result = parser.parse(content);

        // Should have 2 entries: Comment block + Alias (not merged)
        assert_eq!(result.entries.len(), 2);

        let comment = &result.entries[0];
        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1)); // Starts from first comment
        assert_eq!(comment.end_line, Some(3)); // Ends at third comment line

        let alias = &result.entries[1];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.name, "test");
        assert_eq!(alias.line_number, Some(4)); // Starts on its own line
        assert!(!alias.value.contains("# Header comment")); // NOT merged
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
        // First comment merges with alias, second comment is standalone
        let parser = BashParser::new();
        let content = "# First block\nalias a='a'\n# Second block";
        let result = parser.parse(content);

        // Should have 2 entries: Alias (with merged comment) + Comment
        assert_eq!(result.entries.len(), 2);

        let alias = &result.entries[0];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.name, "a");
        assert!(alias.value.contains("# First block"));

        let comment = &result.entries[1];
        assert_eq!(comment.entry_type, EntryType::Comment);
        assert!(comment.value.contains("# Second block"));
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

        assert_eq!(blanks.len(), 0);
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
        // raw_line contains complete content (comment + code)
        assert_eq!(code.value, ("# Note\necho hello".to_string()));
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
        // raw_line contains complete content (comment + blank + code)
        assert_eq!(code.value, ("# Header\n\necho hi".to_string()));
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
    fn test_comment_then_alias_merged() {
        // # Section header
        // alias ll='ls -la'
        // → Merged into single Alias entry starting at L1
        let parser = BashParser::new();
        let content = "# Section header\nalias ll='ls -la'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);

        let alias = &result.entries[0];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.name, "ll");
        assert_eq!(alias.line_number, Some(1)); // Starts from comment line
        assert_eq!(alias.end_line, Some(2)); // Ends at alias line
                                             // Value should contain both comment and alias
        assert!(alias.value.contains("# Section header"));
        assert!(alias.value.contains("alias ll='ls -la'"));
    }

    #[test]
    fn test_comment_blank_alias_merged() {
        // NEW BEHAVIOR: Multiple comments don't merge down
        // # Section header
        // # with description
        // (blank)
        // alias ll='ls -la'
        // Expected: Comment block (L1-L3) + Alias (L4)
        let parser = BashParser::new();
        let content = "# Section header\n# with description\n\nalias ll='ls -la'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 2);

        let comment = &result.entries[0];
        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(3)); // Absorbs blank line

        let alias = &result.entries[1];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.line_number, Some(4)); // Starts on its own line
        assert!(!alias.value.contains("# Section header")); // NOT merged
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
        assert_eq!(code.value, "echo hi\n\n");
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
        assert_eq!(first.value, "echo first\n");
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
                                             // value should contain "# comment\n" (2 lines: comment + blank)
        assert_eq!(entry.value, "# comment\n");
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
                                             // value should contain all 3 lines
        assert_eq!(entry.value, "# comment\n\n");
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

    // === Tests for new merge logic ===

    #[test]
    fn test_alias_with_options() {
        let parser = BashParser::new();
        let content = "alias -g ll='ls -la'";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        let alias = &result.entries[0];
        assert_eq!(alias.entry_type, EntryType::Alias);
        assert_eq!(alias.name, "ll");
        assert_eq!(alias.value, "alias -g ll='ls -la'");
    }

    #[test]
    fn test_multiple_comments_dont_merge_down() {
        // Rule 2 & 4: Multiple comments form independent blocks
        let parser = BashParser::new();
        let content = "# comment 1\n# comment 2\n\nalias a='b'";
        let result = parser.parse(content);

        // Should have 2 entries: Comment block + Alias (not merged)
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].entry_type, EntryType::Comment);
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(3)); // Comment absorbs blank line
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].line_number, Some(4)); // Alias starts on line 4
        assert!(!result.entries[1].value.contains("comment")); // Not merged
    }

    #[test]
    fn test_blank_lines_merge_up_not_down() {
        // Rule 1: Blank lines only merge upward
        let parser = BashParser::new();
        let content = "\n\nalias a='b'\n\nalias c='d'";
        let result = parser.parse(content);

        // Should have 3 entries:
        // 1. Code (blank lines)
        // 2. Alias a (absorbs trailing blank)
        // 3. Alias c
        assert_eq!(result.entries.len(), 3);

        // First entry: leading blank lines
        assert_eq!(result.entries[0].entry_type, EntryType::Code);
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(2));

        // Second entry: alias a with trailing blank absorbed
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].line_number, Some(3));
        assert_eq!(result.entries[1].end_line, Some(4)); // Absorbs blank line 4

        // Third entry: alias c
        assert_eq!(result.entries[2].entry_type, EntryType::Alias);
        assert_eq!(result.entries[2].line_number, Some(5));
    }

    #[test]
    fn test_single_comment_merges_down() {
        // Rule 3: Single comment merges downward to structured entry
        let parser = BashParser::new();
        let content = "# single comment\nalias a='b'";
        let result = parser.parse(content);

        // Should have 1 entry: Alias with merged comment
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[0].line_number, Some(1)); // Starts from comment
        assert_eq!(result.entries[0].end_line, Some(2));
        assert!(result.entries[0].value.contains("# single comment"));
        assert!(result.entries[0].value.contains("alias a='b'"));
    }

    #[test]
    fn test_single_vs_multiple_comment_distinction() {
        // Rule 4: Distinguish between single and multiple comments
        let parser = BashParser::new();
        let content = "# comment 1\n# comment 2\n\nalias a='b'\n\n# single\n\nalias c='d'\n";
        let result = parser.parse(content);

        // Should have 3 entries:
        // 1. Multiple comments (not merged with alias a)
        // 2. Alias a (absorbs trailing blank)
        // 3. Alias c (merges with single comment, absorbs trailing blank)
        assert_eq!(result.entries.len(), 3);

        // Multiple comments block
        assert_eq!(result.entries[0].entry_type, EntryType::Comment);
        assert_eq!(result.entries[0].line_number, Some(1));

        // Alias a (not merged with comments above)
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].line_number, Some(4));
        assert!(!result.entries[1].value.contains("comment 1"));

        // Alias c (merged with single comment)
        assert_eq!(result.entries[2].entry_type, EntryType::Alias);
        assert_eq!(result.entries[2].line_number, Some(6)); // Starts from single comment
        assert!(result.entries[2].value.contains("# single"));
    }

    #[test]
    fn test_alias_with_trailing_blanks() {
        // Test that alias correctly absorbs trailing blank lines
        let parser = BashParser::new();
        let content = "alias foo='bar'\n\n\nexport BAR=baz\n";
        let result = parser.parse(content);

        // Alias should absorb L2-L3 trailing blanks
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[0].name, "foo");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(3));
        assert_eq!(result.entries[0].value, "alias foo='bar'\n\n");

        // Export starts at L4
        assert_eq!(result.entries[1].entry_type, EntryType::EnvVar);
        assert_eq!(result.entries[1].name, "BAR");
        assert_eq!(result.entries[1].line_number, Some(4));
        assert_eq!(result.entries[1].value, "export BAR=baz");
    }

    #[test]
    fn test_env_with_trailing_blanks() {
        // Test that export correctly absorbs trailing blank lines
        let parser = BashParser::new();
        let content = "export FOO=bar\n\n\nalias TEST='value'\n";
        let result = parser.parse(content);

        // Export should absorb L2-L3 trailing blanks
        assert_eq!(result.entries[0].entry_type, EntryType::EnvVar);
        assert_eq!(result.entries[0].name, "FOO");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(3));
        assert_eq!(result.entries[0].value, "export FOO=bar\n\n");

        // Alias starts at L4
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].line_number, Some(4));
    }

    #[test]
    fn test_source_with_trailing_blanks() {
        // Test that source correctly absorbs trailing blank lines
        let parser = BashParser::new();
        let content = "source ~/.profile\n\n\nalias TEST='value'\n";
        let result = parser.parse(content);

        // Source should absorb L2-L3 trailing blanks
        assert_eq!(result.entries[0].entry_type, EntryType::Source);
        assert_eq!(result.entries[0].name, ".profile");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(3));
        assert_eq!(result.entries[0].value, "source ~/.profile\n\n");

        // Alias starts at L4
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].line_number, Some(4));
    }

    #[test]
    fn test_multiline_alias_with_trailing_blanks() {
        // Test that multi-line alias correctly absorbs trailing blank lines
        let parser = BashParser::new();
        let content = "alias complex='line1\nline2\nline3'\n\n\nalias next='value'\n";
        let result = parser.parse(content);

        // Multi-line alias should absorb L4-L5 trailing blanks
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[0].name, "complex");
        assert_eq!(result.entries[0].line_number, Some(1));
        assert_eq!(result.entries[0].end_line, Some(5));
        assert_eq!(
            result.entries[0].value,
            "alias complex='line1\nline2\nline3'\n\n"
        );

        // Next alias starts at L6
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].line_number, Some(6));
    }
}
