# Multi-step Undo/Redo & Config Delete Prompt Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace single-step undo with bounded multi-step undo/redo, and improve config file deletion prompts to show original config patterns and warn about cascading glob removals.

**Architecture:** Convert `Option<UndoSnapshot>` to `VecDeque<UndoSnapshot>` (undo) + `Vec<UndoSnapshot>` (redo) with a cap of 20. Add `Action::Redo` mapped to `y` key. For config deletion, reverse-lookup the raw config pattern and expand it to detect multi-file impact before showing the confirmation prompt.

**Tech Stack:** Rust, ratatui TUI, crossterm, glob crate (already in deps)

**Spec:** `docs/superpowers/specs/2026-03-26-undo-redo-config-delete-design.md`

---

## File Structure

| File | Role | Changes |
|------|------|---------|
| `src/tui/state.rs` | State types | No structural changes needed (UndoSnapshot stays the same) |
| `src/tui/operations.rs` | Entry manipulation | Add `push_undo()`, `find_matching_config_pattern()` |
| `src/tui/keys.rs` | Key bindings | Add `Action::Redo`, map `y` key |
| `src/tui/app.rs` | Main TUI logic | Replace `undo_snapshot` field with stacks, update all 7 snapshot sites + undo/redo/cancel handlers |
| `src/tui/ui.rs` | Rendering | Update help text, make confirm popup support multi-line |
| `tests/tui_logic_tests.rs` | Tests | Add undo/redo stack tests, config pattern lookup tests |

---

### Task 1: Add `push_undo` helper and `Redo` action

**Files:**
- Modify: `src/tui/operations.rs:1-5` (add import + constant + function)
- Modify: `src/tui/keys.rs:6-42` (add Redo variant)
- Modify: `src/tui/keys.rs:90` (add y key mapping)

- [ ] **Step 1: Write the failing test for push_undo**

Add to `tests/tui_logic_tests.rs`:

```rust
#[test]
fn test_push_undo_caps_at_max() {
    use std::collections::VecDeque;
    use wenv::tui::state::UndoSnapshot;

    let mut undo_stack: VecDeque<UndoSnapshot> = VecDeque::new();
    let mut redo_stack: Vec<UndoSnapshot> = Vec::new();

    // Push 21 snapshots
    for _ in 0..21 {
        let profile = make_test_profile();
        let snapshot = operations::take_snapshot(&profile);
        operations::push_undo(&mut undo_stack, &mut redo_stack, snapshot);
    }

    assert_eq!(undo_stack.len(), 20); // capped at MAX_UNDO_HISTORY
}

#[test]
fn test_push_undo_clears_redo() {
    use std::collections::VecDeque;
    use wenv::tui::state::UndoSnapshot;

    let mut undo_stack: VecDeque<UndoSnapshot> = VecDeque::new();
    let mut redo_stack: Vec<UndoSnapshot> = Vec::new();

    // Simulate: operation → undo → new operation should clear redo
    let profile = make_test_profile();
    let s1 = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, s1);

    // Simulate undo by moving to redo
    if let Some(s) = undo_stack.pop_back() {
        redo_stack.push(s);
    }
    assert_eq!(redo_stack.len(), 1);

    // New operation should clear redo
    let s2 = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, s2);
    assert_eq!(redo_stack.len(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_push_undo --no-run 2>&1 | head -20`
Expected: compile error — `push_undo` does not exist yet

- [ ] **Step 3: Implement `push_undo` in operations.rs**

Add at top of `src/tui/operations.rs` (after existing imports, before `take_snapshot`):

```rust
use std::collections::VecDeque;

pub const MAX_UNDO_HISTORY: usize = 20;

/// Push an undo snapshot onto the stack, clearing redo history.
/// If the stack exceeds MAX_UNDO_HISTORY, the oldest snapshot is discarded.
pub fn push_undo(
    undo_stack: &mut VecDeque<UndoSnapshot>,
    redo_stack: &mut Vec<UndoSnapshot>,
    snapshot: UndoSnapshot,
) {
    if undo_stack.len() >= MAX_UNDO_HISTORY {
        undo_stack.pop_front();
    }
    undo_stack.push_back(snapshot);
    redo_stack.clear();
}
```

Also re-export `UndoSnapshot` type in the public interface (it's already public via `state.rs`).

- [ ] **Step 4: Add `Action::Redo` to keys.rs**

In `src/tui/keys.rs`, add `Redo` variant to the `Action` enum (after line 27 `Undo`):

```rust
    Undo,
    Redo,
```

In `map_normal_key` (line 90), add after the `z` mapping:

```rust
        KeyCode::Char('z') => Action::Undo,
        KeyCode::Char('y') => Action::Redo,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test test_push_undo -- --nocapture`
Expected: both `test_push_undo_caps_at_max` and `test_push_undo_clears_redo` PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/operations.rs src/tui/keys.rs tests/tui_logic_tests.rs
git commit -m "feat: add push_undo helper with bounded stack and Action::Redo

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: Replace `Option<UndoSnapshot>` with undo/redo stacks in TuiApp

**Files:**
- Modify: `src/tui/app.rs:18` (import VecDeque)
- Modify: `src/tui/app.rs:38` (replace field)
- Modify: `src/tui/app.rs:69` (init)

- [ ] **Step 1: Update imports in app.rs**

At `src/tui/app.rs:18`, change the import from:
```rust
use crate::tui::state::{AppMode, ClipboardState, FileMovingState, MoveState, UndoSnapshot};
```
to:
```rust
use crate::tui::state::{AppMode, ClipboardState, FileMovingState, MoveState};
```

Add at line 10 (imports section):
```rust
use std::collections::VecDeque;
```

- [ ] **Step 2: Replace field in TuiApp struct**

At `src/tui/app.rs:38`, replace:
```rust
    pub undo_snapshot: Option<UndoSnapshot>,
```
with:
```rust
    pub undo_stack: VecDeque<crate::tui::state::UndoSnapshot>,
    pub redo_stack: Vec<crate::tui::state::UndoSnapshot>,
```

- [ ] **Step 3: Update TuiApp::new initialization**

At `src/tui/app.rs:69`, replace:
```rust
            undo_snapshot: None,
```
with:
```rust
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
```

- [ ] **Step 4: Verify it compiles (expect errors at usage sites)**

Run: `cargo check 2>&1 | grep "undo_snapshot" | head -10`
Expected: errors at the 5 snapshot assignment sites + undo handler + cancel handlers. This is expected — we'll fix them in the next tasks.

- [ ] **Step 5: Commit (WIP — intentionally breaks compilation; fixed in Tasks 3-4, squash before merge)**

```bash
git add src/tui/app.rs
git commit -m "refactor: replace Option<UndoSnapshot> with undo/redo stacks (WIP)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: Update all snapshot-taking call sites

**Files:**
- Modify: `src/tui/app.rs:339,354,420,456,777-778,1400,1478-1479`

All 7 locations that do `self.undo_snapshot = Some(take_snapshot(...))` must change to `push_undo(...)`.

- [ ] **Step 1: Replace all 7 snapshot sites**

**Site 1 — Delete entries** (`app.rs:339`):
```rust
// BEFORE:
self.undo_snapshot = Some(crate::tui::operations::take_snapshot(&self.profile));
// AFTER:
let snapshot = crate::tui::operations::take_snapshot(&self.profile);
crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
```

**Site 2 — Cut entries** (`app.rs:354`):
Same replacement pattern.

**Site 3 — Start move** (`app.rs:420`):
Same replacement pattern.

**Site 4 — Paste entries** (`app.rs:456`):
Same replacement pattern.

**Site 5 — Remark toggle** (`app.rs:777-778`):
```rust
// BEFORE:
self.undo_snapshot =
    Some(crate::tui::operations::take_snapshot(&self.profile));
// AFTER:
let snapshot = crate::tui::operations::take_snapshot(&self.profile);
crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
```

**Site 6 — Edit entry** (`app.rs:1400`):
Same replacement pattern.

**Site 7 — Add entry** (`app.rs:1478-1479`):
```rust
// BEFORE:
self.undo_snapshot =
    Some(crate::tui::operations::take_snapshot(&self.profile));
// AFTER:
let snapshot = crate::tui::operations::take_snapshot(&self.profile);
crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
```

- [ ] **Step 2: Verify no more references to `undo_snapshot` at assignment sites**

Run: `grep -n "undo_snapshot = Some" src/tui/app.rs`
Expected: no output (all replaced)

- [ ] **Step 3: Commit**

```bash
git add src/tui/app.rs
git commit -m "refactor: update all snapshot sites to use push_undo

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: Update Undo handler and Cancel handlers

**Files:**
- Modify: `src/tui/app.rs:469-478` (Undo handler)
- Modify: `src/tui/app.rs:633-638` (Moving cancel)
- Modify: `src/tui/app.rs:651-652` (ConfirmDelete cancel)

**IMPORTANT SUBTLETY**: The current code takes the undo_snapshot BEFORE entering confirmation/move mode. On cancel, it either restores from it (Moving) or discards it (ConfirmDelete). With the stack, on cancel we must `pop_back()` from `undo_stack` to undo the pre-emptive push. The redo_stack was already cleared by `push_undo`, which is correct since the operation didn't actually complete.

- [ ] **Step 1: Update Undo handler**

At `src/tui/app.rs:469-478`, replace:
```rust
            Action::Undo => {
                if let Some(snapshot) = self.undo_snapshot.take() {
                    crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                    self.selection.clear();
                    self.rebuild_list();
                    self.message = Some("Undone".into());
                } else {
                    self.message = Some("Nothing to undo".into());
                }
            }
```
with:
```rust
            Action::Undo => {
                if let Some(snapshot) = self.undo_stack.pop_back() {
                    let current = crate::tui::operations::take_snapshot(&self.profile);
                    self.redo_stack.push(current);
                    crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                    self.selection.clear();
                    self.rebuild_list();
                    let remaining = self.undo_stack.len();
                    self.message = Some(format!("Undone ({remaining} left)"));
                } else {
                    self.message = Some("Nothing to undo".into());
                }
            }
```

- [ ] **Step 2: Add Redo handler**

Add right after the Undo handler block:
```rust
            Action::Redo => {
                if let Some(snapshot) = self.redo_stack.pop() {
                    let current = crate::tui::operations::take_snapshot(&self.profile);
                    self.undo_stack.push_back(current);
                    if self.undo_stack.len() > crate::tui::operations::MAX_UNDO_HISTORY {
                        self.undo_stack.pop_front();
                    }
                    crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                    self.selection.clear();
                    self.rebuild_list();
                    let remaining = self.redo_stack.len();
                    self.message = Some(format!("Redone ({remaining} left)"));
                } else {
                    self.message = Some("Nothing to redo".into());
                }
            }
```

- [ ] **Step 3: Update Moving cancel handler**

At `src/tui/app.rs:633-638`, replace:
```rust
                    AppMode::Moving => {
                        let from_sel = self.move_state.as_ref().is_some_and(|ms| ms.from_selection);
                        // Restore from snapshot
                        if let Some(snapshot) = self.undo_snapshot.take() {
                            crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                        }
```
with:
```rust
                    AppMode::Moving => {
                        let from_sel = self.move_state.as_ref().is_some_and(|ms| ms.from_selection);
                        // Pop the pre-emptive undo snapshot and restore from it
                        if let Some(snapshot) = self.undo_stack.pop_back() {
                            crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                        }
```

- [ ] **Step 4: Update ConfirmDelete cancel handler**

At `src/tui/app.rs:651-652`, replace:
```rust
                    AppMode::ConfirmDelete => {
                        if let Some(_snapshot) = self.undo_snapshot.take() {}
```
with:
```rust
                    AppMode::ConfirmDelete => {
                        self.undo_stack.pop_back(); // Discard pre-emptive snapshot
```

- [ ] **Step 5: Verify no remaining references to `undo_snapshot`**

Run: `grep -n "undo_snapshot" src/tui/app.rs`
Expected: no output

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all tests pass, no compile errors

- [ ] **Step 7: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat: implement multi-step undo (z) and redo (y)

Replace single Option<UndoSnapshot> with bounded VecDeque (max 20)
and Vec redo stack. Undo pushes current state to redo; new operations
clear redo. Cancel handlers properly pop pre-emptive snapshots.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 5: Write undo/redo integration tests

**Files:**
- Modify: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write multi-step undo test**

```rust
#[test]
fn test_multi_step_undo() {
    use std::collections::VecDeque;
    use wenv::tui::state::UndoSnapshot;

    let mut profile = make_test_profile();
    let mut undo_stack: VecDeque<UndoSnapshot> = VecDeque::new();
    let mut redo_stack: Vec<UndoSnapshot> = Vec::new();
    let items = profile.build_visible_list();

    // Operation 1: delete entry "ll"
    let snap1 = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, snap1);
    operations::delete_entries(&mut profile, &items, &[1]);
    assert_eq!(profile.files[0].entries.len(), 2);

    // Operation 2: delete entry "gs" (now at index 1 after rebuild)
    let items = profile.build_visible_list();
    let snap2 = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, snap2);
    operations::delete_entries(&mut profile, &items, &[1]);
    assert_eq!(profile.files[0].entries.len(), 1);

    // Undo operation 2
    let current = operations::take_snapshot(&profile);
    redo_stack.push(current);
    let snapshot = undo_stack.pop_back().unwrap();
    operations::restore_snapshot(&mut profile, snapshot);
    assert_eq!(profile.files[0].entries.len(), 2);

    // Undo operation 1
    let current = operations::take_snapshot(&profile);
    redo_stack.push(current);
    let snapshot = undo_stack.pop_back().unwrap();
    operations::restore_snapshot(&mut profile, snapshot);
    assert_eq!(profile.files[0].entries.len(), 3);
    assert_eq!(profile.files[0].entries[0].name, "ll");
}
```

- [ ] **Step 2: Write redo test**

```rust
#[test]
fn test_redo_after_undo() {
    use std::collections::VecDeque;
    use wenv::tui::state::UndoSnapshot;

    let mut profile = make_test_profile();
    let mut undo_stack: VecDeque<UndoSnapshot> = VecDeque::new();
    let mut redo_stack: Vec<UndoSnapshot> = Vec::new();
    let items = profile.build_visible_list();

    // Delete "ll"
    let snap = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, snap);
    operations::delete_entries(&mut profile, &items, &[1]);
    assert_eq!(profile.files[0].entries.len(), 2);

    // Undo
    let current = operations::take_snapshot(&profile);
    redo_stack.push(current);
    let snapshot = undo_stack.pop_back().unwrap();
    operations::restore_snapshot(&mut profile, snapshot);
    assert_eq!(profile.files[0].entries.len(), 3);
    assert_eq!(redo_stack.len(), 1);

    // Redo
    let current = operations::take_snapshot(&profile);
    undo_stack.push_back(current);
    let snapshot = redo_stack.pop().unwrap();
    operations::restore_snapshot(&mut profile, snapshot);
    assert_eq!(profile.files[0].entries.len(), 2); // back to deleted state
}
```

- [ ] **Step 3: Write redo-cleared-by-new-op test**

```rust
#[test]
fn test_redo_cleared_by_new_operation() {
    use std::collections::VecDeque;
    use wenv::tui::state::UndoSnapshot;

    let mut profile = make_test_profile();
    let mut undo_stack: VecDeque<UndoSnapshot> = VecDeque::new();
    let mut redo_stack: Vec<UndoSnapshot> = Vec::new();
    let items = profile.build_visible_list();

    // Delete → undo → redo_stack has 1
    let snap = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, snap);
    operations::delete_entries(&mut profile, &items, &[1]);

    let current = operations::take_snapshot(&profile);
    redo_stack.push(current);
    let snapshot = undo_stack.pop_back().unwrap();
    operations::restore_snapshot(&mut profile, snapshot);
    assert_eq!(redo_stack.len(), 1);

    // New operation should clear redo
    let items = profile.build_visible_list();
    let snap = operations::take_snapshot(&profile);
    operations::push_undo(&mut undo_stack, &mut redo_stack, snap);
    operations::delete_entries(&mut profile, &items, &[1]);

    assert_eq!(redo_stack.len(), 0); // cleared
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test test_multi_step_undo test_redo -- --nocapture`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add tests/tui_logic_tests.rs
git commit -m "test: add multi-step undo/redo integration tests

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 6: Add `find_matching_config_pattern` helper

**Files:**
- Modify: `src/tui/operations.rs` (add function at bottom)
- Modify: `tests/tui_logic_tests.rs` (add unit test)

- [ ] **Step 1: Write the failing test**

Add to `tests/tui_logic_tests.rs`:

```rust
#[test]
fn test_find_matching_config_pattern_exact_path() {
    use std::collections::HashMap;
    use wenv::model::config::{Config, FilesConfig, UiConfig};
    use wenv::tui::operations::find_matching_config_pattern;

    let home = dirs::home_dir().unwrap();
    let resolved_path = home.join(".bashrc");

    let mut files = HashMap::new();
    files.insert(
        "bash".to_string(),
        FilesConfig {
            paths: vec!["~/.bashrc".to_string(), "~/.bash_profile".to_string()],
        },
    );
    let config = Config {
        ui: UiConfig::default(),
        files,
    };

    let result = find_matching_config_pattern(&config, "bash", &resolved_path);
    assert!(result.is_some());
    let (raw_pattern, matched_files) = result.unwrap();
    assert_eq!(raw_pattern, "~/.bashrc");
    assert_eq!(matched_files.len(), 1);
    assert_eq!(matched_files[0], resolved_path);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_find_matching_config_pattern --no-run 2>&1 | head -10`
Expected: compile error — function doesn't exist

- [ ] **Step 3: Implement `find_matching_config_pattern`**

Add to bottom of `src/tui/operations.rs`:

```rust
/// Given a resolved file path, find the raw config pattern that matches it
/// and return all files that pattern resolves to.
pub fn find_matching_config_pattern(
    config: &crate::model::Config,
    shell_key: &str,
    resolved_path: &std::path::Path,
) -> Option<(String, Vec<std::path::PathBuf>)> {
    let files_config = config.files.get(shell_key)?;
    for raw_pattern in &files_config.paths {
        let resolved = crate::config::path_resolver::resolve_paths(&[raw_pattern.clone()]);
        if resolved.iter().any(|(p, _)| p == resolved_path) {
            let all_paths: Vec<std::path::PathBuf> =
                resolved.into_iter().map(|(p, _)| p).collect();
            return Some((raw_pattern.clone(), all_paths));
        }
    }
    None
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test test_find_matching_config_pattern -- --nocapture`
Expected: PASS

Note: This test depends on `~` expanding to the actual home dir. If running in a restricted environment, the tilde expansion must work correctly. The `dirs` crate is already a dependency.

- [ ] **Step 5: Commit**

```bash
git add src/tui/operations.rs tests/tui_logic_tests.rs
git commit -m "feat: add find_matching_config_pattern for config path reverse-lookup

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 7: Update config delete prompt to show raw pattern and affected files

**Files:**
- Modify: `src/tui/app.rs:321-330` (Delete handler for FileHeader)
- Modify: `src/tui/app.rs:529-559` (ConfirmRemoveFile handler)

- [ ] **Step 1: Update delete prompt construction**

At `src/tui/app.rs:321-330`, replace the `FileHeader` branch:

```rust
// BEFORE:
                if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
                    let fi = *fi;
                    self.pending_remove_fi = Some(fi);
                    self.previous_mode = Some(self.mode.clone());
                    self.mode = AppMode::ConfirmRemoveFile;
                    self.message = Some(format!(
                        "Remove '{}' from config? (y/n) (file won't be deleted)",
                        self.profile.files[fi].display_name()
                    ));
// AFTER:
                if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
                    let fi = *fi;
                    let resolved_path = &self.profile.files[fi].path;

                    let (raw_pattern, affected_files) =
                        crate::tui::operations::find_matching_config_pattern(
                            &self.config,
                            &self.shell_key,
                            resolved_path,
                        )
                        .unwrap_or_else(|| {
                            (resolved_path.display().to_string(), vec![resolved_path.clone()])
                        });

                    self.pending_remove_fi = Some(fi);
                    self.previous_mode = Some(self.mode.clone());
                    self.mode = AppMode::ConfirmRemoveFile;

                    if affected_files.len() <= 1 {
                        self.message = Some(format!(
                            "Remove '{}' from config? (y/n)\n(file won't be deleted)",
                            raw_pattern
                        ));
                    } else {
                        let other_files: Vec<String> = affected_files
                            .iter()
                            .filter(|p| p.as_path() != resolved_path)
                            .map(|p| format!("  {}", p.display()))
                            .collect();
                        self.message = Some(format!(
                            "Remove '{}' from config? (y/n)\nAlso removes:\n{}\n(files won't be deleted)",
                            raw_pattern,
                            other_files.join("\n")
                        ));
                    }
```

- [ ] **Step 2: Update ConfirmRemoveFile handler for multi-file removal**

At `src/tui/app.rs:529-559`, replace the `ConfirmRemoveFile` handler:

```rust
                    AppMode::ConfirmRemoveFile => {
                        if let Some(fi) = self.pending_remove_fi.take() {
                            let path = self.profile.files[fi].path.clone();

                            let shell_key = self.shell_key.clone();
                            if let Some(files_config) = self.config.files.get_mut(&shell_key) {
                                // Find all files affected by the matching config pattern
                                let affected_paths: Vec<std::path::PathBuf> = files_config
                                    .paths
                                    .iter()
                                    .filter(|p| {
                                        let resolved =
                                            crate::config::path_resolver::resolve_paths(&[p.to_string()]);
                                        resolved.iter().any(|(rp, _)| *rp == path)
                                    })
                                    .flat_map(|p| {
                                        crate::config::path_resolver::resolve_paths(&[p.to_string()])
                                            .into_iter()
                                            .map(|(rp, _)| rp)
                                    })
                                    .collect();

                                // Remove the matching pattern from config
                                files_config.paths.retain(|p| {
                                    let resolved =
                                        crate::config::path_resolver::resolve_paths(&[p.to_string()]);
                                    !resolved.iter().any(|(rp, _)| *rp == path)
                                });

                                if let Err(e) = self.config.save() {
                                    self.message = Some(format!("Config save error: {}", e));
                                } else {
                                    // Remove ALL affected files from profile
                                    let removed_count = if affected_paths.is_empty() {
                                        self.profile.files.remove(fi);
                                        1
                                    } else {
                                        let before = self.profile.files.len();
                                        self.profile
                                            .files
                                            .retain(|f| !affected_paths.contains(&f.path));
                                        before - self.profile.files.len()
                                    };

                                    // Recalculate file_index for remaining entries
                                    for (new_fi, file) in
                                        self.profile.files.iter_mut().enumerate()
                                    {
                                        for entry in &mut file.entries {
                                            entry.file_index = new_fi;
                                        }
                                    }

                                    self.selection.clear();
                                    self.rebuild_list();
                                    if removed_count > 1 {
                                        self.message = Some(format!(
                                            "Removed {} files from config",
                                            removed_count
                                        ));
                                    } else {
                                        self.message = Some("Removed from config".into());
                                    }
                                }
                            }
                        }
                        self.mode = AppMode::Normal;
                        self.previous_mode = None;
                    }
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add src/tui/app.rs
git commit -m "feat: show raw config pattern and affected files in delete prompt

When deleting a config file path, the prompt now shows the original
config pattern (with ~ or $HOME) instead of the resolved path. For
glob patterns matching multiple files, lists all affected files.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 8: Update confirm popup to support multi-line messages

**Files:**
- Modify: `src/tui/ui.rs:428-454`

- [ ] **Step 1: Replace the confirm popup renderer**

At `src/tui/ui.rs:428-454`, replace `draw_confirm_popup`:

```rust
fn draw_confirm_popup(f: &mut Frame, area: Rect, app: &TuiApp) {
    let msg = app.message.as_deref().unwrap_or("Confirm? (y/n)");

    let lines: Vec<&str> = msg.split('\n').collect();
    let max_line_width = lines.iter().map(|l| l.len()).max().unwrap_or(20);

    let popup_width = ((max_line_width as u16) + 4).min(area.width - 4);
    let popup_height = ((lines.len() as u16) + 2).min(area.height - 2);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let title = match &app.mode {
        AppMode::ConfirmDelete => " Confirm Delete ",
        AppMode::ConfirmQuit => " Unsaved Changes ",
        _ => " Confirm ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let text_lines: Vec<Line> = lines.iter().map(|l| Line::from(*l)).collect();
    let text = Paragraph::new(text_lines)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(text, popup_area);
}
```

- [ ] **Step 2: Verify build**

Run: `cargo check`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/tui/ui.rs
git commit -m "feat: support multi-line messages in confirm popup

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 9: Update help text

**Files:**
- Modify: `src/tui/ui.rs:579` (undo help line)

- [ ] **Step 1: Update help text**

At `src/tui/ui.rs:579`, replace:
```rust
        Line::from("  z           Undo last operation"),
```
with:
```rust
        Line::from("  z           Undo (multi-step)"),
        Line::from("  y           Redo"),
```

- [ ] **Step 2: Verify build**

Run: `cargo check`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/tui/ui.rs
git commit -m "docs: update help text with multi-step undo and redo

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 10: Final validation

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: no warnings related to our changes

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: no formatting issues

- [ ] **Step 4: Verify no remaining references to old undo_snapshot**

Run: `grep -rn "undo_snapshot" src/`
Expected: no output

- [ ] **Step 5: Squash WIP commits if desired, or leave as-is**

The Task 2 commit is marked WIP since it intentionally breaks compilation (fixed in Task 3-4). Consider squashing Tasks 2-4 into a single commit:

```bash
git rebase -i HEAD~8  # squash the WIP commit with its fixes
```
