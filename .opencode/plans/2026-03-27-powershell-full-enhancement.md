# PowerShell Full Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring PowerShell parser to feature parity with Bash: merge logic, pipeline/scriptblock/block-comment recognition, and additional syntax support.

**Architecture:** Extract shared merge helpers into `pending.rs`, then update PowerShell parser to use them while adding new syntax detection. Each task is independently testable.

**Tech Stack:** Rust, regex (lazy_static), anyhow

---

## File Map

| File | Responsibility |
|------|----------------|
| `src/parser/pending.rs` | Add `entry_to_trailing_pending()`, `merge_pending_with_structured()`, `BlockComment` boundary |
| `src/parser/bash/mod.rs` | Replace private methods with calls to `pending.rs` shared functions |
| `src/parser/pwsh/patterns.rs` | Add 7 new regex patterns |
| `src/parser/pwsh/parsers.rs` | Add 6 new detection/parsing functions |
| `src/parser/pwsh/control.rs` | Add `"do"` to `count_control_start()` |
| `src/parser/pwsh/mod.rs` | Main parse loop rewrite for merging + new syntax integration |

---

### Task 1: Extract shared merge functions to `pending.rs`

**Files:**
- Modify: `src/parser/pending.rs` (add 2 functions after line 357)
- Modify: `src/parser/bash/mod.rs` (replace methods at lines 83-125)

- [ ] **Step 1: Add `entry_to_trailing_pending()` to `pending.rs`**

Add `use crate::model::Entry;` to the existing import at line 24 (change `use crate::model::EntryType;` to `use crate::model::{Entry, EntryType};`).

Add these functions after the `PendingBlock` impl block (after line 357, before `#[cfg(test)]`):

```rust
/// Convert a completed Entry into a PendingBlock that absorbs trailing blank lines.
/// Used after structured entries (Alias, EnvVar, Source, Function) complete.
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

/// Merge a pending entry (Comment/Code/blank lines) with a structured entry.
/// Only a single comment with no absorbed blanks merges downward.
/// All other cases flush the pending block as a separate entry.
///
/// Returns `(Option<Entry>, Entry)` — optionally a flushed pending entry, plus the merged result.
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
```

- [ ] **Step 2: Update `bash/mod.rs` to use shared functions**

Remove `BashParser::entry_to_trailing_pending()` method (lines 83-97).

Remove `BashParser::merge_pending_with_structured()` method (lines 99-125).

Add import at top of `bash/mod.rs`:
```rust
use super::pending::{entry_to_trailing_pending, merge_pending_with_structured};
```

Update all call sites — replace `Self::merge_pending_with_structured(pending_entry.take(), entry, self)` with `merge_pending_with_structured(pending_entry.take(), entry, |b| self.build_entry_from_pending(b))`.

Replace `Self::entry_to_trailing_pending(merged)` with `entry_to_trailing_pending(merged)`.

Affected locations:
- Alias Complete (around line 425-431)
- EnvVar Complete (around line 474-481)
- Source Complete (around line 523-530)
- Function single-line (around line 566-572)

- [ ] **Step 3: Run all tests to verify no regressions**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 4: Run clippy and fmt check**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`

- [ ] **Step 5: Commit**

```bash
git add src/parser/pending.rs src/parser/bash/mod.rs
git commit -m "refactor: extract merge_pending_with_structured and entry_to_trailing_pending to pending.rs"
```

---

### Task 2: Add `comment_count` tracking to PowerShell comment absorption

**Files:**
- Modify: `src/parser/pwsh/mod.rs` (line 245)

- [ ] **Step 1: Add `increment_comment_count()` call**

In `pwsh/mod.rs`, comment absorption path (around line 243-246), add `pending.increment_comment_count();` after `pending.add_line(line, line_number);`.

- [ ] **Step 2: Commit**

```bash
git add src/parser/pwsh/mod.rs
git commit -m "fix(pwsh): add missing increment_comment_count call in comment absorption"
```

---

### Task 3: Port Bash merging logic to PowerShell parser

**Files:**
- Modify: `src/parser/pwsh/mod.rs`

- [ ] **Step 1: Add import for shared merge functions**

```rust
use super::pending::{entry_to_trailing_pending, merge_pending_with_structured};
```

- [ ] **Step 2: Fix blank line absorption to seal CodeWithBlanks**

Change blank handling (lines 218-222) to match Bash — seal both Comment and CodeWithBlanks:

```rust
if matches!(
    pending.merge_type(),
    Some(MergeType::CodeWithBlanks) | Some(MergeType::Comment)
) {
    pending.has_absorbed_blanks = true;
}
```

- [ ] **Step 3: Update Alias Complete to use merge_pending_with_structured**

```rust
ParseEvent::Complete(entry) => {
    let (pending_entry_to_add, merged) =
        merge_pending_with_structured(pending_entry.take(), entry, |b| self.build_entry_from_pending(b));
    if let Some(pending_e) = pending_entry_to_add {
        result.add_entry(pending_e);
    }
    pending_entry = Some(entry_to_trailing_pending(merged));
    continue;
}
```

- [ ] **Step 4: Update EnvVar Complete with same pattern**

Same as Step 3.

- [ ] **Step 5: Update EnvVar Started with merge-down for preceding comment**

```rust
ParseEvent::Started { entry_type, name, boundary, first_line } => {
    let (merged_first_line, start_line) =
        if let Some(pending) = pending_entry.take() {
            if pending.can_merge_down() {
                let merged = format!("{}\n{}", pending.raw_content(), first_line);
                (merged, pending.start_line)
            } else {
                result.add_entry(self.build_entry_from_pending(pending));
                (first_line.to_string(), line_number)
            }
        } else {
            (first_line.to_string(), line_number)
        };
    active_block = Some(PendingBlock {
        lines: vec![merged_first_line],
        start_line, end_line: line_number, boundary,
        entry_hint: Some(entry_type), name: Some(name),
        value: None, comment_count: 0, has_absorbed_blanks: false,
    });
    continue;
}
```

- [ ] **Step 6: Update Source Complete with same merge pattern**

Same as Step 3.

- [ ] **Step 7: Update Function detection with merge + trailing blank absorption**

Single-line: use `merge_pending_with_structured` + `entry_to_trailing_pending`.
Multi-line: add merge-down logic for preceding comments, then create BraceCounting block.

- [ ] **Step 8: Update active_block BraceCounting completion**

Change from `result.add_entry(entry)` to `pending_entry = Some(entry_to_trailing_pending(entry))`.

- [ ] **Step 9: Update Here-String (QuoteCounting) completion**

Change from `result.add_entry(entry)` to `pending_entry = Some(entry_to_trailing_pending(entry))`.

- [ ] **Step 10: Add adjacent Code line merging in fallback path**

```rust
Some(pending) if pending.entry_hint == Some(EntryType::Code) => {
    if pending.merge_type() == Some(MergeType::CodeWithBlanks)
        && !pending.has_absorbed_blanks
    {
        pending.add_line(line, line_number);
    } else {
        if let Some(entry) = self.flush_pending_comment_code(&mut pending_entry) {
            result.add_entry(entry);
        }
        pending_entry = Some(PendingBlock::code(line_number, line));
    }
}
```

- [ ] **Step 11: Write merging tests**

```rust
#[test]
fn test_single_comment_merges_into_alias() {
    let parser = PowerShellParser::new();
    let content = "# My alias\nSet-Alias ll Get-ChildItem";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry_type, EntryType::Alias);
    assert_eq!(result.entries[0].name, "ll");
    assert_eq!(result.entries[0].line_number, Some(1));
    assert_eq!(result.entries[0].end_line, Some(2));
}

#[test]
fn test_multi_comment_does_not_merge_into_alias() {
    let parser = PowerShellParser::new();
    let content = "# Header\n# Description\nSet-Alias ll Get-ChildItem";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].entry_type, EntryType::Comment);
    assert_eq!(result.entries[1].entry_type, EntryType::Alias);
}

#[test]
fn test_alias_absorbs_trailing_blanks() {
    let parser = PowerShellParser::new();
    let content = "Set-Alias ll Get-ChildItem\n\n\nSet-Alias gs git";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].end_line, Some(3));
}

#[test]
fn test_adjacent_code_lines_merge() {
    let parser = PowerShellParser::new();
    let content = "Write-Host 'a'\nWrite-Host 'b'\nWrite-Host 'c'";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].end_line, Some(3));
}

#[test]
fn test_code_with_blank_gap_splits() {
    let parser = PowerShellParser::new();
    let content = "Write-Host 'a'\n\nWrite-Host 'b'";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 2);
}

#[test]
fn test_function_absorbs_trailing_blanks() {
    let parser = PowerShellParser::new();
    let content = "function Get-Name {\n    'test'\n}\n\nSet-Alias ll Get-ChildItem";
    let result = parser.parse(content);
    let funcs: Vec<_> = result.entries.iter().filter(|e| e.entry_type == EntryType::Function).collect();
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].end_line, Some(5));
}

#[test]
fn test_single_comment_merges_into_function() {
    let parser = PowerShellParser::new();
    let content = "# Helper\nfunction Get-Name {\n    'test'\n}";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry_type, EntryType::Function);
    assert!(result.entries[0].value.starts_with("# Helper"));
}
```

- [ ] **Step 12: Update affected existing tests**

`test_comment_then_alias_not_merged` — rename and update: single comment NOW merges into alias, so expect 1 entry.

`test_comment_blank_alias_scenario` — multiple comments + blank should still produce 2 entries.

`test_blank_lines_grouped` — blanks absorbed by alias, so no separate blank entries. Assert still passes.

- [ ] **Step 13: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 14: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`

- [ ] **Step 15: Commit**

```bash
git add src/parser/pwsh/mod.rs
git commit -m "feat(pwsh): port Bash merging logic — comment merge-down, trailing blank absorption, adjacent code merge"
```

---

### Task 4: Add pipeline/scriptblock detection patterns

**Files:**
- Modify: `src/parser/pwsh/patterns.rs`
- Modify: `src/parser/pwsh/parsers.rs`

- [ ] **Step 1: Add regex patterns to `patterns.rs`**

```rust
pub static ref PIPELINE_BLOCK_RE: Regex = Regex::new(
    r#"(\w[\w-]*)\s*\{$"#
).unwrap();

pub static ref SCRIPTBLOCK_ASSIGN_RE: Regex = Regex::new(
    r#"(\$\w[\w:]*)\s*=\s*\{$"#
).unwrap();
```

- [ ] **Step 2: Add detection function to `parsers.rs`**

```rust
pub fn detect_scriptblock_start(line: &str) -> Option<String> {
    if let Some(caps) = SCRIPTBLOCK_ASSIGN_RE.captures(line) {
        let (open, close) = count_braces_outside_quotes(line);
        if open > close {
            return Some(caps[1].to_string());
        }
    }
    if line.contains('|') {
        if let Some(caps) = PIPELINE_BLOCK_RE.captures(line) {
            let (open, close) = count_braces_outside_quotes(line);
            if open > close {
                return Some(caps[1].to_string());
            }
        }
    }
    None
}
```

- [ ] **Step 3: Write pattern and detection tests, run them**

Run: `cargo test --lib patterns && cargo test --lib parsers`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/parser/pwsh/patterns.rs src/parser/pwsh/parsers.rs
git commit -m "feat(pwsh): add pipeline and scriptblock detection patterns"
```

---

### Task 5: Integrate pipeline/scriptblock into parse loop

**Files:**
- Modify: `src/parser/pwsh/mod.rs`

- [ ] **Step 1: Add import**

```rust
use super::parsers::{
    detect_function_start, detect_scriptblock_start, is_heredoc_end, try_parse_alias,
    try_parse_env, try_parse_source,
};
```

- [ ] **Step 2: Add scriptblock detection before fallback Code**

Insert after function detection, before fallback Code section:

```rust
if let Some(block_name) = detect_scriptblock_start(trimmed) {
    let (open, close) = count_braces_outside_quotes(trimmed);
    let brace_count = (open as i32).saturating_sub(close as i32);
    let is_single_line = brace_count == 0 && trimmed.contains('}');

    if is_single_line {
        let entry = Entry::new(EntryType::Code, block_name, line.to_string())
            .with_line_number(line_number)
            .with_end_line(line_number);
        let (pending_entry_to_add, merged) =
            merge_pending_with_structured(pending_entry.take(), entry, |b| self.build_entry_from_pending(b));
        if let Some(pending_e) = pending_entry_to_add {
            result.add_entry(pending_e);
        }
        pending_entry = Some(entry_to_trailing_pending(merged));
    } else {
        let (merged_first_line, start_line) =
            if let Some(pending) = pending_entry.take() {
                if matches!(
                    pending.entry_hint,
                    Some(EntryType::Comment) | Some(EntryType::Code)
                ) && !pending.has_absorbed_blanks
                {
                    let mut lines = pending.lines;
                    lines.push(line.to_string());
                    (lines, pending.start_line)
                } else {
                    result.add_entry(self.build_entry_from_pending(pending));
                    (vec![line.to_string()], line_number)
                }
            } else {
                (vec![line.to_string()], line_number)
            };
        active_block = Some(PendingBlock {
            lines: merged_first_line, start_line, end_line: line_number,
            boundary: BoundaryType::BraceCounting { brace_count },
            entry_hint: Some(EntryType::Code), name: Some(block_name),
            value: None, comment_count: 0, has_absorbed_blanks: false,
        });
    }
    continue;
}
```

- [ ] **Step 3: Write tests**

```rust
#[test]
fn test_pipeline_multiline_block() {
    let parser = PowerShellParser::new();
    let content = "1..9 | ForEach-Object {\n    $num = $_\n    Write-Host $num\n}";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].name, "ForEach-Object");
    assert_eq!(result.entries[0].line_number, Some(1));
    assert_eq!(result.entries[0].end_line, Some(4));
}

#[test]
fn test_pipeline_block_with_internal_blanks() {
    let parser = PowerShellParser::new();
    let content = "1..9 | ForEach-Object {\n    $num = $_\n\n    Write-Host $num\n}";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert!(result.entries[0].value.contains("\n\n"));
}

#[test]
fn test_pipeline_with_preceding_comment() {
    let parser = PowerShellParser::new();
    let content = "# Build numbers\n1..9 | ForEach-Object {\n    $_\n}";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].line_number, Some(1));
    assert!(result.entries[0].value.starts_with("# Build numbers"));
}

#[test]
fn test_pipeline_absorbs_trailing_blanks() {
    let parser = PowerShellParser::new();
    let content = "1..9 | ForEach-Object {\n    $_\n}\n\nWrite-Host 'done'";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].end_line, Some(5));
}

#[test]
fn test_scriptblock_assign_multiline() {
    let parser = PowerShellParser::new();
    let content = "$block = {\n    Write-Host 'hi'\n}";
    let result = parser.parse(content);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].name, "$block");
}
```

- [ ] **Step 4: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`

- [ ] **Step 5: Commit**

```bash
git add src/parser/pwsh/mod.rs
git commit -m "feat(pwsh): integrate pipeline/scriptblock detection into parse loop"
```

---

### Task 6: Add block comment `<# ... #>` support

**Files:**
- Modify: `src/parser/pending.rs` (add `BlockComment` variant)
- Modify: `src/parser/pwsh/patterns.rs` (add 2 regexes)
- Modify: `src/parser/pwsh/parsers.rs` (add 2 detection functions)
- Modify: `src/parser/pwsh/mod.rs` (add handling in parse loop)

- [ ] **Step 1: Add `BlockComment` to `BoundaryType` in `pending.rs`**

```rust
/// Block comment <# ... #> — accumulates lines until #> is found.
BlockComment,
```

- [ ] **Step 2: Add regex patterns to `patterns.rs`**

```rust
pub static ref BLOCK_COMMENT_START_RE: Regex = Regex::new(r#"^<#\s*$"#).unwrap();
pub static ref BLOCK_COMMENT_END_RE: Regex = Regex::new(r#"^\s*#>\s*$"#).unwrap();
```

- [ ] **Step 3: Add detection functions to `parsers.rs`**

```rust
pub fn is_block_comment_start(line: &str) -> bool {
    BLOCK_COMMENT_START_RE.is_match(line)
}

pub fn is_block_comment_end(line: &str) -> bool {
    BLOCK_COMMENT_END_RE.is_match(line)
}
```

- [ ] **Step 4: Add handling in parse loop**

Before standalone comment check, add block comment start detection. In active_block match, add BlockComment arm that checks for `#>` and converts to trailing pending.

- [ ] **Step 5: Write tests**

```rust
#[test]
fn test_block_comment_single() {
    let parser = PowerShellParser::new();
    let content = "<#\nA comment\n#>";
    let result = parser.parse(content);
    let comments: Vec<_> = result.entries.iter().filter(|e| e.entry_type == EntryType::Comment).collect();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].line_number, Some(1));
    assert_eq!(comments[0].end_line, Some(3));
}

#[test]
fn test_block_comment_does_not_merge_with_hash_comment() {
    let parser = PowerShellParser::new();
    let content = "<#\nBlock\n#>\n# Regular\nWrite-Host 'hi'";
    let result = parser.parse(content);
    let comments: Vec<_> = result.entries.iter().filter(|e| e.entry_type == EntryType::Comment).collect();
    assert_eq!(comments.len(), 1);
    assert!(comments[0].value.contains("<#"));
}
```

- [ ] **Step 6: Run clippy and fmt, commit**

```bash
git add src/parser/pending.rs src/parser/pwsh/patterns.rs src/parser/pwsh/parsers.rs src/parser/pwsh/mod.rs
git commit -m "feat(pwsh): add block comment <# ... #> support"
```

---

### Task 7: Add `do { }`, `class`, `enum`, `Import-Module`

**Files:**
- Modify: `src/parser/pwsh/control.rs` (add `do`)
- Modify: `src/parser/pwsh/patterns.rs` (add 3 regexes)
- Modify: `src/parser/pwsh/parsers.rs` (add 3 functions)
- Modify: `src/parser/pwsh/mod.rs` (integrate)

- [ ] **Step 1: Add `do` to `count_control_start()` in `control.rs`**

```rust
|| part.starts_with("do ")
|| part.starts_with("do{")
|| part == "do"
```

- [ ] **Step 2: Add regex patterns to `patterns.rs`**

```rust
pub static ref CLASS_RE: Regex = Regex::new(r#"^class\s+(\w[\w-]*)"#).unwrap();
pub static ref ENUM_RE: Regex = Regex::new(r#"^enum\s+(\w[\w-]*)\s*\{"#).unwrap();
pub static ref IMPORT_MODULE_RE: Regex = Regex::new(r#"^(?:Import-Module|ipmo)\s+(.+)$"#).unwrap();
```

- [ ] **Step 3: Add detection/parsing functions to `parsers.rs`**

`detect_class_start()`, `detect_enum_start()`, `try_parse_import_module()`.

- [ ] **Step 4: Integrate into parse loop**

Import-Module: after `try_parse_source`, same merge pattern.
Class/Enum: after function detection, before scriptblock, using BraceCounting with `entry_hint = Code` and `name = class/enum name`.

- [ ] **Step 5: Write tests**

```rust
#[test]
fn test_do_while_control_structure() { ... }
#[test]
fn test_class_detection() { ... }
#[test]
fn test_enum_detection() { ... }
#[test]
fn test_import_module_as_source() { ... }
```

- [ ] **Step 6: Run clippy and fmt, commit**

```bash
git add src/parser/pwsh/control.rs src/parser/pwsh/patterns.rs src/parser/pwsh/parsers.rs src/parser/pwsh/mod.rs
git commit -m "feat(pwsh): add do/while, class, enum, Import-Module syntax support"
```

---

### Task 8: Add single-quoted Here-String `@' ... '@`

**Files:**
- Modify: `src/parser/pwsh/patterns.rs` (add regex)
- Modify: `src/parser/pwsh/parsers.rs` (update `try_parse_env` + `is_heredoc_end`)

- [ ] **Step 1: Add `ENV_HEREDOC_START_SINGLE_RE` pattern**

```rust
pub static ref ENV_HEREDOC_START_SINGLE_RE: Regex = Regex::new(
    r#"^\$env:(\w+)\s*=\s*@'$"#
).unwrap();
```

- [ ] **Step 2: Update `try_parse_env` to detect `@'`**

Add check before existing `@"` check.

- [ ] **Step 3: Update `is_heredoc_end` to detect `'@`**

```rust
pub fn is_heredoc_end(line: &str) -> bool {
    line == r#""@"# || line == "'@"
}
```

- [ ] **Step 4: Write test, run clippy and fmt, commit**

```bash
git add src/parser/pwsh/patterns.rs src/parser/pwsh/parsers.rs
git commit -m "feat(pwsh): add single-quoted Here-String @' ... '@ support"
```

---

### Task 9: Full regression test

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 2: Run clippy and fmt**

Run: `cargo clippy -- -D warnings && cargo fmt -- --check`

- [ ] **Step 3: Verify Bash parser unaffected**

Run: `cargo test --lib bash`
Expected: ALL PASS

- [ ] **Step 4: Run integration tests**

Run: `cargo test --test pwsh_heredoc_integration && cargo test --test integration`

- [ ] **Step 5: Final commit if any fixes needed**
