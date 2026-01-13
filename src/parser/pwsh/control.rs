//! # Control Structure Detection (PowerShell)
//!
//! Detects PowerShell control structure keywords for tracking nesting depth.
//!
//! ## Tracked Structures
//!
//! | Start Keywords |
//! |----------------|
//! | `if` |
//! | `foreach` |
//! | `while` |
//! | `for` |
//! | `switch` |
//! | `try` |
//!
//! ## End Detection
//!
//! PowerShell control structures end with `}`. However, continuations like
//! `else`, `elseif`, `catch`, `finally` don't end the structure.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let starts = count_control_start("if ($true) {");  // returns 1
//! let ends = count_control_end("}");  // returns 1
//! let ends = count_control_end("} else {");  // returns 0 (continuation)
//! ```

/// Count the number of control structure openings in a line.
///
/// Matches: `if`, `foreach`, `while`, `for`, `switch`, `try`
///
/// # Arguments
///
/// - `line`: The line to analyze (should be trimmed)
///
/// # Returns
///
/// Number of control structure start keywords found.
pub fn count_control_start(line: &str) -> usize {
    let mut count = 0;
    let lower = line.to_lowercase();

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

/// Count the number of control structure closings in a line.
///
/// In PowerShell, control structures end with `}`, but continuations
/// like `else`, `elseif`, `catch`, `finally` don't count as endings.
///
/// # Arguments
///
/// - `line`: The line to analyze (should be trimmed)
///
/// # Returns
///
/// Number of control structure endings found.
pub fn count_control_end(line: &str) -> usize {
    let trimmed = line.trim();
    let lower = trimmed.to_lowercase();

    // A lone closing brace ends a control structure
    if trimmed == "}" {
        return 1;
    }

    // Check for end patterns that don't continue (no else/catch/finally)
    if trimmed.ends_with('}') {
        // If this is NOT a continuation keyword
        if !lower.contains("else") && !lower.contains("catch") && !lower.contains("finally") {
            let close_count = trimmed.matches('}').count();
            let open_count = trimmed.matches('{').count();
            if close_count > open_count {
                return close_count - open_count;
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_control_start_if() {
        assert_eq!(count_control_start("if ($true) {"), 1);
        assert_eq!(count_control_start("if($x)"), 1);
    }

    #[test]
    fn test_count_control_start_loops() {
        assert_eq!(count_control_start("foreach ($i in @(1,2,3)) {"), 1);
        assert_eq!(count_control_start("while ($true) {"), 1);
        assert_eq!(count_control_start("for ($i=0; $i -lt 10; $i++) {"), 1);
    }

    #[test]
    fn test_count_control_start_switch() {
        assert_eq!(count_control_start("switch ($x) {"), 1);
    }

    #[test]
    fn test_count_control_start_try() {
        assert_eq!(count_control_start("try {"), 1);
        assert_eq!(count_control_start("try{"), 1);
    }

    #[test]
    fn test_count_control_end_simple() {
        assert_eq!(count_control_end("}"), 1);
    }

    #[test]
    fn test_count_control_end_continuation() {
        assert_eq!(count_control_end("} else {"), 0);
        assert_eq!(count_control_end("} elseif ($x) {"), 0);
        assert_eq!(count_control_end("} catch {"), 0);
        assert_eq!(count_control_end("} finally {"), 0);
    }

    #[test]
    fn test_count_control_end_none() {
        assert_eq!(count_control_end("Write-Host 'hello'"), 0);
    }
}
