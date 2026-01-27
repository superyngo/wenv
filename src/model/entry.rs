//! Entry data structures for shell configuration items

use serde::{Deserialize, Serialize};

/// Entry type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryType {
    Alias,
    Function,
    EnvVar,
    Source,
    Code,    // Raw code lines or control structures
    Comment, // Pure comment lines
}

impl std::fmt::Display for EntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryType::Alias => write!(f, "alias"),
            EntryType::Function => write!(f, "func"),
            EntryType::EnvVar => write!(f, "env"),
            EntryType::Source => write!(f, "source"),
            EntryType::Code => write!(f, "code"),
            EntryType::Comment => write!(f, "comment"),
        }
    }
}

impl std::str::FromStr for EntryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "alias" => Ok(EntryType::Alias),
            "func" | "function" => Ok(EntryType::Function),
            "env" | "envvar" => Ok(EntryType::EnvVar),
            "source" => Ok(EntryType::Source),
            "code" => Ok(EntryType::Code),
            "comment" => Ok(EntryType::Comment),
            _ => Err(format!("Unknown entry type: {}", s)),
        }
    }
}

/// A single configuration entry
///
/// # Field Semantics
/// - `entry_type`: Classification for UI filtering and grouping (Alias, Function, EnvVar, Source, Code, Comment)
/// - `name`: Extracted identifier purely for UI display and search (e.g., alias name, function name, line number for Code/Comment)
/// - `value`: **Complete raw syntax** including keywords, options, quotes, and any leading comments/blank lines merged from pending entries
/// - `line_number`: Starting line number in source file
/// - `end_line`: Ending line number for multi-line entries (functions, code blocks, merged comment+code)
///
/// # Value Field Evolution
/// Previously `value` contained cleaned/processed content (e.g., alias value without quotes).
/// Now `value` contains the **complete original syntax** that can be written back to file directly.
///
/// Examples:
/// - Alias: `value = "alias -g ll='ls -la'"` (not just `'ls -la'`)
/// - EnvVar: `value = "export PATH=\"/usr/bin\""`  (not just `"/usr/bin"`)
/// - Function: `value = "foo() { echo hi; }"` (complete definition)
/// - Source: `value = "source ~/.profile"` (not just `~/.profile`)
/// - Comment: `value = "# This is a comment"` (including `#` prefix)
/// - Code: `value = "echo hello"` (raw shell code)
///
/// # Comment/Blank Line Merging
/// When a Comment/blank line precedes a structured entry (Alias, Function, EnvVar, Source),
/// the pending mechanism merges them into a single entry with:
/// - `entry_type`: The structured entry type
/// - `value`: Combined content (e.g., `"# comment\n\nalias foo='bar'"`)
/// - `name`: Extracted from the structured part (e.g., `foo`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub entry_type: EntryType,
    pub name: String,
    pub value: String,
    pub line_number: Option<usize>,
    pub end_line: Option<usize>,
}

impl Entry {
    pub fn new(entry_type: EntryType, name: String, value: String) -> Self {
        Self {
            entry_type,
            name,
            value,
            line_number: None,
            end_line: None,
        }
    }

    pub fn with_line_number(mut self, line_number: usize) -> Self {
        self.line_number = Some(line_number);
        self
    }

    pub fn with_end_line(mut self, end_line: usize) -> Self {
        self.end_line = Some(end_line);
        self
    }

    /// Merge another entry into this one, extending the line range and combining content.
    ///
    /// # Merging Rules
    /// - Updates `end_line` to cover both entries
    /// - Merges `value` content with newline separator (value contains complete original content)
    /// - Type upgrade: Comment + non-empty Code -> Code
    /// - For merged entries, `value` contains all lines (comments, blanks, code)
    /// - `name` is updated to reflect the merged line range for Comment/Code entries
    ///
    /// # Examples
    /// ```
    /// // Comment + Alias merging:
    /// // Entry 1: Comment { value: "# Set up path alias", line_number: 1 }
    /// // Entry 2: Alias { value: "alias p='pwd'", line_number: 3 }
    /// // Result: Alias { value: "# Set up path alias\n\nalias p='pwd'", name: "p", line_number: 1, end_line: 3 }
    /// ```
    pub fn merge_trailing(&mut self, other: Entry) {
        // Update end_line to cover the other entry
        self.end_line = other.end_line.or(other.line_number);

        // Merge value content (contains complete original content including comments, blanks)
        // value format: lines separated by \n (separator format, not terminator)
        self.value.push('\n');
        self.value.push_str(&other.value);

        // Type upgrade: Comment + non-empty Code -> Code
        // Keep self.value's first line for list display when Comment->Code upgrade happens
        if self.entry_type == EntryType::Comment
            && other.entry_type == EntryType::Code
            && !other.value.is_empty()
        {
            self.entry_type = EntryType::Code;
            // Don't overwrite self.name - preserve Comment's first line identifier
            // Complete content is now in self.value
        }

        // Update name to reflect new line range for Comment/Code entries
        if let (Some(start), Some(end)) = (self.line_number, self.end_line) {
            let prefix = if self.entry_type == EntryType::Comment {
                "#L"
            } else {
                "L"
            };
            if start == end {
                self.name = format!("{}{}", prefix, start);
            } else {
                self.name = format!("{}{}-L{}", prefix, start, end);
            }
        }
    }

    /// Check if this is a blank line entry (Code with empty or whitespace-only value).
    pub fn is_blank(&self) -> bool {
        self.entry_type == EntryType::Code && self.value.trim().is_empty()
    }
}

/// Parse result containing entries and warnings
#[derive(Debug)]
pub struct ParseResult {
    pub entries: Vec<Entry>,
    pub warnings: Vec<ParseWarning>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn add_warning(&mut self, warning: ParseWarning) {
        self.warnings.push(warning);
    }
}

impl Default for ParseResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Warning generated during parsing
#[derive(Debug)]
pub struct ParseWarning {
    pub line_number: usize,
    pub message: String,
    pub raw_line: String,
}

impl ParseWarning {
    pub fn new(
        line_number: usize,
        message: impl Into<String>,
        raw_line: impl Into<String>,
    ) -> Self {
        Self {
            line_number,
            message: message.into(),
            raw_line: raw_line.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_display() {
        assert_eq!(format!("{}", EntryType::Alias), "alias");
        assert_eq!(format!("{}", EntryType::Function), "func");
        assert_eq!(format!("{}", EntryType::EnvVar), "env");
        assert_eq!(format!("{}", EntryType::Source), "source");
    }

    #[test]
    fn test_entry_type_from_str() {
        assert_eq!("alias".parse::<EntryType>().unwrap(), EntryType::Alias);
        assert_eq!("func".parse::<EntryType>().unwrap(), EntryType::Function);
        assert_eq!(
            "function".parse::<EntryType>().unwrap(),
            EntryType::Function
        );
        assert_eq!("env".parse::<EntryType>().unwrap(), EntryType::EnvVar);
        assert_eq!("source".parse::<EntryType>().unwrap(), EntryType::Source);
    }

    #[test]
    fn test_entry_creation() {
        let entry = Entry::new(EntryType::Alias, "ll".into(), "ls -la".into()).with_line_number(10);

        assert_eq!(entry.name, "ll");
        assert_eq!(entry.value, "ls -la");
        assert_eq!(entry.line_number, Some(10));
    }

    #[test]
    fn test_entry_is_blank() {
        let blank = Entry::new(EntryType::Code, "L1".into(), String::new()).with_line_number(1);
        assert!(blank.is_blank());

        let non_blank = Entry::new(EntryType::Code, "L1".into(), "echo hi".into());
        assert!(!non_blank.is_blank());

        let comment = Entry::new(EntryType::Comment, "#L1".into(), "header".into());
        assert!(!comment.is_blank());
    }

    #[test]
    fn test_entry_merge_trailing_comment_absorbs_blank() {
        let mut comment =
            Entry::new(EntryType::Comment, "#L1".into(), "# Header".into()).with_line_number(1);

        let blank = Entry::new(EntryType::Code, "L2".into(), String::new()).with_line_number(2);

        comment.merge_trailing(blank);

        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(2));
        assert_eq!(comment.name, "#L1-L2"); // Comment keeps #L prefix
                                            // value contains complete content
        assert_eq!(comment.value, "# Header\n");
    }

    #[test]
    fn test_entry_merge_trailing_comment_plus_code() {
        let mut comment =
            Entry::new(EntryType::Comment, "#L1".into(), "# Note".into()).with_line_number(1);

        let code =
            Entry::new(EntryType::Code, "L2".into(), "echo hello".into()).with_line_number(2);

        comment.merge_trailing(code);

        assert_eq!(comment.entry_type, EntryType::Code);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(2));
        // value contains complete content (comment + code)
        assert_eq!(comment.value, "# Note\necho hello");
        // name preserves Comment's first line identifier but loses #L prefix when upgraded to Code
        assert_eq!(comment.name, "L1-L2");
    }

    #[test]
    fn test_entry_merge_trailing_value() {
        let mut entry =
            Entry::new(EntryType::Code, "L1".into(), "line1".into()).with_line_number(1);

        let other = Entry::new(EntryType::Code, "L2".into(), "line2".into()).with_line_number(2);

        entry.merge_trailing(other);

        assert_eq!(entry.value, "line1\nline2");
    }
}
