//! Indentation detection and formatting utilities
//!
//! This module provides utilities for detecting and normalizing indentation
//! in shell configuration files.

/// Detect the indentation style used in the content
/// Returns the most commonly used indentation unit (e.g., "    " for 4 spaces, "\t" for tab)
pub fn detect_indent_style(content: &str) -> String {
    let mut tab_count = 0;
    let mut space_counts: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();

    for line in content.lines() {
        if line.is_empty() || line.trim().is_empty() {
            continue;
        }

        let indent_len = line.len() - line.trim_start().len();
        if indent_len == 0 {
            continue;
        }

        if line.starts_with('\t') {
            tab_count += 1;
        } else if line.starts_with(' ') {
            *space_counts.entry(indent_len).or_insert(0) += 1;
        }
    }

    // If tabs are more common, use tab
    let total_space_count: usize = space_counts.values().sum();
    if tab_count > total_space_count / 2 && tab_count > 0 {
        return "\t".to_string();
    }

    // Find the most common space indentation that looks like a base unit
    // Common units: 2, 4, 8 spaces
    let common_indent = find_gcd_indent(&space_counts);
    " ".repeat(common_indent) // find_gcd_indent already defaults to 4
}

/// Find the greatest common divisor of indentation levels
/// This helps detect if the file uses 2-space or 4-space indentation
fn find_gcd_indent(space_counts: &std::collections::HashMap<usize, usize>) -> usize {
    if space_counts.is_empty() {
        return 4;
    }

    let indents: Vec<usize> = space_counts.keys().copied().collect();
    if indents.is_empty() {
        return 4;
    }

    let mut result = indents[0];
    for &indent in indents.iter().skip(1) {
        result = gcd(result, indent);
        if result == 1 {
            // If GCD is 1, likely mixed indentation, default to 4
            return 4;
        }
    }

    // Prefer 2 or 4, not 1 or odd numbers
    if result == 1 || result == 3 || result > 8 {
        4
    } else {
        result
    }
}

/// Calculate GCD of two numbers
fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Format function body with proper indentation, preserving relative structure
///
/// # Arguments
/// * `body` - The function body content (lines between `{` and `}`)
/// * `base_indent` - The base indentation unit to use (e.g., "    " or "\t")
///
/// # Returns
/// The formatted body with normalized indentation
pub fn format_body_preserve_relative(body: &str, base_indent: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();

    if lines.is_empty() {
        return String::new();
    }

    // Find minimum indentation among non-empty lines
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut result = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            result.push(String::new());
        } else {
            let current_indent = line.len() - line.trim_start().len();
            let relative_indent = current_indent.saturating_sub(min_indent);
            // Use base_indent for each level of relative indentation
            let indent_str = format!("{}{}", base_indent, " ".repeat(relative_indent));
            result.push(format!("{}{}", indent_str, line.trim_start()));
        }
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_indent_style_4_spaces() {
        let content = r#"
function test() {
    echo "hello"
    if true; then
        echo "nested"
    fi
}
"#;
        let style = detect_indent_style(content);
        assert_eq!(style, "    ");
    }

    #[test]
    fn test_detect_indent_style_2_spaces() {
        let content = r#"
function test() {
  echo "hello"
  if true; then
    echo "nested"
  fi
}
"#;
        let style = detect_indent_style(content);
        assert_eq!(style, "  ");
    }

    #[test]
    fn test_detect_indent_style_tabs() {
        let content =
            "function test() {\n\techo \"hello\"\n\tif true; then\n\t\techo \"nested\"\n\tfi\n}";
        let style = detect_indent_style(content);
        assert_eq!(style, "\t");
    }

    #[test]
    fn test_format_body_preserve_relative() {
        let body = "  echo \"start\"\n  if true; then\n    echo \"nested\"\n  fi";
        let formatted = format_body_preserve_relative(body, "    ");
        assert!(formatted.contains("    echo \"start\""));
        assert!(formatted.contains("      echo \"nested\""));
    }

    #[test]
    fn test_format_body_empty() {
        let body = "";
        let formatted = format_body_preserve_relative(body, "    ");
        assert_eq!(formatted, "");
    }
}
