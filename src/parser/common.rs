//! Common parsing utilities

use crate::model::Entry;

/// Builder for multi-line functions
#[derive(Debug)]
pub struct FunctionBuilder {
    pub name: String,
    pub start_line: usize,
    pub lines: Vec<String>,
}

impl FunctionBuilder {
    pub fn new(name: String, start_line: usize) -> Self {
        Self {
            name,
            start_line,
            lines: Vec::new(),
        }
    }

    pub fn add_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn build(self, entry_type: crate::model::EntryType) -> Entry {
        let body = self.lines.join("\n");
        Entry::new(entry_type, self.name, body)
            .with_line_number(self.start_line)
            .with_raw_line(self.lines.join("\n"))
    }
}

/// Builder for code blocks (control structures, etc.)
#[derive(Debug)]
pub struct CodeBlockBuilder {
    pub start_line: usize,
    pub lines: Vec<String>,
}

impl CodeBlockBuilder {
    pub fn new(start_line: usize) -> Self {
        Self {
            start_line,
            lines: Vec::new(),
        }
    }

    pub fn add_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn build(self) -> Entry {
        let end_line = self.start_line + self.lines.len() - 1;
        let name = if self.lines.len() == 1 {
            format!("L{}", self.start_line)
        } else {
            format!("L{}-L{}", self.start_line, end_line)
        };
        let body = self.lines.join("\n");
        Entry::new(crate::model::EntryType::Code, name, body.clone())
            .with_line_number(self.start_line)
            .with_end_line(end_line)
            .with_raw_line(body)
    }
}

/// Extract inline comment from a line
pub fn extract_comment(line: &str, comment_char: char) -> (String, Option<String>) {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let chars: Vec<char> = line.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            c if c == comment_char && !in_single_quote && !in_double_quote => {
                let code: String = chars[..i].iter().collect();
                let comment: String = chars[i + 1..].iter().collect();
                return (
                    code.trim_end().to_string(),
                    Some(comment.trim().to_string()),
                );
            }
            _ => {}
        }
    }

    (line.to_string(), None)
}

/// Count braces outside of quoted strings
/// Returns (open_brace_count, close_brace_count)
pub fn count_braces_outside_quotes(line: &str) -> (usize, usize) {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut open_count = 0;
    let mut close_count = 0;

    for c in line.chars() {
        match c {
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '{' if !in_single_quote && !in_double_quote => open_count += 1,
            '}' if !in_single_quote && !in_double_quote => close_count += 1,
            _ => {}
        }
    }

    (open_count, close_count)
}

/// Strip quotes from a value
pub fn strip_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_quotes_single() {
        assert_eq!(strip_quotes("'hello'"), "hello");
    }

    #[test]
    fn test_strip_quotes_double() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
    }

    #[test]
    fn test_strip_quotes_none() {
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn test_extract_comment() {
        let (code, comment) = extract_comment("alias ll='ls -la' # list files", '#');
        assert_eq!(code, "alias ll='ls -la'");
        assert_eq!(comment, Some("list files".to_string()));
    }

    #[test]
    fn test_extract_comment_in_quotes() {
        let (code, comment) = extract_comment("alias test='echo # not a comment'", '#');
        assert_eq!(code, "alias test='echo # not a comment'");
        assert_eq!(comment, None);
    }

    #[test]
    fn test_count_braces_simple() {
        let (open, close) = count_braces_outside_quotes("function test() {");
        assert_eq!(open, 1);
        assert_eq!(close, 0);
    }

    #[test]
    fn test_count_braces_in_string() {
        // Braces inside quotes should be ignored
        let (open, close) = count_braces_outside_quotes(r#"echo "Unmatched brace: {""#);
        assert_eq!(open, 0);
        assert_eq!(close, 0);
    }

    #[test]
    fn test_count_braces_mixed() {
        let (open, close) = count_braces_outside_quotes(r#"if test { echo "}" }"#);
        assert_eq!(open, 1);
        assert_eq!(close, 1);
    }

    #[test]
    fn test_count_braces_single_quotes() {
        let (open, close) = count_braces_outside_quotes("echo '{' }");
        assert_eq!(open, 0);
        assert_eq!(close, 1);
    }
}
