# TUI Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 11 TUI enhancements: key rebinding, copy/remark operations, parser improvements (code merging, multi-parse edit, line recalc), config management (add/remove files), startup validation, and Windows $PROFILE fix.

**Architecture:** Infrastructure-first approach — build `recalculate_line_numbers()` and `replace_entry_with_parsed()` utilities, then layer features in dependency order. Each feature commits independently. Parser changes (code merging) are isolated from TUI changes.

**Tech Stack:** Rust, ratatui (TUI), crossterm (terminal), dialoguer (prompts), toml (config), regex (patterns)

**Spec:** `docs/superpowers/specs/2026-03-26-tui-enhancements-design.md`

---

## File Structure

| File | Role | Changes |
|------|------|---------|
| `src/model/profile.rs` | Data model | Add `writable` field to ProfileFile, add `recalculate_line_numbers()` method |
| `src/tui/keys.rs` | Key bindings | Remap keys, add new Action variants (Copy, Remark, AddFile, TextInput*) |
| `src/tui/state.rs` | App state types | Add TextInputState, InputPurpose, new AppMode variants |
| `src/tui/app.rs` | Main app logic | Add config/shell_key fields, handlers for Copy/Remark/AddFile/TextInput, file check |
| `src/tui/operations.rs` | Entry operations | Add `replace_entry_with_parsed()`, `comment_value()`, `uncomment_value()` |
| `src/tui/ui.rs` | UI rendering | Update help screen, add greyed-out styling, add text input bar |
| `src/parser/pending.rs` | Parser state | Add `has_absorbed_blanks` field |
| `src/parser/bash/mod.rs` | Bash parser | Adjacent code merging logic |
| `src/config/path_resolver.rs` | Path expansion | $PROFILE PowerShell fallback |
| `src/utils/path.rs` | Path utilities | Add `check_writable()` |
| `src/main.rs` | Entry point | Add startup file check with dialoguer prompts |
| `CLAUDE.md` | Documentation | Update key bindings table |
| `tests/tui_logic_tests.rs` | TUI tests | Tests for new operations |
| `tests/config_tests.rs` | Config tests | Tests for $PROFILE expansion |

---

## Task 1: Infrastructure — Line Number Recalculation (#6)

**Files:**
- Modify: `src/model/profile.rs:14-21` (ProfileFile struct)
- Test: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write failing tests for recalculate_line_numbers**

In `tests/tui_logic_tests.rs`, add:

```rust
#[test]
fn test_recalculate_line_numbers_single_line_entries() {
    let mut profile = make_test_profile();
    // File 0 has entries: "alias ll='ls -la'" (1 line), "export PATH='/bin'" (1 line)
    profile.files[0].recalculate_line_numbers();

    assert_eq!(profile.files[0].entries[0].line_number, Some(1));
    assert_eq!(profile.files[0].entries[0].end_line, Some(1));
    assert_eq!(profile.files[0].entries[1].line_number, Some(2));
    assert_eq!(profile.files[0].entries[1].end_line, Some(2));
}

#[test]
fn test_recalculate_line_numbers_multiline_entries() {
    let mut file = ProfileFile::new(PathBuf::from("/tmp/test"), true);
    file.entries = vec![
        make_test_entry("foo", "foo() {\n  echo hi\n}", EntryType::Function, 0),
        make_test_entry("bar", "alias bar='baz'", EntryType::Alias, 0),
    ];
    file.recalculate_line_numbers();

    assert_eq!(file.entries[0].line_number, Some(1));
    assert_eq!(file.entries[0].end_line, Some(3)); // 3 lines: foo() {\n  echo hi\n}
    assert_eq!(file.entries[1].line_number, Some(4));
    assert_eq!(file.entries[1].end_line, Some(4));
}

#[test]
fn test_recalculate_updates_code_comment_names() {
    let mut file = ProfileFile::new(PathBuf::from("/tmp/test"), true);
    file.entries = vec![
        make_test_entry("alias1", "alias a='b'", EntryType::Alias, 0),
        {
            let mut e = make_test_entry("L99", "echo hello", EntryType::Code, 0);
            e.line_number = Some(99); // stale
            e
        },
        {
            let mut e = make_test_entry("#L99-L100", "# comment\n# more", EntryType::Comment, 0);
            e.line_number = Some(99); // stale
            e
        },
    ];
    file.recalculate_line_numbers();

    assert_eq!(file.entries[1].name, "L2");
    assert_eq!(file.entries[1].line_number, Some(2));
    assert_eq!(file.entries[2].name, "#L3-L4");
    assert_eq!(file.entries[2].line_number, Some(3));
    assert_eq!(file.entries[2].end_line, Some(4));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_recalculate -- --nocapture 2>&1 | head -30`
Expected: Compilation errors — `recalculate_line_numbers` method doesn't exist.

- [ ] **Step 3: Implement recalculate_line_numbers()**

In `src/model/profile.rs`, add method to `impl ProfileFile`:

```rust
/// Recalculate line_number, end_line, and name for all entries.
/// Call after any mutation (add, delete, move, paste, edit, remark).
/// Entry.value uses separator format: N lines = N-1 '\n'.
/// When written to file, each value gets a '\n' terminator, so next entry
/// starts at (previous end_line + 1).
pub fn recalculate_line_numbers(&mut self) {
    let mut current_line = 1usize;
    for entry in &mut self.entries {
        let line_count = entry.value.split('\n').count();
        entry.line_number = Some(current_line);
        let end = current_line + line_count - 1;
        entry.end_line = if end > current_line {
            Some(end)
        } else {
            entry.line_number
        };

        match entry.entry_type {
            crate::model::EntryType::Comment => {
                entry.name = if end > current_line {
                    format!("#L{}-L{}", current_line, end)
                } else {
                    format!("#L{}", current_line)
                };
            }
            crate::model::EntryType::Code => {
                entry.name = if end > current_line {
                    format!("L{}-L{}", current_line, end)
                } else {
                    format!("L{}", current_line)
                };
            }
            _ => {} // Alias, Function, EnvVar, Source keep parsed names
        }
        current_line = end + 1;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_recalculate -- --nocapture`
Expected: All 3 tests PASS.

- [ ] **Step 5: Wire recalculate calls into existing operations**

In `src/tui/operations.rs`, add calls after mutations:

In `delete_entries()`, after the removal loop (before returning):
```rust
for fi in by_file.keys() {
    profile.files[*fi].recalculate_line_numbers();
}
```

In `paste_entries()`, at the end:
```rust
profile.files[fi].recalculate_line_numbers();
```

In `save_dirty_files()`, before writing:
```rust
file.recalculate_line_numbers();
```

In `src/tui/app.rs`, in `execute_move()`, after insertion (before `self.rebuild_list()`):
```rust
// Recalculate line numbers for affected files
let mut affected_files: std::collections::HashSet<usize> = by_file.keys().cloned().collect();
affected_files.insert(target_fi);
for fi in affected_files {
    if fi < self.profile.files.len() {
        self.profile.files[fi].recalculate_line_numbers();
    }
}
```

In `run_edit_entry()`, after updating entry:
```rust
self.profile.files[fi].recalculate_line_numbers();
```

In `run_add_entry()`, after inserting entries:
```rust
self.profile.files[fi].recalculate_line_numbers();
```

- [ ] **Step 6: Run full test suite**

Run: `cargo test 2>&1 | tail -5`
Expected: All tests pass. Some parser tests may need updating if they assert specific line numbers — check and fix.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: add line number auto-recalculation after mutations (#6)

ProfileFile::recalculate_line_numbers() updates line_number, end_line,
and Code/Comment entry names after any entry mutation. Wired into
delete, paste, move, edit, and add operations.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 2: Infrastructure — replace_entry_with_parsed (#7 prep)

**Files:**
- Modify: `src/tui/operations.rs`
- Test: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write failing test**

In `tests/tui_logic_tests.rs`:

```rust
#[test]
fn test_replace_entry_with_parsed_multiple() {
    use wenv::tui::operations::replace_entry_with_parsed;

    let mut file = ProfileFile::new(PathBuf::from("/tmp/test"), true);
    file.entries = vec![
        make_test_entry("a", "alias a='1'", EntryType::Alias, 0),
        make_test_entry("b", "alias b='2'", EntryType::Alias, 0),
        make_test_entry("c", "alias c='3'", EntryType::Alias, 0),
    ];

    let replacements = vec![
        make_test_entry("x", "alias x='10'", EntryType::Alias, 0),
        make_test_entry("y", "alias y='20'", EntryType::Alias, 0),
    ];

    let count = replace_entry_with_parsed(&mut file, 1, replacements, 0);

    assert_eq!(count, 2);
    assert_eq!(file.entries.len(), 4); // was 3, removed 1, added 2
    assert_eq!(file.entries[0].name, "a");
    assert_eq!(file.entries[1].name, "x");
    assert_eq!(file.entries[2].name, "y");
    assert_eq!(file.entries[3].name, "c");
    assert!(file.dirty);
}

#[test]
fn test_replace_entry_with_empty_deletes() {
    use wenv::tui::operations::replace_entry_with_parsed;

    let mut file = ProfileFile::new(PathBuf::from("/tmp/test"), true);
    file.entries = vec![
        make_test_entry("a", "alias a='1'", EntryType::Alias, 0),
        make_test_entry("b", "alias b='2'", EntryType::Alias, 0),
    ];

    let count = replace_entry_with_parsed(&mut file, 0, vec![], 0);

    assert_eq!(count, 0);
    assert_eq!(file.entries.len(), 1);
    assert_eq!(file.entries[0].name, "b");
    assert!(file.dirty);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_replace_entry -- --nocapture 2>&1 | head -20`
Expected: Compilation error — function doesn't exist.

- [ ] **Step 3: Implement replace_entry_with_parsed**

In `src/tui/operations.rs`:

```rust
use crate::model::profile::ProfileFile;

/// Replace a single entry with zero or more parsed entries at the same position.
/// Returns the number of new entries inserted.
/// If new_entries is empty, the original entry is deleted.
pub fn replace_entry_with_parsed(
    file: &mut ProfileFile,
    entry_index: usize,
    new_entries: Vec<Entry>,
    file_index: usize,
) -> usize {
    file.entries.remove(entry_index);

    let count = new_entries.len();
    for (i, mut entry) in new_entries.into_iter().enumerate() {
        entry.file_index = file_index;
        file.entries.insert(entry_index + i, entry);
    }

    file.dirty = true;
    file.recalculate_line_numbers();
    count
}
```

Ensure `ProfileFile` is imported at the top of `operations.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_replace_entry -- --nocapture`
Expected: Both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add replace_entry_with_parsed utility (#7 prep)

Generic function to replace one entry with zero or more parsed entries
at the same position. Used by edit-entry and remark-toggle features.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 3: Key Rebinding (#2) + Copy Operation (#3)

**Files:**
- Modify: `src/tui/keys.rs:6-87` (Action enum + map_normal_key)
- Modify: `src/tui/app.rs:265-280` (add Copy handler)
- Modify: `src/tui/ui.rs:491-537` (help screen)
- Test: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write failing test for copy operation**

In `tests/tui_logic_tests.rs`:

```rust
#[test]
fn test_copy_entries() {
    let mut profile = make_test_profile();
    let items = profile.build_visible_list();

    // Find an entry index (first Entry in visible_items)
    let entry_idx = items.iter().position(|item| matches!(item, ListItem::Entry(_, _))).unwrap();

    // Copy the entry
    let copied: Vec<Entry> = vec![entry_idx]
        .iter()
        .filter_map(|&idx| match items.get(idx) {
            Some(ListItem::Entry(fi, ei)) => Some(profile.files[*fi].entries[*ei].clone()),
            _ => None,
        })
        .collect();

    assert_eq!(copied.len(), 1);
    // Original entries should be unchanged (not deleted like cut)
    let original_count: usize = profile.files.iter().map(|f| f.entries.len()).sum();
    assert!(original_count > 0);
    // No file should be dirty after copy
    assert!(!profile.files.iter().any(|f| f.dirty));
}
```

- [ ] **Step 2: Run test to verify it passes (copy is just clone, no new function needed)**

Run: `cargo test test_copy_entries -- --nocapture`
Expected: PASS (copy logic is inline in app.rs, test validates the pattern).

- [ ] **Step 3: Update Action enum**

In `src/tui/keys.rs`, add new variants to `Action`:

```rust
pub enum Action {
    NavigateUp,
    NavigateDown,
    PageUp,
    PageDown,
    Home,
    End,
    ToggleExpand,
    CollapseAll,
    ExpandAll,
    Edit,
    Add,
    Delete,
    ToggleSelect,
    RangeSelectUp,
    RangeSelectDown,
    Cut,
    Copy,           // NEW
    Paste,
    StartMove,
    Search,
    Undo,
    Remark,         // NEW
    AddFile,        // NEW
    Help,
    Save,
    Quit,
    Confirm,
    Cancel,
    SearchInput(char),
    SearchBackspace,
    TextInputChar(char),    // NEW
    TextInputBackspace,     // NEW
    TextInputLeft,          // NEW
    TextInputRight,         // NEW
    Noop,
}
```

- [ ] **Step 4: Update map_normal_key() with new bindings**

In `src/tui/keys.rs`, replace the key mapping block in `map_normal_key()`:

```rust
fn map_normal_key(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Up => return Action::RangeSelectUp,
            KeyCode::Down => return Action::RangeSelectDown,
            _ => {}
        }
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char('s') = key.code {
            return Action::Save;
        }
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::NavigateUp,
        KeyCode::Down | KeyCode::Char('j') => Action::NavigateDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Char('0') => Action::CollapseAll,
        KeyCode::Char('9') => Action::ExpandAll,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('n') => Action::Add,        // was 'a'
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('s') => Action::ToggleSelect,
        KeyCode::Char('x') => Action::Cut,
        KeyCode::Char('c') => Action::Copy,        // NEW
        KeyCode::Char('v') => Action::Paste,       // was 'p'
        KeyCode::Char('m') => Action::StartMove,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('z') => Action::Undo,        // was 'u'
        KeyCode::Char('r') => Action::Remark,      // NEW
        KeyCode::Char('a') => Action::AddFile,     // NEW (repurposed)
        KeyCode::Char('?') => Action::Help,
        KeyCode::Char('w') => Action::Save,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Esc => Action::Cancel,
        _ => Action::Noop,
    }
}
```

- [ ] **Step 5: Add map_text_input_key() function**

In `src/tui/keys.rs`, add after `map_popup_key()`:

```rust
fn map_text_input_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::TextInputBackspace,
        KeyCode::Left => Action::TextInputLeft,
        KeyCode::Right => Action::TextInputRight,
        KeyCode::Char(c) => Action::TextInputChar(c),
        _ => Action::Noop,
    }
}
```

Update `map_key()` to route `AppMode::TextInput`:

```rust
pub fn map_key(mode: &AppMode, key: KeyEvent) -> Action {
    match mode {
        AppMode::Normal => map_normal_key(key),
        AppMode::Moving => map_moving_key(key),
        AppMode::Searching => map_search_key(key),
        AppMode::ShowingDetail => map_detail_key(key),
        AppMode::TextInput => map_text_input_key(key),
        _ => map_popup_key(key),
    }
}
```

- [ ] **Step 6: Add Copy handler in app.rs**

In `src/tui/app.rs`, in `handle_action()`, after the `Action::Cut` block, add:

```rust
Action::Copy => {
    let targets = self.get_operation_targets();
    if !targets.is_empty() {
        let copied: Vec<crate::model::Entry> = targets
            .iter()
            .filter_map(|&idx| match self.visible_items.get(idx) {
                Some(ListItem::Entry(fi, ei)) => {
                    Some(self.profile.files[*fi].entries[*ei].clone())
                }
                _ => None,
            })
            .collect();
        let count = copied.len();
        self.clipboard.entries = copied;
        self.message = Some(format!("Copied {} entries", count));
    }
}
```

Also add placeholder handlers for the new actions that will be implemented later:

```rust
Action::Remark => {
    self.message = Some("Remark: not yet implemented".into());
}
Action::AddFile => {
    self.message = Some("Add file: not yet implemented".into());
}
Action::TextInputChar(_) | Action::TextInputBackspace | Action::TextInputLeft | Action::TextInputRight => {}
```

- [ ] **Step 7: Update help screen in ui.rs**

In `src/tui/ui.rs`, update the help popup text to reflect all new key bindings. Find the `draw_help_popup` function and update the editing section:

Replace the editing section entries:
- `e` → Edit entry/file (unchanged)
- `n` → New entry (was `a`)
- `d` → Delete entry / Remove file
- `x` → Cut entries
- `c` → Copy entries (NEW)
- `v` → Paste entries (was `p`)
- `m` → Move entries (unchanged)
- `z` → Undo (was `u`)
- `r` → Toggle remark (NEW)
- `a` → Add file path (NEW)

- [ ] **Step 8: Build and test**

Run: `cargo build 2>&1 | tail -10 && cargo test 2>&1 | tail -5`
Expected: Builds with no errors. All tests pass.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: rebind keys and add copy operation (#2, #3)

Key changes: a→n (new entry), p→v (paste), u→z (undo).
New keys: c (copy), r (remark placeholder), a (add file placeholder).
Removed: r (refresh from disk) — replaced by r (remark).
Copy clones selected entries to clipboard without deleting originals.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 4: New Entry at File Beginning (#10)

**Files:**
- Modify: `src/tui/app.rs:785-787` (run_add_entry insert_pos)
- Test: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write failing test**

In `tests/tui_logic_tests.rs`:

```rust
#[test]
fn test_paste_at_file_header_inserts_at_beginning() {
    use wenv::tui::operations::paste_entries;

    let mut profile = make_test_profile();
    let items = profile.build_visible_list();

    // Find first FileHeader index
    let header_idx = items
        .iter()
        .position(|item| matches!(item, ListItem::FileHeader(_)))
        .unwrap();

    let new_entry = make_test_entry("new", "alias new='cmd'", EntryType::Alias, 0);
    paste_entries(&mut profile, &items, header_idx, &[new_entry]);

    // The new entry should be at position 0 in the file
    assert_eq!(profile.files[0].entries[0].name, "new");
}
```

- [ ] **Step 2: Run test — verify it already passes (paste_entries already inserts at 0 for FileHeader)**

Run: `cargo test test_paste_at_file_header -- --nocapture`
Expected: PASS — `paste_entries()` already has `Some(ListItem::FileHeader(fi)) => (*fi, 0)`.

- [ ] **Step 3: Fix run_add_entry() to insert at beginning on FileHeader**

In `src/tui/app.rs`, in `run_add_entry()`, change the insert_pos logic:

```rust
// Before:
let insert_pos = match self.visible_items.get(self.cursor) {
    Some(ListItem::Entry(_, ei)) => ei + 1,
    _ => self.profile.files[fi].entries.len(),
};

// After:
let insert_pos = match self.visible_items.get(self.cursor) {
    Some(ListItem::Entry(_, ei)) => ei + 1,
    Some(ListItem::FileHeader(_)) => 0,  // Insert at beginning
    _ => self.profile.files[fi].entries.len(),
};
```

- [ ] **Step 4: Build and test**

Run: `cargo build 2>&1 | tail -5 && cargo test 2>&1 | tail -5`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: insert new entry at file beginning when on FileHeader (#10)

When pressing 'n' (new) on a FileHeader, the new entry is inserted
at position 0 (beginning) instead of appended to end.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 5: Windows $PROFILE Expansion (#11)

**Files:**
- Modify: `src/config/path_resolver.rs:14-24`
- Test: `tests/config_tests.rs`

- [ ] **Step 1: Write test for query_powershell_profile fallback**

In `tests/config_tests.rs`:

```rust
#[test]
fn test_expand_env_vars_unknown_var_unchanged() {
    // When an env var is not set and has no special handling, it stays as-is
    std::env::remove_var("NONEXISTENT_VAR_XYZ");
    let result = wenv::config::path_resolver::expand_env_vars("$NONEXISTENT_VAR_XYZ/foo");
    // Should remain unchanged since var doesn't exist
    assert_eq!(result, "$NONEXISTENT_VAR_XYZ/foo");
}
```

- [ ] **Step 2: Run test to verify current behavior**

Run: `cargo test test_expand_env_vars_unknown -- --nocapture`
Expected: May pass or fail depending on current behavior. This establishes baseline.

- [ ] **Step 3: Implement query_powershell_profile() and update expand_env_vars()**

In `src/config/path_resolver.rs`:

```rust
pub fn expand_env_vars(path: &str) -> String {
    let mut result = path.to_string();
    let re = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in re.captures_iter(path) {
        let var_name = &cap[1];
        if let Ok(val) = std::env::var(var_name) {
            result = result.replace(&cap[0], &val);
        } else if var_name == "PROFILE" {
            if let Some(val) = query_powershell_profile() {
                result = result.replace(&cap[0], &val);
            }
        }
    }
    result
}

/// Query PowerShell for the $PROFILE path when not available as env var.
/// Tries `pwsh` first (cross-platform), then `powershell` (Windows-only).
fn query_powershell_profile() -> Option<String> {
    for cmd in &["pwsh", "powershell"] {
        if let Ok(output) = std::process::Command::new(cmd)
            .args(["-NoProfile", "-Command", "echo $PROFILE"])
            .output()
        {
            if output.status.success() {
                let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}
```

- [ ] **Step 4: Build and test**

Run: `cargo build 2>&1 | tail -5 && cargo test config_tests -- --nocapture 2>&1 | tail -10`
Expected: Build OK, all config tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "fix: expand \$PROFILE by querying PowerShell on Windows (#11)

When \$PROFILE env var is not set (e.g., running from cmd or bash on
Windows), fall back to querying pwsh/powershell for the profile path.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 6: Adjacent Code Entry Merging (#8)

**Files:**
- Modify: `src/parser/pending.rs:84-109` (PendingBlock struct)
- Modify: `src/parser/bash/mod.rs` (pending entry handling for code lines)
- Test: `src/parser/bash/mod.rs` (embedded tests)

- [ ] **Step 1: Write failing tests for code merging**

In `src/parser/bash/mod.rs`, in the `#[cfg(test)] mod tests` block, add:

```rust
#[test]
fn test_adjacent_code_lines_merged() {
    let parser = BashParser::new();
    let input = "echo hello\necho world\n";
    let result = parser.parse(input);

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].entry_type, EntryType::Code);
    assert_eq!(result.entries[0].value, "echo hello\necho world");
}

#[test]
fn test_code_lines_split_by_blank() {
    let parser = BashParser::new();
    let input = "echo hello\n\necho world\n";
    let result = parser.parse(input);

    // Blank line separates: first code+blank entry, then second code entry
    assert!(result.entries.len() >= 2);
    // First entry should contain "echo hello" and trailing blank
    assert!(result.entries[0].value.contains("echo hello"));
    // Last entry should be "echo world"
    let last = result.entries.last().unwrap();
    assert!(last.value.contains("echo world"));
}

#[test]
fn test_three_adjacent_code_lines_merged() {
    let parser = BashParser::new();
    let input = "echo a\necho b\necho c\n";
    let result = parser.parse(input);

    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].value, "echo a\necho b\necho c");
}

#[test]
fn test_code_then_blank_then_code_separate() {
    let parser = BashParser::new();
    let input = "echo a\necho b\n\necho c\n";
    let result = parser.parse(input);

    // First entry: "echo a\necho b" + trailing blank
    // Second entry: "echo c"
    assert!(result.entries.len() >= 2);
    let last = result.entries.last().unwrap();
    assert_eq!(last.value, "echo c");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_adjacent_code -- --nocapture 2>&1 | head -20`
Run: `cargo test test_code_lines_split -- --nocapture 2>&1 | head -20`
Run: `cargo test test_three_adjacent -- --nocapture 2>&1 | head -20`
Expected: FAIL — currently each code line is a separate entry.

- [ ] **Step 3: Add has_absorbed_blanks to PendingBlock**

In `src/parser/pending.rs`, add field to `PendingBlock`:

```rust
pub struct PendingBlock {
    pub lines: Vec<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub boundary: BoundaryType,
    pub entry_hint: Option<EntryType>,
    pub name: Option<String>,
    pub value: Option<String>,
    pub comment_count: usize,
    pub has_absorbed_blanks: bool,  // NEW
}
```

Initialize to `false` in all factory methods (`new()`, `function()`, `control()`, `multiline_alias()`, `comment()`, `blank_lines()`, `code()`). Set `has_absorbed_blanks: false` in each.

- [ ] **Step 4: Modify bash parser to merge adjacent code**

In `src/parser/bash/mod.rs`, find the section where a new code line encounters an existing `CodeWithBlanks` pending entry. The key change:

When pending is `CodeWithBlanks` and new line is a code line (not blank, not comment, not structured):
- If `!pending.has_absorbed_blanks` → add line to pending (merge)
- If `pending.has_absorbed_blanks` → flush pending, start new code pending

When pending is `CodeWithBlanks` and new line is blank:
- Add to pending, set `pending.has_absorbed_blanks = true`

The exact code changes depend on the parser's flow structure — look for where `CodeWithBlanks` pending blocks encounter new non-blank, non-comment, non-structured lines and modify that branch.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test test_adjacent_code test_code_lines_split test_three_adjacent test_code_then_blank -- --nocapture`
Expected: All 4 new tests PASS.

- [ ] **Step 6: Run full test suite — fix any broken tests**

Run: `cargo test 2>&1 | tail -20`
Expected: Some existing tests may break if they expect individual code entries for adjacent lines. Update those tests to expect merged entries.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: merge adjacent code entries in parser (#8)

Adjacent code lines (no blank between) are now merged into a single
Code entry. Trailing blank lines are absorbed. A blank line gap starts
a new entry. Added has_absorbed_blanks tracking to PendingBlock.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 7: Multi-Parse Edit (#7)

**Files:**
- Modify: `src/tui/app.rs:716-736` (run_edit_entry)
- Test: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write test for multi-entry edit replacement**

This is covered by Task 2's tests for `replace_entry_with_parsed`. The integration point is in `run_edit_entry()` which we can't easily unit test (requires TUI). Verify the logic manually.

- [ ] **Step 2: Modify run_edit_entry() to use all parsed entries**

In `src/tui/app.rs`, replace the parsing section in `run_edit_entry()`:

```rust
Ok(Some(new_content)) => {
    let new_content = new_content.trim_end_matches('\n').to_string();
    if new_content != value {
        self.undo_snapshot =
            Some(crate::tui::operations::take_snapshot(&self.profile));
        let parser = crate::parser::get_parser(self.profile.shell_type);
        let parsed = parser.parse(&new_content);
        let new_entries: Vec<_> = parsed
            .entries
            .into_iter()
            .map(|mut e| {
                e.file_index = fi;
                e
            })
            .collect();

        if new_entries.is_empty() {
            // Empty edit = delete entry
            self.profile.files[fi].entries.remove(ei);
            self.profile.files[fi].dirty = true;
            self.profile.files[fi].recalculate_line_numbers();
            self.rebuild_list();
            self.message = Some("Entry deleted (empty content)".into());
        } else {
            let count = crate::tui::operations::replace_entry_with_parsed(
                &mut self.profile.files[fi],
                ei,
                new_entries,
                fi,
            );
            self.rebuild_list();
            self.message = Some(if count == 1 {
                "Entry updated".into()
            } else {
                format!("Entry replaced with {} entries", count)
            });
        }
    } else {
        self.message = Some("No changes".into());
    }
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build 2>&1 | tail -5 && cargo test 2>&1 | tail -5`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: edit entry produces multiple entries from parsed content (#7)

Editing an entry now parses the full content and replaces the original
with all resulting entries. Empty edit result deletes the entry.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 8: Startup File Path Validation (#1)

**Files:**
- Modify: `src/model/profile.rs:14-21` (add writable field)
- Modify: `src/main.rs` (add startup_file_check)
- Modify: `src/tui/app.rs:27-62` (add config/shell_key fields, readonly checks)
- Modify: `src/tui/ui.rs:210-238` (greyed-out file headers and entries)
- Create: `src/utils/path.rs` addition (check_writable function)

- [ ] **Step 1: Add writable field to ProfileFile**

In `src/model/profile.rs`, add to ProfileFile struct:

```rust
pub struct ProfileFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub content: String,
    pub expanded: bool,
    pub dirty: bool,
    pub exists: bool,
    pub writable: bool,  // NEW
}
```

Update all constructors (`new()`, `new_with_entries()`) to default `writable: true`.

- [ ] **Step 2: Add check_writable() utility**

In `src/utils/path.rs`, add:

```rust
use std::path::Path;

/// Check if a file path is writable.
/// For existing files: try opening for write.
/// For non-existent files: check parent directory.
pub fn check_writable(path: &Path) -> bool {
    if path.exists() {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .is_ok()
    } else {
        path.parent().map_or(false, |p| p.exists() && check_writable(p))
    }
}
```

Export from `src/utils/mod.rs`:
```rust
pub mod path;
```

- [ ] **Step 3: Add startup_file_check to main.rs**

In `src/main.rs`, add function and call it before `TuiApp::new()`:

```rust
use wenv::config::path_resolver;
use wenv::model::profile::ShellProfile;
use wenv::model::Config;

fn startup_file_check(
    profile: &mut ShellProfile,
    config: &mut Config,
    shell_key: &str,
) -> Result<()> {
    let mut paths_to_remove: Vec<std::path::PathBuf> = Vec::new();

    for file in &mut profile.files {
        if !file.exists {
            println!("⚠ File not found: {}", file.path.display());
            let create = dialoguer::Confirm::new()
                .with_prompt("  Create this file?")
                .default(true)
                .interact_opt()?
                .unwrap_or(false);

            if create {
                if let Some(parent) = file.path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::File::create(&file.path) {
                    Ok(_) => {
                        file.exists = true;
                        file.content = String::new();
                        println!("  ✓ Created: {}", file.path.display());
                    }
                    Err(e) => {
                        eprintln!("  ✗ Failed to create: {}", e);
                        let remove = dialoguer::Confirm::new()
                            .with_prompt("  Remove this path from config?")
                            .default(false)
                            .interact_opt()?
                            .unwrap_or(false);
                        if remove {
                            paths_to_remove.push(file.path.clone());
                        }
                    }
                }
            }
        }
    }

    if !paths_to_remove.is_empty() {
        if let Some(files_config) = config.files.get_mut(shell_key) {
            files_config.paths.retain(|p| {
                let expanded =
                    path_resolver::expand_env_vars(&path_resolver::expand_tilde(p));
                !paths_to_remove
                    .iter()
                    .any(|r| r == &std::path::PathBuf::from(&expanded))
            });
        }
        config.save()?;
        profile.files.retain(|f| !paths_to_remove.contains(&f.path));
    }

    for file in &mut profile.files {
        file.writable = if file.exists {
            wenv::utils::path::check_writable(&file.path)
        } else {
            false
        };
    }

    Ok(())
}
```

In main(), call it:
```rust
let mut profile = model::profile::load_shell_profile(&config, shell_type)?;
startup_file_check(&mut profile, &mut config, shell_key)?;
TuiApp::new(profile, messages)?.run()
```

- [ ] **Step 4: Add config and shell_key fields to TuiApp**

In `src/tui/app.rs`, modify TuiApp struct:

```rust
pub struct TuiApp {
    pub profile: ShellProfile,
    pub visible_items: Vec<ListItem>,
    pub cursor: usize,
    pub mode: AppMode,
    pub previous_mode: Option<AppMode>,
    pub should_quit: bool,
    pub message: Option<String>,
    pub messages: &'static Messages,
    pub selection: SelectionState,
    pub clipboard: ClipboardState,
    pub undo_snapshot: Option<UndoSnapshot>,
    pub move_state: Option<MoveState>,
    pub search: Option<SearchState>,
    pub list_visible_height: usize,
    pub config: crate::model::Config,       // NEW
    pub shell_key: String,                   // NEW
}
```

Update `TuiApp::new()` to accept and store these:
```rust
pub fn new(
    profile: ShellProfile,
    messages: &'static Messages,
    config: crate::model::Config,
    shell_key: String,
) -> Result<Self>
```

Update `main.rs` call site accordingly.

- [ ] **Step 5: Add readonly checks in action handlers**

In `src/tui/app.rs`, add a helper method:

```rust
fn is_current_file_writable(&self) -> bool {
    let fi = self.current_file_index();
    fi < self.profile.files.len() && self.profile.files[fi].writable
}
```

Add writable checks at the top of these action handlers: `Edit`, `Add`, `Delete` (entry branch), `Cut`, `Paste`, `StartMove`, `Remark`. Pattern:
```rust
if !self.is_current_file_writable() {
    self.message = Some("File is read-only".into());
    return Ok(EditorRequest::None);
}
```

- [ ] **Step 6: Add greyed-out styling in ui.rs**

In `src/tui/ui.rs`, in the file header rendering section, add a writable check:

When rendering file headers: if `!file.writable`, use `Style::default().fg(Color::DarkGray)` instead of Yellow.

When rendering entries: if `!profile.files[fi].writable`, use `Style::default().fg(Color::DarkGray)` for all text elements.

- [ ] **Step 7: Update AppMode enum in state.rs**

In `src/tui/state.rs`:

```rust
pub enum AppMode {
    Normal,
    Searching,
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    ConfirmQuit,
    Moving,
    TextInput,          // NEW
    ConfirmRemoveFile,  // NEW
    ConfirmCreateFile,  // NEW
}
```

Also add TextInputState:

```rust
pub enum InputPurpose {
    AddFilePath,
}

pub struct TextInputState {
    pub prompt: String,
    pub value: String,
    pub cursor_pos: usize,
    pub purpose: InputPurpose,
}
```

- [ ] **Step 8: Build and test**

Run: `cargo build 2>&1 | tail -10 && cargo test 2>&1 | tail -5`
Expected: Build OK, all tests pass. Fix any compilation errors from constructor changes.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: startup file validation and readonly mode (#1)

- Check file existence at startup with dialoguer prompts
- Offer to create missing files or remove from config
- Check write permissions; greyed-out display for readonly files
- Block editing operations on readonly files
- Add config/shell_key to TuiApp for runtime config changes

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 9: Remove File from Config (#4)

**Files:**
- Modify: `src/tui/app.rs` (Delete handler for FileHeader, Confirm handler)
- Modify: `src/tui/state.rs` (already has ConfirmRemoveFile from Task 8)

- [ ] **Step 1: Add pending_remove_fi field to TuiApp**

In `src/tui/app.rs`, add to TuiApp struct:

```rust
pub pending_remove_fi: Option<usize>,  // NEW: file index pending config removal
```

Initialize to `None` in `new()`.

- [ ] **Step 2: Modify Delete handler to detect FileHeader**

In `src/tui/app.rs`, in `handle_action()`, modify `Action::Delete`:

```rust
Action::Delete => {
    if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
        let fi = *fi;
        self.pending_remove_fi = Some(fi);
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::ConfirmRemoveFile;
        self.message = Some(format!(
            "Remove '{}' from config? (y/n) (file won't be deleted)",
            self.profile.files[fi].display_name()
        ));
    } else {
        // Original entry deletion logic (unchanged)
        let targets = self.get_operation_targets();
        if !targets.is_empty() {
            self.undo_snapshot =
                Some(crate::tui::operations::take_snapshot(&self.profile));
            self.previous_mode = Some(self.mode.clone());
            self.mode = AppMode::ConfirmDelete;
            let count = targets.len();
            self.message = Some(format!("Delete {} entries? (y/n)", count));
        }
    }
}
```

- [ ] **Step 3: Add ConfirmRemoveFile handler in Confirm action**

In `handle_action()`, in the `Action::Confirm` match, add:

```rust
AppMode::ConfirmRemoveFile => {
    if let Some(fi) = self.pending_remove_fi.take() {
        let path = self.profile.files[fi].path.clone();

        // Remove from config
        let shell_key = self.shell_key.clone();
        if let Some(files_config) = self.config.files.get_mut(&shell_key) {
            files_config.paths.retain(|p| {
                let expanded = crate::config::path_resolver::expand_env_vars(
                    &crate::config::path_resolver::expand_tilde(p),
                );
                std::path::PathBuf::from(&expanded) != path
            });
        }
        if let Err(e) = self.config.save() {
            self.message = Some(format!("Config save error: {}", e));
        } else {
            // Remove from profile
            self.profile.files.remove(fi);

            // Fix file_index for remaining entries
            for (new_fi, file) in self.profile.files.iter_mut().enumerate() {
                for entry in &mut file.entries {
                    entry.file_index = new_fi;
                }
            }

            self.selection.clear();
            self.rebuild_list();
            self.message = Some("Removed from config".into());
        }
    }
    self.mode = AppMode::Normal;
    self.previous_mode = None;
}
```

Also handle Cancel for ConfirmRemoveFile:

In `Action::Cancel`, add:
```rust
AppMode::ConfirmRemoveFile => {
    self.pending_remove_fi = None;
    self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
    self.message = Some("Cancelled".into());
}
```

- [ ] **Step 4: Build and test**

Run: `cargo build 2>&1 | tail -10 && cargo test 2>&1 | tail -5`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: remove file from config with d on FileHeader (#4)

Pressing 'd' on a file header prompts to remove the file path from
wenv config.toml. The file itself is not deleted. Config is saved
and the profile reloaded.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 10: Add File Path to Config (#5)

**Files:**
- Modify: `src/tui/app.rs` (AddFile handler, TextInput handling, ConfirmCreateFile)
- Modify: `src/tui/ui.rs` (text input bar rendering)

- [ ] **Step 1: Add text_input and pending_create_path fields to TuiApp**

In `src/tui/app.rs`, add to TuiApp struct:

```rust
pub text_input: Option<crate::tui::state::TextInputState>,
pub pending_create_path: Option<(String, std::path::PathBuf)>,
```

Initialize both to `None` in `new()`.

- [ ] **Step 2: Implement AddFile action handler**

In `handle_action()`, replace the placeholder:

```rust
Action::AddFile => {
    self.text_input = Some(crate::tui::state::TextInputState {
        prompt: "New file path: ".into(),
        value: String::new(),
        cursor_pos: 0,
        purpose: crate::tui::state::InputPurpose::AddFilePath,
    });
    self.mode = AppMode::TextInput;
    self.message = None;
}
```

- [ ] **Step 3: Implement TextInput key handlers**

In `handle_action()`:

```rust
Action::TextInputChar(c) => {
    if let Some(ref mut input) = self.text_input {
        input.value.insert(input.cursor_pos, c);
        input.cursor_pos += 1;
    }
}
Action::TextInputBackspace => {
    if let Some(ref mut input) = self.text_input {
        if input.cursor_pos > 0 {
            input.cursor_pos -= 1;
            input.value.remove(input.cursor_pos);
        }
    }
}
Action::TextInputLeft => {
    if let Some(ref mut input) = self.text_input {
        if input.cursor_pos > 0 {
            input.cursor_pos -= 1;
        }
    }
}
Action::TextInputRight => {
    if let Some(ref mut input) = self.text_input {
        if input.cursor_pos < input.value.len() {
            input.cursor_pos += 1;
        }
    }
}
```

- [ ] **Step 4: Implement TextInput Confirm (Enter)**

In `Action::Confirm`, add `AppMode::TextInput` branch:

```rust
AppMode::TextInput => {
    if let Some(input) = self.text_input.take() {
        match input.purpose {
            crate::tui::state::InputPurpose::AddFilePath => {
                let raw_path = input.value.trim().to_string();
                if raw_path.is_empty() {
                    self.mode = AppMode::Normal;
                    return Ok(EditorRequest::None);
                }

                let expanded = crate::config::path_resolver::expand_env_vars(
                    &crate::config::path_resolver::expand_tilde(&raw_path),
                );
                let path = std::path::PathBuf::from(&expanded);

                if self.profile.files.iter().any(|f| f.path == path) {
                    self.message = Some("Path already in config".into());
                    self.mode = AppMode::Normal;
                    return Ok(EditorRequest::None);
                }

                if !path.exists() {
                    self.pending_create_path = Some((raw_path, path));
                    self.mode = AppMode::ConfirmCreateFile;
                    self.message =
                        Some("File doesn't exist. Create? (y/n)".into());
                } else {
                    self.add_file_to_config_and_profile(raw_path, path)?;
                    self.mode = AppMode::Normal;
                }
            }
        }
    }
}
```

- [ ] **Step 5: Implement add_file_to_config_and_profile helper**

In `src/tui/app.rs`, add method to `impl TuiApp`:

```rust
fn add_file_to_config_and_profile(
    &mut self,
    raw_path: String,
    path: std::path::PathBuf,
) -> anyhow::Result<()> {
    let shell_key = self.shell_key.clone();
    let files_config = self
        .config
        .files
        .entry(shell_key)
        .or_insert_with(|| crate::model::FilesConfig { paths: vec![] });
    files_config.paths.push(raw_path);
    self.config.save()?;

    let fi = self.profile.files.len();
    let exists = path.exists();
    let content = if exists {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let parser = crate::parser::get_parser(self.profile.shell_type);
    let parsed = parser.parse(&content);
    let entries: Vec<_> = parsed
        .entries
        .into_iter()
        .map(|mut e| {
            e.file_index = fi;
            e
        })
        .collect();

    let mut file = crate::model::profile::ProfileFile::new(path.clone(), exists);
    file.entries = entries;
    file.content = content;
    file.expanded = true;
    file.writable = crate::utils::path::check_writable(&path);
    self.profile.files.push(file);

    self.rebuild_list();
    self.message = Some("File added to config".into());
    Ok(())
}
```

- [ ] **Step 6: Implement ConfirmCreateFile handler**

In `Action::Confirm`, add:

```rust
AppMode::ConfirmCreateFile => {
    if let Some((raw_path, path)) = self.pending_create_path.take() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        match std::fs::File::create(&path) {
            Ok(_) => {
                self.add_file_to_config_and_profile(raw_path, path)?;
            }
            Err(e) => {
                self.message = Some(format!("Failed to create: {}", e));
            }
        }
    }
    self.mode = AppMode::Normal;
}
```

In `Action::Cancel`, add:
```rust
AppMode::ConfirmCreateFile => {
    self.pending_create_path = None;
    self.mode = AppMode::Normal;
    self.message = Some("Cancelled".into());
}
AppMode::TextInput => {
    self.text_input = None;
    self.mode = AppMode::Normal;
    self.message = None;
}
```

- [ ] **Step 7: Add text input bar rendering in ui.rs**

In `src/tui/ui.rs`, in the `draw()` function, add rendering for TextInput mode. When `app.mode == AppMode::TextInput`, render a text input bar at the bottom (similar position to search bar):

```rust
if let Some(ref input) = app.text_input {
    // Render: "{prompt}{value}█" at bottom of screen
    let text = format!("{}{}", input.prompt, input.value);
    let input_bar = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan));
    // Render in the status/message area
}
```

- [ ] **Step 8: Build and test**

Run: `cargo build 2>&1 | tail -10 && cargo test 2>&1 | tail -5`
Expected: All pass.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: add file path to config with 'a' key (#5)

Press 'a' to enter a file path via TUI text input bar. The path is
added to config.toml, parsed, and loaded into the profile. Prompts
to create the file if it doesn't exist.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 11: Remark Toggle (#9)

**Files:**
- Modify: `src/tui/operations.rs` (add comment_value, uncomment_value)
- Modify: `src/tui/app.rs` (Remark handler)
- Test: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write tests for comment/uncomment helpers**

In `tests/tui_logic_tests.rs`:

```rust
#[test]
fn test_comment_value_adds_hash() {
    use wenv::tui::operations::{comment_value, uncomment_value};

    let input = "alias foo='bar'\nexport PATH='/bin'";
    let result = comment_value(input);
    assert_eq!(result, "# alias foo='bar'\n# export PATH='/bin'");
}

#[test]
fn test_comment_value_preserves_blank_lines() {
    use wenv::tui::operations::comment_value;

    let input = "echo hello\n\necho world";
    let result = comment_value(input);
    assert_eq!(result, "# echo hello\n\n# echo world");
}

#[test]
fn test_comment_value_double_comments() {
    use wenv::tui::operations::comment_value;

    let input = "# already commented\necho hello";
    let result = comment_value(input);
    assert_eq!(result, "# # already commented\n# echo hello");
}

#[test]
fn test_uncomment_value_removes_hash() {
    use wenv::tui::operations::uncomment_value;

    let input = "# alias foo='bar'\n# export PATH='/bin'";
    let result = uncomment_value(input);
    assert_eq!(result, "alias foo='bar'\nexport PATH='/bin'");
}

#[test]
fn test_uncomment_value_preserves_blank_lines() {
    use wenv::tui::operations::uncomment_value;

    let input = "# echo hello\n\n# echo world";
    let result = uncomment_value(input);
    assert_eq!(result, "echo hello\n\necho world");
}

#[test]
fn test_uncomment_value_handles_no_space() {
    use wenv::tui::operations::uncomment_value;

    let input = "#echo hello";
    let result = uncomment_value(input);
    assert_eq!(result, "echo hello");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_comment_value test_uncomment_value -- --nocapture 2>&1 | head -20`
Expected: Compilation error — functions don't exist.

- [ ] **Step 3: Implement comment_value and uncomment_value**

In `src/tui/operations.rs`:

```rust
/// Add "# " to all non-blank lines (including already-commented lines).
pub fn comment_value(value: &str) -> String {
    value
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("# {}", line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove leading "# " or "#" from non-blank lines.
pub fn uncomment_value(value: &str) -> String {
    value
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else if line.starts_with("# ") {
                line[2..].to_string()
            } else if line.starts_with('#') {
                line[1..].to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_comment_value test_uncomment_value -- --nocapture`
Expected: All 6 tests PASS.

- [ ] **Step 5: Implement Remark handler in app.rs**

In `handle_action()`, replace the placeholder:

```rust
Action::Remark => {
    let targets = self.get_operation_targets();
    if targets.is_empty() {
        return Ok(EditorRequest::None);
    }

    // Check writable
    let target_entries: Vec<(usize, usize)> = targets
        .iter()
        .filter_map(|&idx| match self.visible_items.get(idx) {
            Some(ListItem::Entry(fi, ei)) => Some((*fi, *ei)),
            _ => None,
        })
        .collect();

    if target_entries.is_empty() {
        return Ok(EditorRequest::None);
    }

    let affected_files: std::collections::HashSet<usize> =
        target_entries.iter().map(|(fi, _)| *fi).collect();

    if affected_files.iter().any(|&fi| !self.profile.files[fi].writable) {
        self.message = Some("File is read-only".into());
        return Ok(EditorRequest::None);
    }

    let all_comment = target_entries.iter().all(|(fi, ei)| {
        self.profile.files[*fi].entries[*ei].entry_type
            == crate::model::EntryType::Comment
    });

    self.undo_snapshot =
        Some(crate::tui::operations::take_snapshot(&self.profile));

    // Track original range for selection restoration
    let first_visible = targets[0];
    let last_visible = *targets.last().unwrap();

    if all_comment {
        // UNCOMMENT: process in reverse order for stable indices
        let mut reversed = target_entries.clone();
        reversed.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

        for (fi, ei) in reversed {
            let value = self.profile.files[fi].entries[ei].value.clone();
            let uncommented = crate::tui::operations::uncomment_value(&value);
            let parser = crate::parser::get_parser(self.profile.shell_type);
            let parsed = parser.parse(&uncommented);
            let new_entries: Vec<_> = parsed
                .entries
                .into_iter()
                .map(|mut e| {
                    e.file_index = fi;
                    e
                })
                .collect();
            crate::tui::operations::replace_entry_with_parsed(
                &mut self.profile.files[fi],
                ei,
                new_entries,
                fi,
            );
        }
        self.message = Some("Uncommented".into());
    } else {
        // COMMENT: add "# " to non-Comment entries
        for (fi, ei) in &target_entries {
            if self.profile.files[*fi].entries[*ei].entry_type
                != crate::model::EntryType::Comment
            {
                let value = self.profile.files[*fi].entries[*ei].value.clone();
                let commented = crate::tui::operations::comment_value(&value);
                let entry = &mut self.profile.files[*fi].entries[*ei];
                entry.value = commented;
                entry.entry_type = crate::model::EntryType::Comment;
                self.profile.files[*fi].dirty = true;
            }
        }
        for &fi in &affected_files {
            self.profile.files[fi].recalculate_line_numbers();
        }
        self.message = Some("Commented".into());
    }

    // Rebuild and restore selection range
    self.rebuild_list();

    // Restore selection: select entries from first_visible through the new range
    self.selection.clear();
    let new_end = (last_visible)
        .min(self.visible_items.len().saturating_sub(1));
    for idx in first_visible..=new_end {
        if matches!(self.visible_items.get(idx), Some(ListItem::Entry(_, _))) {
            self.selection.toggle(idx, &self.visible_items);
        }
    }
}
```

- [ ] **Step 6: Build and test**

Run: `cargo build 2>&1 | tail -10 && cargo test 2>&1 | tail -5`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: remark toggle with r key (#9)

Press 'r' to toggle comment/uncomment on selected entries.
- All Comment entries selected → uncomment (remove '# ', re-parse)
- Otherwise → comment non-Comment entries (add '# ')
- Blank lines within entries preserved
- Supports multi-selection with range restoration

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 12: Update Documentation

**Files:**
- Modify: `CLAUDE.md` (key bindings table)

- [ ] **Step 1: Update TUI Key Bindings Reference in CLAUDE.md**

Replace the key bindings table with:

```markdown
| Key | Action |
|-----|--------|
| `j`/`k`, `↑`/`↓` | Navigate entries |
| `Space` | Toggle expand/detail |
| `s` | Toggle selection |
| `Shift+↑`/`↓` | Extend selection range |
| `Enter` | Edit / Confirm |
| `e` | Edit entry with $EDITOR |
| `n` | New entry with $EDITOR |
| `d` | Delete entries / Remove file from config |
| `x` | Cut selected entries |
| `c` | Copy selected entries |
| `v` | Paste clipboard entries |
| `m` | Enter move mode |
| `r` | Toggle remark (comment/uncomment) |
| `a` | Add file path to config |
| `0` | Collapse all files |
| `9` | Expand all files |
| `z` | Undo last operation |
| `/` | Search/filter entries |
| `Esc` | Clear selection/exit modes |
| `w` / `Ctrl+s` | Save all changes |
| `?` | Show help |
| `q` | Quit (confirms if unsaved) |
```

- [ ] **Step 2: Run clippy and format**

Run: `cargo clippy 2>&1 | tail -20 && cargo fmt`

- [ ] **Step 3: Run full test suite one final time**

Run: `cargo test 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "docs: update CLAUDE.md key bindings for TUI enhancements

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Summary

| Task | Feature(s) | Risk | Key Files |
|------|-----------|------|-----------|
| 1 | #6 Line recalc (infra) | Low | profile.rs, operations.rs, app.rs |
| 2 | #7 replace_entry (infra) | Low | operations.rs |
| 3 | #2 Key rebind + #3 Copy | Low | keys.rs, app.rs, ui.rs |
| 4 | #10 Insert at top | Low | app.rs |
| 5 | #11 $PROFILE fix | Low | path_resolver.rs |
| 6 | #8 Code merging | High | pending.rs, bash/mod.rs |
| 7 | #7 Multi-parse edit | Medium | app.rs |
| 8 | #1 Startup check | Medium | main.rs, profile.rs, ui.rs, path.rs |
| 9 | #4 Remove file | Medium | app.rs |
| 10 | #5 Add file | Medium | app.rs, state.rs, ui.rs |
| 11 | #9 Remark toggle | High | operations.rs, app.rs |
| 12 | Docs | Low | CLAUDE.md |
