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

use crate::model::{Entry, EntryType, ParseResult};

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
        /// Depth of nested `<# ... #>` block comments.
        /// Braces inside block comments are not counted.
        block_comment_depth: usize,
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

    /// Block comment `<# ... #>` — accumulates lines until `#>` is found.
    BlockComment,
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

    /// Whether this block has absorbed blank lines.
    /// Used by CodeWithBlanks to decide if adjacent code should merge or start a new entry.
    pub has_absorbed_blanks: bool,
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
            has_absorbed_blanks: false,
        }
    }

    /// Create a pending block for a function.
    pub fn function(name: String, start_line: usize, first_line: &str, brace_count: i32) -> Self {
        let mut block = Self::new(
            start_line,
            first_line,
            BoundaryType::BraceCounting {
                brace_count,
                block_comment_depth: 0,
            },
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
            BoundaryType::BraceCounting { brace_count, .. } => *brace_count == 0,
            BoundaryType::ParenthesisCounting { parenthesis_count } => *parenthesis_count == 0,
            BoundaryType::QuoteCounting { quote_count } => quote_count % 2 == 0,
            BoundaryType::KeywordTracking { depth } => *depth == 0,
            // AdjacentMerging blocks are never "complete" by themselves;
            // they're completed externally when a non-matching line is seen.
            BoundaryType::AdjacentMerging { .. } => false,
            BoundaryType::BlockComment => false,
        }
    }

    /// Update brace count for BraceCounting boundary.
    pub fn update_brace_count(&mut self, open: i32, close: i32) {
        if let BoundaryType::BraceCounting {
            ref mut brace_count,
            ..
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
    /// Comment blocks that have absorbed blanks are sealed and reject further comments.
    pub fn can_absorb_comment(&self) -> bool {
        match &self.boundary {
            BoundaryType::AdjacentMerging { merge_type } => {
                matches!(merge_type, MergeType::Comment) && !self.has_absorbed_blanks
            }
            _ => false,
        }
    }

    /// Check if this pending block can merge downward into the next entry.
    /// Only a single comment line with no absorbed blanks qualifies.
    pub fn can_merge_down(&self) -> bool {
        self.comment_count == 1 && !self.has_absorbed_blanks
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
    /// Absorb a blank line into this block and mark it as having absorbed blanks
    /// if it is a CodeWithBlanks or Comment block.
    pub fn absorb_blank(&mut self, line: &str, line_number: usize) {
        self.add_line(line, line_number);
        if matches!(
            self.merge_type(),
            Some(MergeType::CodeWithBlanks) | Some(MergeType::Comment)
        ) {
            self.has_absorbed_blanks = true;
        }
    }

    /// Convert this pending block into a Comment or Code Entry, if applicable.
    ///
    /// Returns `Some(Entry)` if this block represents a Comment or Code (or has no hint,
    /// defaulting to Code). Returns `None` for structured types like Function, Alias, etc.
    pub fn into_comment_or_code_entry(&self) -> Option<Entry> {
        let entry_type = self.entry_hint.unwrap_or(EntryType::Code);
        let name = match entry_type {
            EntryType::Comment => {
                if self.start_line == self.end_line {
                    format!("#L{}", self.start_line)
                } else {
                    format!("#L{}-L{}", self.start_line, self.end_line)
                }
            }
            EntryType::Code => self.name.clone().unwrap_or_else(|| {
                if self.start_line == self.end_line {
                    format!("L{}", self.start_line)
                } else {
                    format!("L{}-L{}", self.start_line, self.end_line)
                }
            }),
            _ => return None,
        };

        Some(
            Entry::new(entry_type, name, self.raw_content())
                .with_line_number(self.start_line)
                .with_end_line(self.end_line),
        )
    }
}

pub fn entry_to_trailing_pending(entry: Entry) -> PendingBlock {
    PendingBlock {
        lines: entry.value.split('\n').map(|s| s.to_string()).collect(),
        start_line: entry.line_number.unwrap_or(1),
        end_line: entry.end_line.unwrap_or(entry.line_number.unwrap_or(1)),
        boundary: BoundaryType::AdjacentMerging {
            merge_type: MergeType::CodeWithBlanks,
        },
        entry_hint: Some(entry.entry_type),
        name: Some(entry.name),
        value: None,
        comment_count: 0,
        has_absorbed_blanks: false,
    }
}

pub fn merge_pending_with_structured<F>(
    pending: Option<PendingBlock>,
    entry: Entry,
    build_entry: F,
) -> (Option<Entry>, Entry)
where
    F: FnOnce(PendingBlock) -> Entry,
{
    if let Some(pending) = pending {
        if pending.can_merge_down() {
            let pending_content = pending.raw_content();
            let merged_value = format!("{}\n{}", pending_content, entry.value);
            let end_line = entry
                .end_line
                .or(entry.line_number)
                .unwrap_or(pending.start_line);
            return (
                None,
                Entry::new(entry.entry_type, entry.name, merged_value)
                    .with_line_number(pending.start_line)
                    .with_end_line(end_line),
            );
        }
        return (Some(build_entry(pending)), entry);
    }
    (None, entry)
}
/// Flush a pending block using the provided builder function.
pub fn flush_pending<F>(pending: &mut Option<PendingBlock>, build_entry: F) -> Option<Entry>
where
    F: FnOnce(PendingBlock) -> Entry,
{
    pending.take().map(build_entry)
}

/// Handle an empty line in the pending entry state machine.
///
/// If `pending_entry` can absorb the blank line, it absorbs it and returns `None`.
/// Otherwise, any existing pending block is returned to be flushed, and `pending_entry`
/// is replaced with a new blank line block.
pub fn handle_blank_line(
    pending_entry: &mut Option<PendingBlock>,
    line: &str,
    line_number: usize,
) -> Option<PendingBlock> {
    match pending_entry {
        Some(pending) if pending.can_absorb_blank() => {
            pending.absorb_blank(line, line_number);
            None
        }
        _ => pending_entry.replace(PendingBlock::blank_lines(line_number, line)),
    }
}

/// Handle a standalone comment line in the pending entry state machine.
///
/// If `pending_entry` can absorb the comment line, it absorbs it and returns `None`.
/// Otherwise, any existing pending block is returned to be flushed, and `pending_entry`
/// is replaced with a new comment block.
pub fn handle_comment_line(
    pending_entry: &mut Option<PendingBlock>,
    line: &str,
    line_number: usize,
) -> Option<PendingBlock> {
    match pending_entry {
        Some(pending) if pending.can_absorb_comment() => {
            pending.add_line(line, line_number);
            pending.increment_comment_count();
            None
        }
        _ => pending_entry.replace(PendingBlock::comment(line_number, line)),
    }
}

/// Handle a fallback non-blank code line in the pending entry state machine.
///
/// Merges into adjacent code or a single preceding comment line if eligible.
/// Otherwise, flushes the existing pending entry and starts a new code block.
pub fn handle_code_fallback(
    pending_entry: &mut Option<PendingBlock>,
    line: &str,
    line_number: usize,
) -> Option<PendingBlock> {
    match pending_entry {
        Some(pending)
            if pending.entry_hint == Some(EntryType::Comment) && pending.can_merge_down() =>
        {
            pending.add_line(line, line_number);
            pending.upgrade_to_code();
            None
        }
        Some(pending)
            if pending.entry_hint == Some(EntryType::Code)
                && pending.merge_type() == Some(MergeType::CodeWithBlanks)
                && !pending.has_absorbed_blanks =>
        {
            pending.add_line(line, line_number);
            None
        }
        _ => pending_entry.replace(PendingBlock::code(line_number, line)),
    }
}

/// Commit a structured entry, merging any eligible preceding pending entry
/// and setting the merged entry as trailing pending to absorb subsequent blank lines.
pub fn commit_structured_entry<F>(
    result: &mut ParseResult,
    pending_entry: &mut Option<PendingBlock>,
    entry: Entry,
    build_entry: F,
) where
    F: FnOnce(PendingBlock) -> Entry,
{
    let (pending_to_add, merged) =
        merge_pending_with_structured(pending_entry.take(), entry, build_entry);
    if let Some(pending_e) = pending_to_add {
        result.add_entry(pending_e);
    }
    *pending_entry = Some(entry_to_trailing_pending(merged));
}

/// Merge a preceding pending comment line into the start of a new block, or flush it.
///
/// Returns `(merged_first_line, start_line)`.
pub fn merge_pending_first_line<F>(
    result: &mut ParseResult,
    pending_entry: &mut Option<PendingBlock>,
    line: &str,
    line_number: usize,
    build_entry: F,
) -> (String, usize)
where
    F: FnOnce(PendingBlock) -> Entry,
{
    if let Some(pending) = pending_entry.take() {
        if pending.can_merge_down() {
            let merged = format!("{}\n{}", pending.raw_content(), line);
            (merged, pending.start_line)
        } else {
            result.add_entry(build_entry(pending));
            (line.to_string(), line_number)
        }
    } else {
        (line.to_string(), line_number)
    }
}

/// Start a multi-line pending block from a `ParseEvent::Started`, merging any preceding
/// single comment line into the block's first line, or flushing the pending entry.
#[allow(clippy::too_many_arguments)]
pub fn start_multiline_from_event<F>(
    result: &mut ParseResult,
    pending_entry: &mut Option<PendingBlock>,
    entry_type: EntryType,
    name: String,
    boundary: BoundaryType,
    line: &str,
    line_number: usize,
    build_entry: F,
) -> PendingBlock
where
    F: FnOnce(PendingBlock) -> Entry,
{
    let (merged_first_line, start_line) =
        merge_pending_first_line(result, pending_entry, line, line_number, build_entry);
    PendingBlock {
        lines: vec![merged_first_line],
        start_line,
        end_line: line_number,
        boundary,
        entry_hint: Some(entry_type),
        name: Some(name),
        value: None,
        comment_count: 0,
        has_absorbed_blanks: false,
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
        let mut block = PendingBlock::comment(1, "# header");
        assert!(!block.is_complete());
        assert!(block.can_absorb_blank());
        assert!(block.can_absorb_comment());
        assert!(block.can_merge_down());

        block.has_absorbed_blanks = true;
        assert!(!block.can_absorb_comment());
        assert!(!block.can_merge_down());
    }

    #[test]
    fn test_can_merge_down_multiple_comments() {
        let mut block = PendingBlock::comment(1, "# c1");
        block.add_line("# c2", 2);
        block.comment_count = 2;
        assert!(!block.can_merge_down());
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
    #[test]
    fn test_into_comment_or_code_entry() {
        let comment_single = PendingBlock::comment(5, "# single");
        let entry = comment_single.into_comment_or_code_entry().unwrap();
        assert_eq!(entry.entry_type, EntryType::Comment);
        assert_eq!(entry.name, "#L5");
        assert_eq!(entry.value, "# single");

        let mut comment_multi = PendingBlock::comment(2, "# line 1");
        comment_multi.add_line("# line 2", 3);
        let entry = comment_multi.into_comment_or_code_entry().unwrap();
        assert_eq!(entry.name, "#L2-L3");

        let code_single = PendingBlock::code(10, "echo hi");
        let entry = code_single.into_comment_or_code_entry().unwrap();
        assert_eq!(entry.entry_type, EntryType::Code);
        assert_eq!(entry.name, "L10");

        let mut code_multi = PendingBlock::code(10, "echo 1");
        code_multi.add_line("echo 2", 12);
        let entry = code_multi.into_comment_or_code_entry().unwrap();
        assert_eq!(entry.name, "L10-L12");

        let mut code_named = PendingBlock::code(15, "class Foo {");
        code_named.name = Some("Foo".to_string());
        let entry = code_named.into_comment_or_code_entry().unwrap();
        assert_eq!(entry.name, "Foo");

        let func = PendingBlock::function("my_fn".to_string(), 1, "my_fn() {", 1);
        assert!(func.into_comment_or_code_entry().is_none());
    }

    #[test]
    fn test_handle_blank_and_comment_line() {
        let mut pending = None;
        let flushed = handle_blank_line(&mut pending, "", 1);
        assert!(flushed.is_none());
        assert!(pending.is_some());

        // Blank absorbs blank
        let flushed = handle_blank_line(&mut pending, "", 2);
        assert!(flushed.is_none());
        assert_eq!(pending.as_ref().unwrap().end_line, 2);

        // Comment after blank flushes blank
        let flushed = handle_comment_line(&mut pending, "# comment", 3);
        assert!(flushed.is_some());
        assert_eq!(
            pending.as_ref().unwrap().entry_hint,
            Some(EntryType::Comment)
        );

        // Comment absorbs comment
        let flushed = handle_comment_line(&mut pending, "# comment 2", 4);
        assert!(flushed.is_none());
        assert_eq!(pending.as_ref().unwrap().comment_count, 2);
    }

    #[test]
    fn test_handle_code_fallback_and_merge() {
        let mut pending = Some(PendingBlock::comment(1, "# single"));
        // Single comment merges down to code
        let flushed = handle_code_fallback(&mut pending, "echo 1", 2);
        assert!(flushed.is_none());
        assert_eq!(pending.as_ref().unwrap().entry_hint, Some(EntryType::Code));
        assert_eq!(pending.as_ref().unwrap().raw_content(), "# single\necho 1");

        // Adjacent code merges into code with blanks
        let flushed = handle_code_fallback(&mut pending, "echo 2", 3);
        assert!(flushed.is_none());
        assert_eq!(
            pending.as_ref().unwrap().raw_content(),
            "# single\necho 1\necho 2"
        );

        // Blank line seals code with blanks
        let flushed = handle_blank_line(&mut pending, "", 4);
        assert!(flushed.is_none());
        assert!(pending.as_ref().unwrap().has_absorbed_blanks);

        // Next code line flushes previous code
        let flushed = handle_code_fallback(&mut pending, "echo 3", 5);
        assert!(flushed.is_some());
        assert_eq!(pending.as_ref().unwrap().raw_content(), "echo 3");
    }

    #[test]
    fn test_commit_structured_and_multiline() {
        let mut result = ParseResult::new();
        let mut pending = Some(PendingBlock::comment(1, "# alias doc"));

        let alias_entry = Entry::new(
            EntryType::Alias,
            "ll".to_string(),
            "alias ll='ls -l'".to_string(),
        )
        .with_line_number(2)
        .with_end_line(2);
        commit_structured_entry(&mut result, &mut pending, alias_entry, |b| {
            b.into_comment_or_code_entry().unwrap()
        });

        // The single comment merged down into the alias
        assert_eq!(result.entries.len(), 0);
        let trailing = pending.take().unwrap();
        assert_eq!(trailing.start_line, 1);
        assert_eq!(trailing.end_line, 2);
        assert_eq!(trailing.raw_content(), "# alias doc\nalias ll='ls -l'");
    }
}
