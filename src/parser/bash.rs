//! Bash configuration file parser

use lazy_static::lazy_static;
use regex::Regex;

use super::{
    common::{count_braces_outside_quotes, extract_comment, CodeBlockBuilder, FunctionBuilder},
    Parser,
};
use crate::model::{Entry, EntryType, ParseResult, ShellType};

lazy_static! {
    // alias ll='ls -la' (single quotes)
    // Alias names can include special chars like '.', '~', '-', etc.
    static ref ALIAS_SINGLE_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)='([^']*)'(?:\s*(?:#.*)?)?$"#
    ).unwrap();

    // alias ll="ls -la" (double quotes)
    static ref ALIAS_DOUBLE_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)="([^"]*)"(?:\s*(?:#.*)?)?$"#
    ).unwrap();

    // alias ll=ls (without quotes)
    static ref ALIAS_NOQUOTE_RE: Regex = Regex::new(
        r#"^alias\s+([^\s=]+)=(\S+)(?:\s*(?:#.*)?)?$"#
    ).unwrap();

    // export VAR=value or export VAR="value"
    static ref EXPORT_RE: Regex = Regex::new(
        r#"^export\s+(\w+)=(.*)$"#
    ).unwrap();

    // source file or . file
    static ref SOURCE_RE: Regex = Regex::new(
        r#"^(?:source|\.)\s+(.+)$"#
    ).unwrap();

    // function name() { or function name { or name() {
    static ref FUNC_START_RE: Regex = Regex::new(
        r#"^(?:function\s+)?(\w+)\s*\(\s*\)\s*\{?"#
    ).unwrap();

    // function name { (without parentheses)
    static ref FUNC_KEYWORD_RE: Regex = Regex::new(
        r#"^function\s+(\w+)\s*\{"#
    ).unwrap();
}

/// Bash configuration file parser
pub struct BashParser;

impl BashParser {
    pub fn new() -> Self {
        Self
    }

    fn parse_alias(&self, line: &str, line_num: usize) -> Option<Entry> {
        // Try single quote format first
        if let Some(caps) = ALIAS_SINGLE_RE.captures(line) {
            return Some(
                Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }

        // Try double quote format
        if let Some(caps) = ALIAS_DOUBLE_RE.captures(line) {
            return Some(
                Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }

        // Try unquoted format
        if let Some(caps) = ALIAS_NOQUOTE_RE.captures(line) {
            return Some(
                Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }

        None
    }

    fn parse_export(&self, line: &str, line_num: usize) -> Option<Entry> {
        if let Some(caps) = EXPORT_RE.captures(line) {
            let (value_clean, _inline_comment) = extract_comment(&caps[2], '#');
            let value = super::common::strip_quotes(&value_clean);
            return Some(
                Entry::new(EntryType::EnvVar, caps[1].to_string(), value)
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }
        None
    }

    fn parse_source(&self, line: &str, line_num: usize) -> Option<Entry> {
        if let Some(caps) = SOURCE_RE.captures(line) {
            let (path_clean, _inline_comment) = extract_comment(&caps[1], '#');
            let path = super::common::strip_quotes(&path_clean);
            // Use line number as name (like Code entries) for consistent identification
            let name = format!("L{}", line_num);
            return Some(
                Entry::new(EntryType::Source, name, path)
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }
        None
    }

    fn detect_function_start(&self, line: &str) -> Option<String> {
        if let Some(caps) = FUNC_START_RE.captures(line) {
            return Some(caps[1].to_string());
        }
        if let Some(caps) = FUNC_KEYWORD_RE.captures(line) {
            return Some(caps[1].to_string());
        }
        None
    }

    /// Count the number of control structure openings in a line
    /// Matches: if, while, until, for, case, select
    fn count_control_start(line: &str) -> usize {
        let mut count = 0;
        // Split by semicolon and check each part
        for part in line.split(';') {
            let part = part.trim();
            // Check for various control structure keywords
            let starts_control = part.starts_with("if ")
                || part == "if"
                || part.starts_with("while ")
                || part == "while"
                || part.starts_with("until ")
                || part == "until"
                || part.starts_with("for ")
                || part.starts_with("case ")
                || part.starts_with("select ");

            if starts_control {
                count += 1;
            }
        }
        count
    }

    /// Count the number of control structure closings in a line
    /// Matches: fi, done, esac
    fn count_control_end(line: &str) -> usize {
        let mut count = 0;
        // Split by semicolon and check each part
        for part in line.split(';') {
            let part = part.trim();
            let ends_control = part == "fi"
                || part.starts_with("fi ")
                || part.starts_with("fi;")
                || part == "done"
                || part.starts_with("done ")
                || part.starts_with("done;")
                || part == "esac"
                || part.starts_with("esac ")
                || part.starts_with("esac;");

            if ends_control {
                count += 1;
            }
        }
        // Also check the original line for these keywords at word boundaries
        // This handles cases like "fi" at end of line
        let words: Vec<&str> = line.split_whitespace().collect();
        for word in words {
            let word = word.trim_end_matches(';');
            if count == 0 && (word == "fi" || word == "done" || word == "esac") {
                count += 1;
            }
        }
        count.min(1) // Avoid double counting - max 1 per line for simple cases
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
        let mut in_function = false;
        let mut brace_count = 0;
        let mut current_func: Option<FunctionBuilder> = None;
        let mut pending_comment: Option<String> = None;
        // Track control structure depth (if/fi, while/done, for/done, case/esac, etc.)
        let mut control_depth: usize = 0;
        // Track control structure block
        let mut current_code_block: Option<CodeBlockBuilder> = None;
        // Track consecutive blank lines
        let mut blank_line_start: Option<usize> = None;

        for (line_num, line) in content.lines().enumerate() {
            let line_number = line_num + 1;
            let trimmed = line.trim();

            // Handle multi-line function body first
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

            // Track control structure depth changes
            let prev_depth = control_depth;
            control_depth = control_depth.saturating_sub(Self::count_control_end(trimmed));
            control_depth += Self::count_control_start(trimmed);

            // Handle control structure blocks
            if control_depth > 0 || (prev_depth > 0 && control_depth == 0) {
                // Flush any pending blank lines before starting control block
                if let Some(start) = blank_line_start.take() {
                    let end = line_number - 1;
                    result.add_entry(create_blank_line_entry(start, end));
                }

                // We're inside a control structure, or just closed one
                if current_code_block.is_none() && prev_depth == 0 && control_depth > 0 {
                    // Starting a new control block
                    current_code_block = Some(CodeBlockBuilder::new(line_number));
                }

                if let Some(ref mut block) = current_code_block {
                    block.add_line(line);
                }

                // Check if we just closed the outermost control structure
                if prev_depth > 0 && control_depth == 0 {
                    if let Some(block) = current_code_block.take() {
                        result.add_entry(block.build());
                    }
                }

                pending_comment = None;
                continue;
            }

            // Handle empty lines - group consecutive ones
            if trimmed.is_empty() {
                if blank_line_start.is_none() {
                    blank_line_start = Some(line_number);
                }
                pending_comment = None;
                continue;
            } else {
                // Non-empty line found, flush any pending blank lines
                if let Some(start) = blank_line_start.take() {
                    let end = line_number - 1;
                    result.add_entry(create_blank_line_entry(start, end));
                }
            }

            // Handle pure comment lines
            if let Some(stripped) = trimmed.strip_prefix('#') {
                // Create a comment entry
                let comment_text = stripped.trim().to_string();
                let entry = Entry::new(
                    EntryType::Comment,
                    format!("L{}", line_number),
                    comment_text.clone(),
                )
                .with_line_number(line_number)
                .with_raw_line(line.to_string());
                result.add_entry(entry);
                // Also set pending comment for potential association with next entry
                pending_comment = Some(comment_text);
                continue;
            }

            // Try to parse different entry types
            if let Some(mut entry) = self.parse_alias(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
            } else if let Some(mut entry) = self.parse_export(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
            } else if let Some(mut entry) = self.parse_source(trimmed, line_number) {
                if let Some(comment) = pending_comment.take() {
                    entry = entry.with_comment(comment);
                }
                result.add_entry(entry);
            } else if let Some(func_name) = self.detect_function_start(trimmed) {
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
            } else {
                // Could not parse this line as a known type - capture as Code
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
        }

        // Flush any remaining blank lines at end of file
        if let Some(start) = blank_line_start.take() {
            let end = content.lines().count();
            result.add_entry(create_blank_line_entry(start, end));
        }

        // Check for unclosed function at end of file
        if in_function {
            result.add_warning(crate::model::ParseWarning::new(
                current_func.as_ref().map(|f| f.start_line).unwrap_or(0),
                "Unclosed function definition at end of file",
                "",
            ));
        }

        result
    }

    fn shell_type(&self) -> ShellType {
        ShellType::Bash
    }
}

/// Helper function to create blank line entries (single or range)
fn create_blank_line_entry(start: usize, end: usize) -> Entry {
    let name = if start == end {
        format!("L{}", start)
    } else {
        format!("L{}-L{}", start, end)
    };

    Entry::new(EntryType::Code, name, String::new())
        .with_line_number(start)
        .with_end_line(end)
        .with_raw_line(String::new())
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
    fn test_parse_export_with_quotes() {
        let parser = BashParser::new();
        let content = r#"export PATH="$HOME/bin:$PATH""#;
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, "PATH");
        assert_eq!(result.entries[0].value, "$HOME/bin:$PATH");
    }

    #[test]
    fn test_parse_source() {
        let parser = BashParser::new();
        let content = "source ~/.aliases";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Source);
        assert_eq!(result.entries[0].name, "L1"); // Now uses line number as name
        assert_eq!(result.entries[0].value, "~/.aliases"); // Path is in value
    }

    #[test]
    fn test_parse_source_dot() {
        let parser = BashParser::new();
        let content = ". ~/.profile";
        let result = parser.parse(content);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].entry_type, EntryType::Source);
    }

    #[test]
    fn test_parse_function() {
        let parser = BashParser::new();
        let content = r#"
function greet() {
    echo "Hello, $1"
}"#;
        let result = parser.parse(content);

        // Filter to get only function entries
        let funcs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Function)
            .collect();

        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "greet");
    }

    #[test]
    fn test_parse_mixed_content() {
        let parser = BashParser::new();
        let content = r#"
# Git aliases
alias gs='git status'
alias gd='git diff'

# Editor
export EDITOR=nvim

# Load custom aliases
source ~/.custom_aliases

# Greeting function
greet() {
    echo "Hello"
}"#;
        let result = parser.parse(content);

        // Filter to get only the "main" entries (alias, env, source, function)
        let main_entries: Vec<_> = result
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e.entry_type,
                    EntryType::Alias | EntryType::EnvVar | EntryType::Source | EntryType::Function
                )
            })
            .collect();

        assert_eq!(main_entries.len(), 5);
    }

    #[test]
    fn test_comment_association() {
        let parser = BashParser::new();
        let content = r#"
# List files in long format
alias ll='ls -la'"#;
        let result = parser.parse(content);

        // Find the alias entry
        let alias = result
            .entries
            .iter()
            .find(|e| e.entry_type == EntryType::Alias)
            .expect("Should have an alias entry");

        assert_eq!(alias.comment, Some("List files in long format".to_string()));
    }

    #[test]
    fn test_parse_special_alias_names() {
        let parser = BashParser::new();
        let content = "alias ..='cd ..'\nalias ~='cd ~'\nalias ...='cd ../..'";
        let result = parser.parse(content);

        // Filter to aliases only
        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();

        assert_eq!(aliases.len(), 3);
        assert_eq!(aliases[0].name, "..");
        assert_eq!(aliases[0].value, "cd ..");

        assert_eq!(aliases[1].name, "~");
        assert_eq!(aliases[1].value, "cd ~");

        assert_eq!(aliases[2].name, "...");
        assert_eq!(aliases[2].value, "cd ../..");
    }

    #[test]
    fn test_control_structure_captured_as_code() {
        let parser = BashParser::new();
        let content = r#"
# Top-level source
source ~/.aliases

if [ -f /etc/bash_completion ]; then
    . /etc/bash_completion
fi

# Another top-level alias
alias ll='ls -la'"#;
        let result = parser.parse(content);

        // Should have source, alias, and the if block as Code
        let sources: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Source)
            .collect();
        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();
        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("if ["))
            .collect();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "L3"); // Source now uses line number as name
        assert_eq!(sources[0].value, "~/.aliases"); // Path is in value
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].name, "ll");
        assert_eq!(code_blocks.len(), 1);
        // Verify the if block is captured with its content
        assert!(code_blocks[0].value.contains(". /etc/bash_completion"));
    }

    #[test]
    fn test_nested_control_structures_captured_as_single_block() {
        let parser = BashParser::new();
        let content = r#"
alias top='top-level'

if [ -n "$VAR" ]; then
    export INSIDE_IF=value
    if [ -f "$FILE" ]; then
        source $FILE
    fi
fi

alias bottom='bottom-level'"#;
        let result = parser.parse(content);

        // Should have the nested if blocks as a single Code entry
        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();
        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("if ["))
            .collect();

        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name, "top");
        assert_eq!(aliases[1].name, "bottom");
        // The entire nested if block should be one Code entry
        assert_eq!(code_blocks.len(), 1);
        assert!(code_blocks[0].value.contains("INSIDE_IF"));
        assert!(code_blocks[0].value.contains("source $FILE"));
    }

    #[test]
    fn test_for_loop_captured_as_code() {
        let parser = BashParser::new();
        let content = r#"
alias before='before-loop'

for dir in ~/bin ~/.local/bin; do
    export PATH="$dir:$PATH"
done

alias after='after-loop'"#;
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();
        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("for dir"))
            .collect();

        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name, "before");
        assert_eq!(aliases[1].name, "after");
        assert_eq!(code_blocks.len(), 1);
    }

    #[test]
    fn test_while_loop_captured_as_code() {
        let parser = BashParser::new();
        let content = r#"
alias start='start'

while read line; do
    export LINE="$line"
done < file.txt

alias end='end'"#;
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();

        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name, "start");
        assert_eq!(aliases[1].name, "end");
    }

    #[test]
    fn test_case_statement_captured_as_code() {
        let parser = BashParser::new();
        let content = r#"
alias before='before'

case "$TERM" in
    xterm*)
        export TERM_TYPE=xterm
        ;;
    *)
        export TERM_TYPE=other
        ;;
esac

alias after='after'"#;
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();
        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("case"))
            .collect();

        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name, "before");
        assert_eq!(aliases[1].name, "after");
        assert_eq!(code_blocks.len(), 1);
        assert!(code_blocks[0].value.contains("esac"));
    }

    #[test]
    fn test_empty_lines_captured() {
        let parser = BashParser::new();
        let content = "alias a='a'\n\nalias b='b'";
        let result = parser.parse(content);

        // Should have: alias, empty line (grouped as single entry), alias
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].entry_type, EntryType::Code);
        assert_eq!(result.entries[1].value, "");
        assert_eq!(result.entries[1].name, "L2"); // Single blank line
        assert_eq!(result.entries[2].entry_type, EntryType::Alias);
    }

    #[test]
    fn test_multiple_blank_lines_grouped() {
        let parser = BashParser::new();
        let content = "alias a='a'\n\n\n\nalias b='b'";
        let result = parser.parse(content);

        // Should have: alias, blank lines (grouped), alias
        let blank_entries: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.is_empty())
            .collect();

        assert_eq!(blank_entries.len(), 1);
        assert_eq!(blank_entries[0].name, "L2-L4"); // Three blank lines grouped
        assert_eq!(blank_entries[0].line_number, Some(2));
        assert_eq!(blank_entries[0].end_line, Some(4));
    }

    #[test]
    fn test_comments_captured_as_entries() {
        let parser = BashParser::new();
        let content = "# This is a comment\nalias a='a'";
        let result = parser.parse(content);

        // Should have: comment, alias
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].entry_type, EntryType::Comment);
        assert_eq!(result.entries[0].value, "This is a comment");
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
    }

    #[test]
    fn test_code_block_line_range() {
        let parser = BashParser::new();
        let content = "if true; then\n    echo hi\nfi";
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
}
