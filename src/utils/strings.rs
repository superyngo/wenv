//! String utility functions

/// Split a string into lines, preserving trailing empty lines (unlike `.lines()`).
///
/// Rust's `str::lines()` strips trailing newlines:
/// - `"a\n".lines()` → `["a"]` (loses the trailing newline)
/// - `"a\n\n".lines()` → `["a", ""]` (only 1 empty line, but original has 2 newlines)
///
/// This function preserves trailing empty lines:
/// - `"a\nb"` → `["a", "b"]`
/// - `"a\n"` → `["a"]` (single trailing newline is treated as line terminator)
/// - `"a\n\n"` → `["a", ""]` (preserves trailing empty line)
/// - `"a\n\n\n"` → `["a", "", ""]` (preserves all trailing empty lines)
///
/// # Examples
///
/// ```
/// use wenv::utils::strings::split_lines_preserve_trailing;
///
/// assert_eq!(split_lines_preserve_trailing("a\nb"), vec!["a", "b"]);
/// assert_eq!(split_lines_preserve_trailing("a\n"), vec!["a"]);
/// assert_eq!(split_lines_preserve_trailing("a\n\n"), vec!["a", ""]);
/// assert_eq!(split_lines_preserve_trailing("a\n\n\n"), vec!["a", "", ""]);
/// assert_eq!(split_lines_preserve_trailing(""), Vec::<&str>::new());
/// ```
pub fn split_lines_preserve_trailing(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<&str> = s.split('\n').collect();

    // split('\n') produces an empty string after the final '\n'
    // This empty string represents the line terminator, not an actual empty line.
    // Pop it if present. The remaining empty strings represent actual empty lines.
    if s.ends_with('\n') && result.last() == Some(&"") {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_string() {
        assert_eq!(split_lines_preserve_trailing(""), Vec::<&str>::new());
    }

    #[test]
    fn test_no_newline() {
        assert_eq!(split_lines_preserve_trailing("hello"), vec!["hello"]);
    }

    #[test]
    fn test_single_trailing_newline() {
        // Single trailing newline is treated as line terminator, not an empty line
        assert_eq!(split_lines_preserve_trailing("hello\n"), vec!["hello"]);
    }

    #[test]
    fn test_two_trailing_newlines() {
        // Two trailing newlines = one empty line at the end
        assert_eq!(
            split_lines_preserve_trailing("hello\n\n"),
            vec!["hello", ""]
        );
    }

    #[test]
    fn test_three_trailing_newlines() {
        // Three trailing newlines = two empty lines at the end
        assert_eq!(
            split_lines_preserve_trailing("hello\n\n\n"),
            vec!["hello", "", ""]
        );
    }

    #[test]
    fn test_multiple_lines() {
        assert_eq!(
            split_lines_preserve_trailing("a\nb\nc"),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn test_multiple_lines_with_trailing() {
        assert_eq!(
            split_lines_preserve_trailing("a\nb\nc\n"),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn test_multiple_lines_with_trailing_empty() {
        assert_eq!(
            split_lines_preserve_trailing("a\nb\nc\n\n"),
            vec!["a", "b", "c", ""]
        );
    }

    #[test]
    fn test_comment_with_trailing_empty_lines() {
        // This is the actual use case: comment + 2 empty lines
        let input = "# Comment\n\n\n";
        assert_eq!(
            split_lines_preserve_trailing(input),
            vec!["# Comment", "", ""]
        );
    }

    #[test]
    fn test_only_newlines() {
        // "\n" = one empty line + terminator → [""]
        assert_eq!(split_lines_preserve_trailing("\n"), vec![""]);
        // "\n\n" = two empty lines + terminator → ["", ""]
        assert_eq!(split_lines_preserve_trailing("\n\n"), vec!["", ""]);
        // "\n\n\n" = three empty lines + terminator → ["", "", ""]
        assert_eq!(split_lines_preserve_trailing("\n\n\n"), vec!["", "", ""]);
    }
}
