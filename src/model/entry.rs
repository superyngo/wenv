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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub entry_type: EntryType,
    pub name: String,
    pub value: String,
    pub line_number: Option<usize>,
    pub end_line: Option<usize>, // For multi-line code blocks
    pub raw_line: Option<String>,
}

impl Entry {
    pub fn new(entry_type: EntryType, name: String, value: String) -> Self {
        Self {
            entry_type,
            name,
            value,
            line_number: None,
            end_line: None,
            raw_line: None,
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

    pub fn with_raw_line(mut self, raw_line: String) -> Self {
        self.raw_line = Some(raw_line);
        self
    }

    /// Merge another entry into this one, extending the line range.
    ///
    /// Merging rules:
    /// - Updates end_line to cover both entries
    /// - Merges raw_line content with newline separator (raw_line contains complete original content)
    /// - Type upgrade: Comment + non-empty Code → Code
    /// - No need to store comment in `.comment` field - raw_line already has complete content
    pub fn merge_trailing(&mut self, other: Entry) {
        // Update end_line to cover the other entry
        self.end_line = other.end_line.or(other.line_number);

        // Merge raw_line content (contains complete original content including comments, blanks)
        if let Some(ref mut raw) = self.raw_line {
            if let Some(other_raw) = other.raw_line {
                raw.push('\n');
                raw.push_str(&other_raw);
            }
        } else {
            self.raw_line = other.raw_line;
        }

        // Type upgrade: Comment + non-empty Code → Code
        // Keep self.value (first line of original Comment) for list display
        // raw_line already contains complete content
        if self.entry_type == EntryType::Comment
            && other.entry_type == EntryType::Code
            && !other.value.is_empty()
        {
            self.entry_type = EntryType::Code;
            // Don't overwrite self.name and self.value - preserve Comment's first line
            // for display purposes. Complete content is in raw_line.
        }

        // Update name to reflect new line range
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
        let mut comment = Entry::new(EntryType::Comment, "#L1".into(), "Header".into())
            .with_line_number(1)
            .with_raw_line("# Header".into());

        let blank = Entry::new(EntryType::Code, "L2".into(), String::new())
            .with_line_number(2)
            .with_raw_line(String::new());

        comment.merge_trailing(blank);

        assert_eq!(comment.entry_type, EntryType::Comment);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(2));
        assert_eq!(comment.name, "#L1-L2"); // Comment keeps #L prefix
    }

    #[test]
    fn test_entry_merge_trailing_comment_plus_code() {
        let mut comment = Entry::new(EntryType::Comment, "#L1".into(), "Note".into())
            .with_line_number(1)
            .with_raw_line("# Note".into());

        let code = Entry::new(EntryType::Code, "L2".into(), "echo hello".into())
            .with_line_number(2)
            .with_raw_line("echo hello".into());

        comment.merge_trailing(code);

        assert_eq!(comment.entry_type, EntryType::Code);
        assert_eq!(comment.line_number, Some(1));
        assert_eq!(comment.end_line, Some(2));
        // value preserves Comment's first line for list display
        assert_eq!(comment.value, "Note");
        // raw_line contains complete content (comment + code)
        assert_eq!(comment.raw_line, Some("# Note\necho hello".into()));
        // name updated to reflect line range
        assert_eq!(comment.name, "L1-L2");
        // comment field is no longer set in merge_trailing - raw_line has complete content
    }

    #[test]
    fn test_entry_merge_trailing_raw_line() {
        let mut entry = Entry::new(EntryType::Code, "L1".into(), "line1".into())
            .with_line_number(1)
            .with_raw_line("line1".into());

        let other = Entry::new(EntryType::Code, "L2".into(), "line2".into())
            .with_line_number(2)
            .with_raw_line("line2".into());

        entry.merge_trailing(other);

        assert_eq!(entry.raw_line, Some("line1\nline2".into()));
    }
}
