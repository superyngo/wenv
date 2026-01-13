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
    count_braces_outside_quotes, create_blank_line_entry, CodeBlockBuilder, CommentBlockBuilder,
    FunctionBuilder, QuotedValueBuilder,
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

        // Comment block state
        let mut current_comment_block: Option<CommentBlockBuilder> = None;

        // Blank line tracking
        let mut blank_line_start: Option<usize> = None;

        // Pending comment for association with next entry
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
                // Flush any pending items before control block
                flush_pending_items(
                    &mut result,
                    &mut current_comment_block,
                    &mut blank_line_start,
                    line_number,
                );

                // Start or continue control block
                if current_code_block.is_none() && prev_depth == 0 && control_depth > 0 {
                    current_code_block = Some(CodeBlockBuilder::new(line_number));
                }

                if let Some(ref mut block) = current_code_block {
                    block.add_line(line);
                }

                // Close control block
                if prev_depth > 0 && control_depth == 0 {
                    if let Some(block) = current_code_block.take() {
                        result.add_entry(block.build());
                    }
                }

                pending_comment = None;
                continue;
            }

            // ------------------------------------------------------------------
            // Handle empty lines
            // ------------------------------------------------------------------
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

            // ------------------------------------------------------------------
            // Handle comment lines (adjacent merging)
            // ------------------------------------------------------------------
            if CommentBlockBuilder::is_standalone_comment(trimmed) {
                if let Some(ref mut block) = current_comment_block {
                    // Continue existing comment block
                    block.add_line(line);
                } else {
                    // Start new comment block
                    current_comment_block = Some(CommentBlockBuilder::new(line_number, line));
                }

                // Extract comment text for potential association
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

            // ------------------------------------------------------------------
            // Try to parse entry types
            // ------------------------------------------------------------------

            // Try alias
            match try_parse_alias(trimmed, line_number) {
                AliasParseResult::SingleLine(mut entry) => {
                    if let Some(comment) = pending_comment.take() {
                        entry = entry.with_comment(comment);
                    }
                    result.add_entry(entry);
                    continue;
                }
                AliasParseResult::MultiLineStart { builder } => {
                    current_alias = Some(builder);
                    continue;
                }
                AliasParseResult::NotAlias => {}
            }

            // Try export
            match try_parse_export(trimmed, line_number) {
                ExportParseResult::SingleLine(mut entry) => {
                    if let Some(comment) = pending_comment.take() {
                        entry = entry.with_comment(comment);
                    }
                    result.add_entry(entry);
                    continue;
                }
                ExportParseResult::MultiLineStart { builder } => {
                    current_env = Some(builder);
                    continue;
                }
                ExportParseResult::NotExport => {}
            }

            // Try source
            if let Some(mut entry) = try_parse_source(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
                continue;
            }

            // Try function
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

        // === Flush remaining state ===

        // Flush remaining comment block
        if let Some(block) = current_comment_block.take() {
            result.add_entry(block.build());
        }

        // Flush remaining blank lines
        if let Some(start) = blank_line_start.take() {
            let end = content.lines().count();
            result.add_entry(create_blank_line_entry(start, end));
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
        let parser = BashParser::new();
        let content = "# First block\n\n# Second block";
        let result = parser.parse(content);

        let comments: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Comment)
            .collect();

        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].name, "#L1");
        assert_eq!(comments[1].name, "#L3");
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
}
