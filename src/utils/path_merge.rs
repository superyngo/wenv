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

        // Split value by ':'
        for segment in entry.value.split(':') {
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
}
