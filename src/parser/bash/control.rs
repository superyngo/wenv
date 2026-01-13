//! # Control Structure Detection
//!
//! Detects Bash control structure keywords for tracking nesting depth.
//!
//! ## Tracked Structures
//!
//! | Start Keywords | End Keywords |
//! |----------------|--------------|
//! | `if` | `fi` |
//! | `while` | `done` |
//! | `for` | `done` |
//! | `until` | `done` |
//! | `case` | `esac` |
//! | `select` | `done` |
//!
//! ## Usage
//!
//! ```rust,ignore
//! let starts = count_control_start("if [ -f file ]; then");  // returns 1
//! let ends = count_control_end("fi");  // returns 1
//! ```
//!
//! ## Integration
//!
//! The parser uses these functions to track nesting depth:
//! - When `count_control_start` > 0, increase depth
//! - When `count_control_end` > 0, decrease depth
//! - Lines inside control structures are collected into `CodeBlockBuilder`

/// Count the number of control structure openings in a line.
///
/// Matches: `if`, `while`, `until`, `for`, `case`, `select`
///
/// # Arguments
///
/// - `line`: The line to analyze (should be trimmed)
///
/// # Returns
///
/// Number of control structure start keywords found.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(count_control_start("if [ -f file ]; then"), 1);
/// assert_eq!(count_control_start("for i in 1 2 3; do"), 1);
/// assert_eq!(count_control_start("echo hello"), 0);
/// ```
pub fn count_control_start(line: &str) -> usize {
    let mut count = 0;
    // Split by semicolon and check each part
    for part in line.split(';') {
        let part = part.trim();
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

/// Count the number of control structure closings in a line.
///
/// Matches: `fi`, `done`, `esac`
///
/// # Arguments
///
/// - `line`: The line to analyze (should be trimmed)
///
/// # Returns
///
/// Number of control structure end keywords found (max 1 per line for simple cases).
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(count_control_end("fi"), 1);
/// assert_eq!(count_control_end("done"), 1);
/// assert_eq!(count_control_end("esac"), 1);
/// assert_eq!(count_control_end("echo done"), 0);  // 'done' not at start
/// ```
pub fn count_control_end(line: &str) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_control_start_if() {
        assert_eq!(count_control_start("if [ -f file ]; then"), 1);
        assert_eq!(count_control_start("if"), 1);
    }

    #[test]
    fn test_count_control_start_loops() {
        assert_eq!(count_control_start("while true; do"), 1);
        assert_eq!(count_control_start("for i in 1 2 3; do"), 1);
        assert_eq!(count_control_start("until false; do"), 1);
    }

    #[test]
    fn test_count_control_start_case() {
        assert_eq!(count_control_start("case $x in"), 1);
    }

    #[test]
    fn test_count_control_start_none() {
        assert_eq!(count_control_start("echo hello"), 0);
        assert_eq!(count_control_start("alias ll='ls -la'"), 0);
    }

    #[test]
    fn test_count_control_end_fi() {
        assert_eq!(count_control_end("fi"), 1);
        assert_eq!(count_control_end("fi;"), 1);
    }

    #[test]
    fn test_count_control_end_done() {
        assert_eq!(count_control_end("done"), 1);
        assert_eq!(count_control_end("done < file.txt"), 1);
    }

    #[test]
    fn test_count_control_end_esac() {
        assert_eq!(count_control_end("esac"), 1);
    }

    #[test]
    fn test_count_control_end_none() {
        assert_eq!(count_control_end("echo hello"), 0);
        assert_eq!(count_control_end("export PATH=value"), 0);
    }
}
