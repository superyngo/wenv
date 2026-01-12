//! PowerShell configuration file parser

use lazy_static::lazy_static;
use regex::Regex;

use super::{
    common::{count_braces_outside_quotes, extract_comment, CodeBlockBuilder, FunctionBuilder},
    Parser,
};
use crate::model::{Entry, EntryType, ParseResult, ShellType};

lazy_static! {
    // Set-Alias ll Get-ChildItem or New-Alias ...
    static ref ALIAS_RE: Regex = Regex::new(
        r#"^(?:Set-Alias|New-Alias)\s+(-Name\s+)?(\w+)\s+(-Value\s+)?(.+)$"#
    ).unwrap();

    // Simple Set-Alias format: Set-Alias name value
    static ref ALIAS_SIMPLE_RE: Regex = Regex::new(
        r#"^(?:Set-Alias|New-Alias)\s+(\w+)\s+(\w[\w-]*)$"#
    ).unwrap();

    // $env:VAR = "value"
    static ref ENV_RE: Regex = Regex::new(
        r#"^\$env:(\w+)\s*=\s*(.+)$"#
    ).unwrap();

    // . .\file.ps1 or . C:\path\file.ps1
    static ref SOURCE_RE: Regex = Regex::new(
        r#"^\.\s+(.+\.ps1)$"#
    ).unwrap();

    // function Name { or function Name() {
    static ref FUNC_START_RE: Regex = Regex::new(
        r#"^function\s+(\w[\w-]*)\s*(?:\([^)]*\))?\s*\{"#
    ).unwrap();
}

/// PowerShell configuration file parser
pub struct PowerShellParser;

impl PowerShellParser {
    pub fn new() -> Self {
        Self
    }

    fn parse_alias(&self, line: &str, line_num: usize) -> Option<Entry> {
        // Try simple format first
        if let Some(caps) = ALIAS_SIMPLE_RE.captures(line) {
            return Some(
                Entry::new(EntryType::Alias, caps[1].to_string(), caps[2].to_string())
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }

        // Try complex format with -Name and -Value
        if let Some(caps) = ALIAS_RE.captures(line) {
            let name = caps[2].to_string();
            let value = super::common::strip_quotes(&caps[4]);
            return Some(
                Entry::new(EntryType::Alias, name, value)
                    .with_line_number(line_num)
                    .with_raw_line(line.to_string()),
            );
        }

        None
    }

    fn parse_env(&self, line: &str, line_num: usize) -> Option<Entry> {
        if let Some(caps) = ENV_RE.captures(line) {
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
        None
    }

    /// Count the number of control structure openings in a line
    /// Matches: if, foreach, while, for, switch, try
    fn count_control_start(line: &str) -> usize {
        let mut count = 0;
        let lower = line.to_lowercase();

        // Check for control structure keywords at start of statement
        for part in lower.split(';') {
            let part = part.trim();
            let starts_control = part.starts_with("if ")
                || part.starts_with("if(")
                || part == "if"
                || part.starts_with("foreach ")
                || part.starts_with("foreach(")
                || part.starts_with("while ")
                || part.starts_with("while(")
                || part.starts_with("for ")
                || part.starts_with("for(")
                || part.starts_with("switch ")
                || part.starts_with("switch(")
                || part.starts_with("try ")
                || part == "try"
                || part.starts_with("try{");

            if starts_control {
                count += 1;
            }
        }
        count
    }

    /// Count the number of control structure closings in a line
    /// In PowerShell, control structures end with closing brace, optionally followed by else/elseif/catch/finally
    fn count_control_end(line: &str) -> usize {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        // A lone closing brace or closing brace at end of line ends a control structure
        // But we need to be careful about else/elseif/catch/finally which continue the structure
        if trimmed == "}" {
            return 1;
        }

        // Check for end patterns that don't continue (no else/catch/finally)
        // If line ends with } and doesn't have continuation keywords
        if trimmed.ends_with('}') {
            // Check if this is NOT a continuation (else, elseif, catch, finally)
            if !lower.contains("else") && !lower.contains("catch") && !lower.contains("finally") {
                // Count closing braces that aren't part of continuation
                let close_count = trimmed.matches('}').count();
                let open_count = trimmed.matches('{').count();
                if close_count > open_count {
                    return close_count - open_count;
                }
            }
        }

        0
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
        let mut in_function = false;
        let mut brace_count = 0;
        let mut current_func: Option<FunctionBuilder> = None;
        let mut pending_comment: Option<String> = None;
        // Track control structure depth (if/foreach/while/for/switch/try)
        let mut control_depth: usize = 0;
        // Track control structure block
        let mut current_code_block: Option<CodeBlockBuilder> = None;

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

            // Handle empty lines - create Code entry to preserve structure
            if trimmed.is_empty() {
                let entry = Entry::new(EntryType::Code, format!("L{}", line_number), String::new())
                    .with_line_number(line_number)
                    .with_raw_line(String::new());
                result.add_entry(entry);
                pending_comment = None;
                continue;
            }

            // Handle pure comment lines (PowerShell uses #)
            if let Some(stripped) = trimmed.strip_prefix('#') {
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
            } else if let Some(mut entry) = self.parse_env(trimmed, line_number) {
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
        ShellType::PowerShell
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
    fn test_parse_source() {
        let parser = PowerShellParser::new();
        let content = r#". .\aliases.ps1"#;
        let result = parser.parse(content);

        let sources: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Source)
            .collect();

        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn test_parse_function() {
        let parser = PowerShellParser::new();
        let content = r#"
function Get-Greeting {
    param($Name)
    Write-Host "Hello, $Name"
}
"#;
        let result = parser.parse(content);

        let funcs: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Function)
            .collect();

        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "Get-Greeting");
    }

    #[test]
    fn test_empty_lines_captured() {
        let parser = PowerShellParser::new();
        let content = "Set-Alias a Get-ChildItem\n\nSet-Alias b Get-Location";
        let result = parser.parse(content);

        // Should have: alias, empty line, alias
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].entry_type, EntryType::Code);
        assert_eq!(result.entries[1].value, "");
        assert_eq!(result.entries[2].entry_type, EntryType::Alias);
    }

    #[test]
    fn test_comments_captured_as_entries() {
        let parser = PowerShellParser::new();
        let content = "# This is a comment\nSet-Alias a Get-ChildItem";
        let result = parser.parse(content);

        // Should have: comment, alias
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].entry_type, EntryType::Comment);
        assert_eq!(result.entries[0].value, "This is a comment");
        assert_eq!(result.entries[1].entry_type, EntryType::Alias);
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
            .expect("Should have an alias entry");

        assert_eq!(alias.comment, Some("List files".to_string()));
    }

    #[test]
    fn test_unparseable_lines_captured_as_code() {
        let parser = PowerShellParser::new();
        let content =
            "Set-Alias a Get-ChildItem\nWrite-Host 'Hello World'\nSet-Alias b Get-Location";
        let result = parser.parse(content);

        // Should have: alias, code (Write-Host), alias
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.entries[0].entry_type, EntryType::Alias);
        assert_eq!(result.entries[1].entry_type, EntryType::Code);
        assert!(result.entries[1].value.contains("Write-Host"));
        assert_eq!(result.entries[2].entry_type, EntryType::Alias);
    }

    #[test]
    fn test_if_statement_captured_as_code() {
        let parser = PowerShellParser::new();
        let content = r#"
Set-Alias before Get-ChildItem

if ($true) {
    $env:TEST = "value"
}

Set-Alias after Get-Location"#;
        let result = parser.parse(content);

        let aliases: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Alias)
            .collect();
        let code_blocks: Vec<_> = result
            .entries
            .iter()
            .filter(|e| e.entry_type == EntryType::Code && e.value.contains("if"))
            .collect();

        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0].name, "before");
        assert_eq!(aliases[1].name, "after");
        assert_eq!(code_blocks.len(), 1);
        assert!(code_blocks[0].value.contains("$env:TEST"));
    }

    #[test]
    fn test_foreach_captured_as_code() {
        let parser = PowerShellParser::new();
        let content = r#"
Set-Alias start Get-ChildItem

foreach ($item in @(1,2,3)) {
    Write-Host $item
}

Set-Alias end Get-Location"#;
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
    fn test_mixed_content() {
        let parser = PowerShellParser::new();
        let content = r#"
# Git aliases
Set-Alias gs git

# Editor
$env:EDITOR = "code"

# Load custom aliases
. .\custom_aliases.ps1

# Greeting function
function Get-Greeting {
    Write-Host "Hello"
}"#;
        let result = parser.parse(content);

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

        assert_eq!(main_entries.len(), 4);
    }

    #[test]
    fn test_code_block_line_range() {
        let parser = PowerShellParser::new();
        let content = "if ($true) {\n    Write-Host 'hi'\n}";
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
