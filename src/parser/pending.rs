//! Unified Pending Block State Machine for Parsing
//!
//! This module provides a common abstraction for tracking multi-line entries
//! during parsing. Each parser (Bash, PowerShell) uses this to accumulate lines
//! until a block is complete.
//!
//! ## Design Philosophy
//!
//! The core concept: **First delimit the boundary of each entry, then produce the entry.**
//!
//! All lines pass through the pending state machine before becoming an Entry.
//! This ensures consistent handling of multi-line constructs.
//!
//! ## Boundary Types
//!
//! | Type | Description | Examples |
//! |------|-------------|----------|
//! | `Complete` | Single-line, already done | `alias x='y'` |
//! | `BraceCounting` | Track `{` and `}` | `function() { ... }` |
//! | `QuoteCounting` | Track odd/even quotes | Multi-line aliases |
//! | `KeywordTracking` | Track control keywords | `if`/`fi`, `while`/`done` |
//! | `AdjacentMerging` | Merge consecutive lines | Comments, blank lines |

use crate::model::EntryType;

/// Boundary detection type - determines when a pending block is complete.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryType {
    /// Single-line entry, already complete.
    Complete,

    /// Track brace balance `{ }` for functions and control blocks.
    /// Block is complete when brace_count reaches 0.
    BraceCounting {
        /// Current brace count (open - close).
        brace_count: i32,
    },

    /// Track parenthesis balance `( )` for multi-line structures like `plugins=(...)`.
    /// Block is complete when parenthesis_count reaches 0.
    ParenthesisCounting {
        /// Current parenthesis count (open - close).
        parenthesis_count: i32,
    },

    /// Track single-quote parity for multi-line quoted values.
    /// Block is complete when quote count is even.
    QuoteCounting {
        /// Current single-quote count.
        quote_count: usize,
    },

    /// Track control structure depth via keywords.
    /// Block is complete when depth reaches 0.
    KeywordTracking {
        /// Current control structure depth.
        depth: usize,
    },

    /// Merge adjacent lines of the same type (comments, blank lines).
    /// Block is complete when a different line type is encountered.
    AdjacentMerging {
        /// The type of line being merged.
        merge_type: MergeType,
    },
}

/// Type of content being merged in AdjacentMerging mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MergeType {
    /// Consecutive comment lines (starting with `#`).
    Comment,
    /// Consecutive blank lines.
    BlankLines,
    /// Non-blank code lines that absorb trailing blanks.
    CodeWithBlanks,
}

/// Accumulated pending block during parsing.
///
/// This accumulates lines until the boundary condition is satisfied,
/// then gets converted into an Entry.
#[derive(Debug)]
pub struct PendingBlock {
    /// Lines accumulated so far (original content, not trimmed).
    pub lines: Vec<String>,

    /// Starting line number (1-indexed).
    pub start_line: usize,

    /// Current ending line number (1-indexed).
    pub end_line: usize,

    /// Boundary detection strategy.
    pub boundary: BoundaryType,

    /// Hint for the entry type (set when block starts).
    pub entry_hint: Option<EntryType>,

    /// Name for the entry (e.g., function name, alias name).
    pub name: Option<String>,

    /// Extracted value (for Alias/EnvVar/etc).
    pub value: Option<String>,

    /// Number of pure comment lines (excluding blank lines).
    /// Used to decide merge behavior: single comment merges down, multiple don't.
    pub comment_count: usize,
}

impl PendingBlock {
    /// Create a new pending block.
    pub fn new(start_line: usize, first_line: &str, boundary: BoundaryType) -> Self {
        Self {
            lines: vec![first_line.to_string()],
            start_line,
            end_line: start_line,
            boundary,
            entry_hint: None,
            name: None,
            value: None,
            comment_count: 0,
        }
    }

    /// Create a pending block for a function.
    pub fn function(name: String, start_line: usize, first_line: &str, brace_count: i32) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::BraceCounting { brace_count },
        );
        block.entry_hint = Some(EntryType::Function);
        block.name = Some(name);
        block
    }

    /// Create a pending block for a control structure.
    pub fn control(start_line: usize, first_line: &str, depth: usize) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::KeywordTracking { depth },
        );
        block.entry_hint = Some(EntryType::Code);
        block
    }

    /// Create a pending block for a multi-line alias.
    pub fn multiline_alias(
        name: String,
        start_line: usize,
        first_line: &str,
        quote_count: usize,
    ) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::QuoteCounting { quote_count },
        );
        block.entry_hint = Some(EntryType::Alias);
        block.name = Some(name);
        block
    }

    /// Create a pending block for a multi-line environment variable.
    pub fn multiline_env(
        name: String,
        start_line: usize,
        first_line: &str,
        quote_count: usize,
    ) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::QuoteCounting { quote_count },
        );
        block.entry_hint = Some(EntryType::EnvVar);
        block.name = Some(name);
        block
    }

    /// Create a pending block for a comment.
    pub fn comment(start_line: usize, first_line: &str) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::AdjacentMerging {
                merge_type: MergeType::Comment,
            },
        );
        block.entry_hint = Some(EntryType::Comment);
        block.comment_count = 1; // First line is a comment
        block
    }

    /// Create a pending block for blank lines.
    pub fn blank_lines(start_line: usize, first_line: &str) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::AdjacentMerging {
                merge_type: MergeType::BlankLines,
            },
        );
        block.entry_hint = Some(EntryType::Code);
        block.comment_count = 0; // No comments
        block
    }

    /// Create a pending block for non-blank code (which absorbs trailing blanks).
    pub fn code(start_line: usize, first_line: &str) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::AdjacentMerging {
                merge_type: MergeType::CodeWithBlanks,
            },
        );
        block.entry_hint = Some(EntryType::Code);
        block
    }

    /// Add a line to the pending block and update the end line.
    pub fn add_line(&mut self, line: &str, line_number: usize) {
        self.lines.push(line.to_string());
        self.end_line = line_number;
    }

    /// Check if this block is complete based on its boundary type.
    pub fn is_complete(&self) -> bool {
        match &self.boundary {
            BoundaryType::Complete => true,
            BoundaryType::BraceCounting { brace_count } => *brace_count == 0,
            BoundaryType::ParenthesisCounting { parenthesis_count } => *parenthesis_count == 0,
            BoundaryType::QuoteCounting { quote_count } => quote_count % 2 == 0,
            BoundaryType::KeywordTracking { depth } => *depth == 0,
            // AdjacentMerging blocks are never "complete" by themselves;
            // they're completed externally when a non-matching line is seen.
            BoundaryType::AdjacentMerging { .. } => false,
        }
    }

    /// Update brace count for BraceCounting boundary.
    pub fn update_brace_count(&mut self, open: i32, close: i32) {
        if let BoundaryType::BraceCounting {
            ref mut brace_count,
        } = self.boundary
        {
            *brace_count += open;
            *brace_count = (*brace_count).saturating_sub(close);
        }
    }

    /// Update parenthesis count for ParenthesisCounting boundary.
    pub fn update_parenthesis_count(&mut self, open: i32, close: i32) {
        if let BoundaryType::ParenthesisCounting {
            ref mut parenthesis_count,
        } = self.boundary
        {
            *parenthesis_count += open;
            *parenthesis_count = (*parenthesis_count).saturating_sub(close);
        }
    }

    /// Update quote count for QuoteCounting boundary.
    pub fn add_quotes(&mut self, count: usize) {
        if let BoundaryType::QuoteCounting {
            ref mut quote_count,
        } = self.boundary
        {
            *quote_count += count;
        }
    }

    /// Update depth for KeywordTracking boundary.
    pub fn update_keyword_depth(&mut self, start_count: usize, end_count: usize) {
        if let BoundaryType::KeywordTracking { ref mut depth } = self.boundary {
            *depth = (*depth).saturating_sub(end_count);
            *depth += start_count;
        }
    }

    /// Get the raw content as a single string (joined by newlines).
    pub fn raw_content(&self) -> String {
        self.lines.join("\n")
    }

    /// Get the merge type if this is an AdjacentMerging block.
    pub fn merge_type(&self) -> Option<MergeType> {
        if let BoundaryType::AdjacentMerging { merge_type } = &self.boundary {
            Some(*merge_type)
        } else {
            None
        }
    }

    /// Upgrade this block's type (e.g., Comment → Code when non-blank code is merged).
    pub fn upgrade_to_code(&mut self) {
        self.entry_hint = Some(EntryType::Code);
        if let BoundaryType::AdjacentMerging { ref mut merge_type } = self.boundary {
            *merge_type = MergeType::CodeWithBlanks;
        }
    }

    /// Check if this block should merge with a blank line.
    pub fn can_absorb_blank(&self) -> bool {
        match &self.boundary {
            BoundaryType::AdjacentMerging { merge_type } => {
                matches!(
                    merge_type,
                    MergeType::Comment | MergeType::BlankLines | MergeType::CodeWithBlanks
                )
            }
            _ => false,
        }
    }

    /// Check if this block should merge with a comment line.
    pub fn can_absorb_comment(&self) -> bool {
        match &self.boundary {
            BoundaryType::AdjacentMerging { merge_type } => {
                matches!(merge_type, MergeType::Comment)
            }
            _ => false,
        }
    }

    /// Increment comment count when absorbing a comment line.
    pub fn increment_comment_count(&mut self) {
        self.comment_count += 1;
    }

    /// Check if this pending block represents a structured entry (Alias/EnvVar/Source/Function)
    /// that is absorbing trailing blank lines.
    pub fn is_structured_entry(&self) -> bool {
        matches!(
            self.entry_hint,
            Some(EntryType::Alias)
                | Some(EntryType::EnvVar)
                | Some(EntryType::Source)
                | Some(EntryType::Function)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_block_complete() {
        let block = PendingBlock::new(1, "alias x='y'", BoundaryType::Complete);
        assert!(block.is_complete());
    }

    #[test]
    fn test_pending_block_brace_counting() {
        let mut block = PendingBlock::function("test".to_string(), 1, "test() {", 1);
        assert!(!block.is_complete());

        block.update_brace_count(0, 1);
        assert!(block.is_complete());
    }

    #[test]
    fn test_pending_block_quote_counting() {
        let mut block =
            PendingBlock::multiline_alias("test".to_string(), 1, "alias test='line1", 1);
        assert!(!block.is_complete());

        block.add_quotes(1); // closing quote
        assert!(block.is_complete());
    }

    #[test]
    fn test_pending_block_keyword_tracking() {
        let mut block = PendingBlock::control(1, "if [ -f file ]; then", 1);
        assert!(!block.is_complete());

        block.update_keyword_depth(0, 1); // fi encountered
        assert!(block.is_complete());
    }

    #[test]
    fn test_pending_block_adjacent_merging() {
        let block = PendingBlock::comment(1, "# header");
        assert!(!block.is_complete()); // never complete by itself
        assert!(block.can_absorb_blank());
        assert!(block.can_absorb_comment());
    }

    #[test]
    fn test_pending_block_raw_content() {
        let mut block = PendingBlock::new(1, "line1", BoundaryType::Complete);
        block.add_line("line2", 2);
        block.add_line("line3", 3);

        assert_eq!(block.raw_content(), "line1\nline2\nline3");
        assert_eq!(block.start_line, 1);
        assert_eq!(block.end_line, 3);
    }

    #[test]
    fn test_upgrade_to_code() {
        let mut block = PendingBlock::comment(1, "# header");
        assert_eq!(block.entry_hint, Some(EntryType::Comment));

        block.upgrade_to_code();
        assert_eq!(block.entry_hint, Some(EntryType::Code));
        assert_eq!(block.merge_type(), Some(MergeType::CodeWithBlanks));
    }
}
