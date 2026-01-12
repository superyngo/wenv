//! Syntax checking (placeholder for future implementation)

use super::{CheckResult, Checker};
use crate::model::Entry;

/// Checks for syntax errors (basic implementation)
pub struct SyntaxChecker;

impl Checker for SyntaxChecker {
    fn check(&self, _entries: &[Entry]) -> CheckResult {
        // Basic syntax checking is already done during parsing
        // This is a placeholder for additional syntax validation
        CheckResult::new()
    }
}
