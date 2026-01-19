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
//! The parser maintains these states:
//!
//! 1. **Normal** - Looking for new entries
//! 2. **InFunction** - Accumulating function body (brace counting)
//! 3. **InControlBlock** - Accumulating control structure (keyword tracking)
//! 4. **InMultilineAlias** - Accumulating multi-line alias (quote counting)
//! 5. **InMultilineEnv** - Accumulating multi-line env var (quote counting)
//! 6. **InCommentBlock** - Accumulating adjacent comments
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
    count_braces_outside_quotes, CodeBlockBuilder, CommentBlockBuilder, FunctionBuilder,
    QuotedValueBuilder,
};
use crate::parser::Parser;

use control::{count_control_end, count_control_start};
use parsers::{
    detect_function_start, try_parse_alias, try_parse_export, try_parse_source, AliasParseResult,
    ExportParseResult,
};

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
}

impl Default for BashParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for BashParser {
    fn parse(&self, content: &str) -> ParseResult {
        let mut result = ParseResult::new();

        // === State tracking variables ===

        // Function parsing state
        let mut in_function = false;
        let mut brace_count = 0;
        let mut current_func: Option<FunctionBuilder> = None;

        // Control structure state
        let mut control_depth: usize = 0;
        let mut current_code_block: Option<CodeBlockBuilder> = None;

        // Multi-line alias/env state
        let mut current_alias: Option<QuotedValueBuilder> = None;
        let mut current_env: Option<QuotedValueBuilder> = None;

        // Pending entry for Comment/Code merging (replaces comment_block + blank_line_start)
        let mut pending_entry: Option<Entry> = None;

        // Pending comment for association with next structured entry
        let mut pending_comment: Option<String> = None;

        // === Main parsing loop ===

        for (line_num, line) in content.lines().enumerate() {
            let line_number = line_num + 1;
            let trimmed = line.trim();

            // ------------------------------------------------------------------
            // Handle multi-line function body (highest priority)
            // ------------------------------------------------------------------
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

            // ------------------------------------------------------------------
            // Handle multi-line alias (quote counting)
            // ------------------------------------------------------------------
            if let Some(ref mut builder) = current_alias {
                builder.add_line(line);
                if builder.is_complete() {
                    let entry = current_alias.take().unwrap().build(EntryType::Alias);
                    result.add_entry(entry);
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Handle multi-line env var (quote counting)
            // ------------------------------------------------------------------
            if let Some(ref mut builder) = current_env {
                builder.add_line(line);
                if builder.is_complete() {
                    let entry = current_env.take().unwrap().build(EntryType::EnvVar);
                    result.add_entry(entry);
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Handle control structure blocks
            // ------------------------------------------------------------------
            let prev_depth = control_depth;
            control_depth = control_depth.saturating_sub(count_control_end(trimmed));
            control_depth += count_control_start(trimmed);

            if control_depth > 0 || (prev_depth > 0 && control_depth == 0) {
                // Start control block - merge pending Comment/Code if present
                if current_code_block.is_none() && prev_depth == 0 && control_depth > 0 {
                    if let Some(pending) = pending_entry.take() {
                        if matches!(pending.entry_type, EntryType::Comment | EntryType::Code) {
                            // Seed block with pending content (includes any absorbed blank lines)
                            let start_line = pending.line_number.unwrap_or(line_number);
                            let mut block = CodeBlockBuilder::new(start_line);
                            // Add lines from pending entry's raw_line
                            // Note: raw_line uses \n as separator (not terminator), so split directly
                            if let Some(ref raw) = pending.raw_line {
                                for l in raw.split('\n') {
                                    block.add_line(l);
                                }
                            } else {
                                block.add_line(&pending.value);
                            }
                            block.add_line(line);
                            current_code_block = Some(block);
                        } else {
                            // Not mergeable, flush normally and start new block
                            result.add_entry(pending);
                            current_code_block = Some(CodeBlockBuilder::new(line_number));
                            if let Some(ref mut block) = current_code_block {
                                block.add_line(line);
                            }
                        }
                    } else {
                        // No pending entry, start fresh block
                        current_code_block = Some(CodeBlockBuilder::new(line_number));
                        if let Some(ref mut block) = current_code_block {
                            block.add_line(line);
                        }
                    }
                } else if let Some(ref mut block) = current_code_block {
                    // Continue existing control block
                    block.add_line(line);
                }

                // Close control block - make result pending for trailing blank absorption
                if prev_depth > 0 && control_depth == 0 {
                    if let Some(block) = current_code_block.take() {
                        pending_entry = Some(block.build());
                    }
                }

                pending_comment = None;
                continue;
            }

            // ------------------------------------------------------------------
            // Handle empty lines (part of pending entry state machine)
            // ------------------------------------------------------------------
            if trimmed.is_empty() {
                let blank_entry =
                    Entry::new(EntryType::Code, format!("L{}", line_number), String::new())
                        .with_line_number(line_number)
                        .with_end_line(line_number) // Single blank line has same end_line
                        .with_raw_line(line.to_string());

                match &mut pending_entry {
                    Some(pending) if pending.entry_type == EntryType::Comment => {
                        // Comment absorbs blank line
                        pending.merge_trailing(blank_entry);
                    }
                    Some(pending) if pending.is_blank() => {
                        // Blank Code absorbs blank line
                        pending.merge_trailing(blank_entry);
                    }
                    Some(pending) if pending.entry_type == EntryType::Code => {
                        // Non-blank Code absorbs trailing blank line
                        pending.merge_trailing(blank_entry);
                    }
                    Some(_) => {
                        // Other pending types: flush and start new blank
                        if let Some(entry) = pending_entry.take() {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(blank_entry);
                    }
                    None => {
                        // Start new blank entry
                        pending_entry = Some(blank_entry);
                    }
                }
                pending_comment = None;
                continue;
            }

            // ------------------------------------------------------------------
            // Handle comment lines (part of pending entry state machine)
            // ------------------------------------------------------------------
            if CommentBlockBuilder::is_standalone_comment(trimmed) {
                // value keeps full raw line (including leading whitespace and # prefix)
                let comment_entry = Entry::new(
                    EntryType::Comment,
                    format!("#L{}", line_number),
                    line.to_string(), // Keep full raw line for value
                )
                .with_line_number(line_number)
                .with_end_line(line_number) // Single-line comment has same end_line
                .with_raw_line(line.to_string());

                // Extract comment text for pending_comment (for association with next entry)
                let comment_text = trimmed.strip_prefix('#').unwrap_or("").trim();

                match &mut pending_entry {
                    Some(pending) if pending.entry_type == EntryType::Comment => {
                        // Comment merges with Comment
                        pending.merge_trailing(comment_entry);
                        // Update pending_comment to last comment text
                        pending_comment = Some(comment_text.to_string());
                    }
                    Some(_) => {
                        // Non-Comment pending: flush and start new comment
                        if let Some(entry) = pending_entry.take() {
                            result.add_entry(entry);
                        }
                        pending_entry = Some(comment_entry);
                        pending_comment = Some(comment_text.to_string());
                    }
                    None => {
                        // Start new comment entry
                        pending_entry = Some(comment_entry);
                        pending_comment = Some(comment_text.to_string());
                    }
                }
                continue;
            }

            // ------------------------------------------------------------------
            // Try to parse structured entry types (Alias, EnvVar, Source, Function)
            // ------------------------------------------------------------------

            // Try alias
            match try_parse_alias(trimmed, line_number) {
                AliasParseResult::SingleLine(mut entry) => {
                    // Flush pending entry
                    if let Some(pending) = pending_entry.take() {
                        result.add_entry(pending);
                    }
                    if let Some(comment) = pending_comment.take() {
                        entry = entry.with_comment(comment);
                    }
                    result.add_entry(entry);
                    continue;
                }
                AliasParseResult::MultiLineStart { builder } => {
                    // Flush pending entry
                    if let Some(pending) = pending_entry.take() {
                        result.add_entry(pending);
                    }
                    current_alias = Some(builder);
                    continue;
                }
                AliasParseResult::NotAlias => {}
            }

            // Try export
            match try_parse_export(trimmed, line_number) {
                ExportParseResult::SingleLine(mut entry) => {
                    // Flush pending entry
                    if let Some(pending) = pending_entry.take() {
                        result.add_entry(pending);
                    }
                    if let Some(comment) = pending_comment.take() {
                        entry = entry.with_comment(comment);
                    }
                    result.add_entry(entry);
                    continue;
                }
                ExportParseResult::MultiLineStart { builder } => {
                    // Flush pending entry
                    if let Some(pending) = pending_entry.take() {
                        result.add_entry(pending);
                    }
                    current_env = Some(builder);
                    continue;
                }
                ExportParseResult::NotExport => {}
            }

            // Try source
            if let Some(mut entry) = try_parse_source(trimmed, line_number) {
                // Flush pending entry
                if let Some(pending) = pending_entry.take() {
                    result.add_entry(pending);
                }
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
                continue;
            }

            // Try function
            if let Some(func_name) = detect_function_start(trimmed) {
                // Flush pending entry
                if let Some(pending) = pending_entry.take() {
                    result.add_entry(pending);
                }

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

            // ------------------------------------------------------------------
            // Fallback: capture as non-blank Code
            // ------------------------------------------------------------------
            let code_entry = Entry::new(
                EntryType::Code,
                format!("L{}", line_number),
                line.to_string(), // Keep full raw line for value
            )
            .with_line_number(line_number)
            .with_end_line(line_number) // Single line Code has same end_line
            .with_raw_line(line.to_string());

            match &mut pending_entry {
                Some(pending) if pending.entry_type == EntryType::Comment => {
                    // Comment + non-blank Code → merge and upgrade to Code
                    // Keep in pending to allow absorbing trailing blank lines
                    pending.merge_trailing(code_entry);
                    // Don't take() here - let it stay pending so it can absorb blanks
                    // via the Code branch below on subsequent iterations
                }
                Some(pending) if pending.entry_type == EntryType::Code => {
                    // Non-blank Code pending + new non-blank Code → flush pending, new pending
                    if let Some(entry) = pending_entry.take() {
                        result.add_entry(entry);
                    }
                    pending_entry = Some(code_entry);
                }
                Some(_) => {
                    // Flush pending, start new pending Code
                    if let Some(entry) = pending_entry.take() {
                        result.add_entry(entry);
                    }
                    pending_entry = Some(code_entry);
                }
                None => {
                    // No pending, start new pending Code
                    pending_entry = Some(code_entry);
                }
            }
            pending_comment = None;
        }

        // === Flush remaining state ===

        // Flush remaining pending entry
        if let Some(entry) = pending_entry.take() {
            result.add_entry(entry);
        }

        // Warn about unclosed function
        if in_function {
            result.add_warning(crate::model::ParseWarning::new(
                current_func.as_ref().map(|f| f.start_line).unwrap_or(0),
                "Unclosed function definition at end of file",
                "",
            ));
        }

        // Warn about unclosed multi-line alias
        if let Some(builder) = current_alias {
            result.add_warning(crate::model::ParseWarning::new(
                builder.start_line,
                "Unclosed multi-line alias at end of file",
                "",
            ));
        }

        // Warn about unclosed multi-line env
        if let Some(builder) = current_env {
            result.add_warning(crate::model::ParseWarning::new(
                builder.start_line,
                "Unclosed multi-line export at end of file",
                "",
            ));
        }

        result
    }

    fn shell_type(&self) -> ShellType {
        ShellType::Bash
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
        assert_eq!(result.entries[0].name, "L1");
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

    #[test]
    fn test_comment_association() {
        let parser = BashParser::new();
        let content = "# List files\nalias ll='ls -la'";
        let result = parser.parse(content);

        let alias = result
            .entries
            .iter()
            .find(|e| e.entry_type == EntryType::Alias)
            .unwrap();

        assert_eq!(alias.comment, Some("List files".to_string()));
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
        // → Comment L1 flushed, Alias L2 with comment association
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
        assert_eq!(alias.comment, Some("Section header".to_string()));
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
        // No comment association because blank line broke the adjacency for pending_comment
        assert_eq!(alias.comment, None);
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
}
