//! Duplicate definition checker

use super::{CheckIssue, CheckResult, Checker, Severity};
use crate::model::{Entry, EntryType};
use std::collections::HashMap;

/// Checks for duplicate definitions
pub struct DuplicateChecker;

impl Checker for DuplicateChecker {
    fn check(&self, entries: &[Entry]) -> CheckResult {
        let mut result = CheckResult::new();

        // Group by type and name
        let mut seen: HashMap<(EntryType, &str), Vec<&Entry>> = HashMap::new();

        for entry in entries {
            seen.entry((entry.entry_type, &entry.name))
                .or_default()
                .push(entry);
        }

        // Report duplicates
        for ((entry_type, name), occurrences) in seen {
            if occurrences.len() > 1 {
                let lines: Vec<String> = occurrences
                    .iter()
                    .filter_map(|e| e.line_number.map(|l| l.to_string()))
                    .collect();

                let issue = CheckIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Duplicate {} '{}' defined on lines: {}",
                        entry_type,
                        name,
                        lines.join(", ")
                    ),
                    line_number: occurrences.first().and_then(|e| e.line_number),
                    entry_name: Some(name.to_string()),
                };

                result.add_issue(issue);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_duplicates() {
        let entries = vec![
            Entry::new(EntryType::Alias, "ll".into(), "ls -la".into()),
            Entry::new(EntryType::Alias, "gs".into(), "git status".into()),
        ];

        let checker = DuplicateChecker;
        let result = checker.check(&entries);

        assert!(result.is_ok());
    }

    #[test]
    fn test_duplicate_alias() {
        let entries = vec![
            Entry::new(EntryType::Alias, "ll".into(), "ls -la".into()).with_line_number(1),
            Entry::new(EntryType::Alias, "ll".into(), "ls -l".into()).with_line_number(5),
        ];

        let checker = DuplicateChecker;
        let result = checker.check(&entries);

        assert!(!result.is_ok());
        assert_eq!(result.issues.len(), 1);
        assert!(result.issues[0].message.contains("Duplicate"));
    }

    #[test]
    fn test_same_name_different_type() {
        // Same name but different type should not be a duplicate
        let entries = vec![
            Entry::new(EntryType::Alias, "ll".into(), "ls -la".into()),
            Entry::new(EntryType::Function, "ll".into(), "echo hello".into()),
        ];

        let checker = DuplicateChecker;
        let result = checker.check(&entries);

        assert!(result.is_ok());
    }
}
