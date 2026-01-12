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
    pub comment: Option<String>,
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
            comment: None,
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

    pub fn with_comment(mut self, comment: String) -> Self {
        self.comment = Some(comment);
        self
    }

    pub fn with_raw_line(mut self, raw_line: String) -> Self {
        self.raw_line = Some(raw_line);
        self
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
        let entry = Entry::new(EntryType::Alias, "ll".into(), "ls -la".into())
            .with_line_number(10)
            .with_comment("List files".into());

        assert_eq!(entry.name, "ll");
        assert_eq!(entry.value, "ls -la");
        assert_eq!(entry.line_number, Some(10));
        assert_eq!(entry.comment, Some("List files".into()));
    }
}
