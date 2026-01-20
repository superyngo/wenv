//! Checker module for validating configuration files

mod duplicate;

pub use duplicate::DuplicateChecker;

use crate::model::Entry;

/// Check result
#[derive(Debug)]
pub struct CheckResult {
    pub issues: Vec<CheckIssue>,
}

impl CheckResult {
    pub fn new() -> Self {
        Self { issues: Vec::new() }
    }

    pub fn add_issue(&mut self, issue: CheckIssue) {
        self.issues.push(issue);
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| matches!(i.severity, Severity::Error))
    }

    pub fn has_warnings(&self) -> bool {
        self.issues
            .iter()
            .any(|i| matches!(i.severity, Severity::Warning))
    }

    pub fn is_ok(&self) -> bool {
        self.issues.is_empty()
    }
}

impl Default for CheckResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Issue severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

/// A single check issue
#[derive(Debug)]
pub struct CheckIssue {
    pub severity: Severity,
    pub message: String,
    pub line_number: Option<usize>,
    pub entry_name: Option<String>,
}

impl CheckIssue {
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            line_number: None,
            entry_name: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            line_number: None,
            entry_name: None,
        }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line_number = Some(line);
        self
    }

    pub fn with_entry(mut self, name: impl Into<String>) -> Self {
        self.entry_name = Some(name.into());
        self
    }
}

/// Trait for checkers
pub trait Checker {
    fn check(&self, entries: &[Entry]) -> CheckResult;
}

/// Run all checks on entries
pub fn check_all(entries: &[Entry]) -> CheckResult {
    let mut result = CheckResult::new();

    // Run duplicate check
    let dup_checker = DuplicateChecker;
    let dup_result = dup_checker.check(entries);
    result.issues.extend(dup_result.issues);

    result
}
