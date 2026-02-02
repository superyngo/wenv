//! PATH environment variable merging utilities

use crate::model::Entry;
use std::collections::HashSet;

/// Result of merging multiple PATH definitions
#[derive(Debug, Clone)]
pub struct PathMergeResult {
    /// Merged PATH value (e.g., "$HOME/bin:$CARGO/bin:$PATH")
    pub merged_value: String,
    /// Line numbers of source entries that were merged
    pub source_lines: Vec<usize>,
}

/// Extract the value part from a complete export statement
///
/// # Examples
/// ```
/// // Input: export PATH="/usr/bin:$PATH"
/// // Output: /usr/bin:$PATH
/// //
/// // Input: export PATH="/dir1":"$PATH"
/// // Output: /dir1:$PATH
/// ```
fn extract_path_value(export_line: &str) -> String {
    let trimmed = export_line.trim();

    // Handle different export formats:
    // 1. export PATH="/value"
    // 2. export PATH='value'
    // 3. export PATH=value
    // 4. export PATH="/part1":"$PATH" (concatenated quoted strings)
    // 5. Just the value (for backward compatibility with tests)

    if let Some(eq_pos) = trimmed.find('=') {
        let value_part = &trimmed[eq_pos + 1..].trim();

        // Check if value starts with a quote
        let starts_with_quote = value_part.starts_with('"') || value_part.starts_with('\'');

        if !starts_with_quote {
            // Case: export PATH=/unquoted/value
            // Just return the value as-is
            return value_part.to_string();
        }

        // Handle quoted values (single quotes, double quotes, or concatenated)
        // Strategy: Find all quoted segments and concatenate them with ':'
        let mut result = String::new();
        let mut chars = value_part.chars().peekable();
        let mut in_double_quote = false;
        let mut in_single_quote = false;
        let mut current_segment = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                '"' if !in_single_quote => {
                    in_double_quote = !in_double_quote;
                    // If closing quote, save segment
                    if !in_double_quote && !current_segment.is_empty() {
                        if !result.is_empty() {
                            result.push(':');
                        }
                        result.push_str(&current_segment);
                        current_segment.clear();
                    }
                }
                '\'' if !in_double_quote => {
                    in_single_quote = !in_single_quote;
                    // If closing quote, save segment
                    if !in_single_quote && !current_segment.is_empty() {
                        if !result.is_empty() {
                            result.push(':');
                        }
                        result.push_str(&current_segment);
                        current_segment.clear();
                    }
                }
                ':' if !in_double_quote && !in_single_quote => {
                    // Ignore colons outside quotes (separator between quoted segments)
                    continue;
                }
                _ => {
                    if in_double_quote || in_single_quote {
                        current_segment.push(ch);
                    }
                    // Ignore content outside quotes (spaces, etc.)
                }
            }
        }

        // Handle remaining segment (shouldn't happen in well-formed input)
        if !current_segment.is_empty() {
            if !result.is_empty() {
                result.push(':');
            }
            result.push_str(&current_segment);
        }

        result
    } else {
        // No '=' found, assume it's just the value (backward compatibility)
        trimmed.to_string()
    }
}

/// Merge multiple PATH environment variable definitions into a single one
///
/// # Logic
/// 1. Extract all path segments from each definition
/// 2. Remove duplicates while preserving order
/// 3. Ensure `$PATH` self-reference appears at the end
/// 4. Return merged value and source line numbers
///
/// # Example
/// ```
/// // Input:
/// // export PATH="$HOME/bin:$PATH"
/// // export PATH="$CARGO_HOME/bin:$PATH"
/// // export PATH="/usr/local/go/bin:$PATH"
///
/// // Output:
/// // export PATH="$HOME/bin:$CARGO_HOME/bin:/usr/local/go/bin:$PATH"
/// ```
pub fn merge_path_definitions(entries: &[&Entry]) -> Option<PathMergeResult> {
    if entries.is_empty() {
        return None;
    }

    // Only process EnvVar entries with name "PATH"
    let path_entries: Vec<&Entry> = entries
        .iter()
        .filter(|e| {
            e.entry_type == crate::model::EntryType::EnvVar && e.name.to_uppercase() == "PATH"
        })
        .copied()
        .collect();

    if path_entries.len() <= 1 {
        return None; // No need to merge single or no PATH definitions
    }

    let mut segments: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut has_path_ref = false;
    let mut source_lines: Vec<usize> = Vec::new();

    for entry in &path_entries {
        if let Some(line) = entry.line_number {
            source_lines.push(line);
        }

        // Extract value from complete export syntax
        let path_value = extract_path_value(&entry.value);

        // Split value by ':'
        for segment in path_value.split(':') {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check if this is a $PATH self-reference
            if trimmed == "$PATH" || trimmed == "${PATH}" {
                has_path_ref = true;
                continue; // Don't add to segments, will append at end
            }

            // Add unique segments
            if seen.insert(trimmed.to_string()) {
                segments.push(trimmed.to_string());
            }
        }
    }

    // Append $PATH reference at the end if any definition had it
    if has_path_ref {
        segments.push("$PATH".to_string());
    }

    let merged_value = segments.join(":");

    Some(PathMergeResult {
        merged_value,
        source_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entry, EntryType};

    #[test]
    fn test_no_merge_needed_single_path() {
        let entry = Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into())
            .with_line_number(1);
        let result = merge_path_definitions(&[&entry]);
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_multiple_paths() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into())
                .with_line_number(1),
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                "$CARGO_HOME/bin:$PATH".into(),
            )
            .with_line_number(2),
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                "/usr/local/go/bin:$PATH".into(),
            )
            .with_line_number(3),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        assert_eq!(
            result.merged_value,
            "$HOME/bin:$CARGO_HOME/bin:/usr/local/go/bin:$PATH"
        );
        assert_eq!(result.source_lines, vec![1, 2, 3]);
    }

    #[test]
    fn test_deduplicate_paths() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into())
                .with_line_number(1),
            Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into())
                .with_line_number(2), // Duplicate
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        assert_eq!(result.merged_value, "$HOME/bin:$PATH");
        assert_eq!(result.source_lines, vec![1, 2]);
    }

    #[test]
    fn test_preserve_order() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "/usr/bin:$PATH".into())
                .with_line_number(1),
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                "/usr/local/bin:$PATH".into(),
            )
            .with_line_number(2),
            Entry::new(EntryType::EnvVar, "PATH".into(), "/opt/bin:$PATH".into())
                .with_line_number(3),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        assert_eq!(
            result.merged_value,
            "/usr/bin:/usr/local/bin:/opt/bin:$PATH"
        );
    }

    #[test]
    fn test_handle_missing_path_ref() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin".into()).with_line_number(1),
            Entry::new(EntryType::EnvVar, "PATH".into(), "$CARGO_HOME/bin".into())
                .with_line_number(2),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        // No $PATH reference in the result
        assert_eq!(result.merged_value, "$HOME/bin:$CARGO_HOME/bin");
    }

    #[test]
    fn test_mixed_path_refs() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into())
                .with_line_number(1),
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                "$CARGO_HOME/bin".into(), // No $PATH ref
            )
            .with_line_number(2),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        // Should still append $PATH since one definition has it
        assert_eq!(result.merged_value, "$HOME/bin:$CARGO_HOME/bin:$PATH");
    }

    #[test]
    fn test_case_insensitive_path_name() {
        let entries = vec![
            Entry::new(EntryType::EnvVar, "Path".into(), "$HOME/bin:$PATH".into())
                .with_line_number(1),
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                "$CARGO_HOME/bin:$PATH".into(),
            )
            .with_line_number(2),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        assert_eq!(result.merged_value, "$HOME/bin:$CARGO_HOME/bin:$PATH");
    }

    #[test]
    fn test_ignore_non_envvar_entries() {
        use crate::model::EntryType;

        let entries = vec![
            Entry::new(EntryType::EnvVar, "PATH".into(), "$HOME/bin:$PATH".into())
                .with_line_number(1),
            Entry::new(EntryType::Alias, "PATH".into(), "echo test".into()).with_line_number(2), // Ignore
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs);

        // Only 1 EnvVar PATH, so no merge
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_with_complete_export_syntax() {
        // Test with real parser output format (complete syntax in value field)
        let entries = vec![
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                r#"export PATH="/usr/local/bin:/usr/sbin:$HOME/.wenget/bin:$HOME/.bun/bin:$HOME/.local/bin:$PATH""#.into(),
            )
            .with_line_number(1),
            Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                r#"export PATH="/run/user/1000/fnm_multishells/6500_1769758207167/bin":"$PATH""#.into(),
            )
            .with_line_number(2),
        ];

        let entry_refs: Vec<&Entry> = entries.iter().collect();
        let result = merge_path_definitions(&entry_refs).unwrap();

        // Expected: All paths merged correctly, not string concatenation
        assert_eq!(
            result.merged_value,
            r#"/usr/local/bin:/usr/sbin:$HOME/.wenget/bin:$HOME/.bun/bin:$HOME/.local/bin:/run/user/1000/fnm_multishells/6500_1769758207167/bin:$PATH"#
        );
        assert_eq!(result.source_lines, vec![1, 2]);
    }

    #[test]
    fn test_extract_value_simple_quoted() {
        let input = r#"export PATH="/usr/bin:$PATH""#;
        assert_eq!(extract_path_value(input), "/usr/bin:$PATH");
    }

    #[test]
    fn test_extract_value_concatenated_quotes() {
        let input = r#"export PATH="/usr/bin":"$PATH""#;
        assert_eq!(extract_path_value(input), "/usr/bin:$PATH");
    }

    #[test]
    fn test_extract_value_unquoted() {
        let input = "export PATH=/usr/bin:$PATH";
        assert_eq!(extract_path_value(input), "/usr/bin:$PATH");
    }

    #[test]
    fn test_extract_value_single_quotes() {
        let input = "export PATH='/usr/bin:$PATH'";
        assert_eq!(extract_path_value(input), "/usr/bin:$PATH");
    }

    #[test]
    fn test_extract_value_backward_compat() {
        // Test backward compatibility with old test format (just the value)
        let input = "$HOME/bin:$PATH";
        assert_eq!(extract_path_value(input), "$HOME/bin:$PATH");
    }
}
