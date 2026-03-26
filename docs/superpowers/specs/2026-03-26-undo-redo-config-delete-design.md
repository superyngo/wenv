# Multi-step Undo/Redo & Config Delete Prompt Improvements

## Problem

1. The undo system (`z` key) only supports a single snapshot — each new operation overwrites the previous one, making it impossible to undo multiple steps.
2. There is no redo capability — once you undo, you cannot redo.
3. When deleting a config file path (`d` on FileHeader), the confirmation prompt shows only the resolved display name, not the original config pattern (which may contain `$HOME`, `~`, or globs like `~/.bash*`). If a glob pattern matches multiple files, removing it silently drops all matched files without warning.

## Solution Overview

- Replace the single `Option<UndoSnapshot>` with a bounded `VecDeque<UndoSnapshot>` stack (max 20) and add a `Vec<UndoSnapshot>` redo stack.
- Add `y` key for redo with standard undo/redo semantics (new operations clear redo history).
- Before confirming config file removal, reverse-lookup the raw config pattern and preview all affected files.

---

## Feature 1: Multi-step Undo

### Data Structure Changes

**`src/tui/state.rs`** — `UndoSnapshot` struct unchanged:

```rust
pub struct UndoSnapshot {
    pub file_states: Vec<(PathBuf, String, Vec<Entry>, bool)>,
}
```

**`src/tui/app.rs`** — Replace in `TuiApp`:

```rust
// BEFORE:
pub undo_snapshot: Option<UndoSnapshot>,

// AFTER:
pub undo_stack: VecDeque<UndoSnapshot>,  // bounded, max 20
pub redo_stack: Vec<UndoSnapshot>,
```

### Operations Changes

**`src/tui/operations.rs`** — `take_snapshot()` remains unchanged. Add helper:

```rust
const MAX_UNDO_HISTORY: usize = 20;

pub fn push_undo(undo_stack: &mut VecDeque<UndoSnapshot>, redo_stack: &mut Vec<UndoSnapshot>, snapshot: UndoSnapshot) {
    if undo_stack.len() >= MAX_UNDO_HISTORY {
        undo_stack.pop_front();
    }
    undo_stack.push_back(snapshot);
    redo_stack.clear();
}
```

### Call Site Changes

All 5 locations in `app.rs` that currently do:

```rust
self.undo_snapshot = Some(take_snapshot(&self.profile));
```

Change to:

```rust
let snapshot = take_snapshot(&self.profile);
push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
```

### Undo Handler (`z` key)

```rust
Action::Undo => {
    if let Some(snapshot) = self.undo_stack.pop_back() {
        self.redo_stack.push(take_snapshot(&self.profile));
        restore_snapshot(&mut self.profile, snapshot);
        self.selection.clear();
        self.rebuild_list();
        let remaining = self.undo_stack.len();
        self.message = Some(format!("Undone ({remaining} left)"));
    } else {
        self.message = Some("Nothing to undo".into());
    }
}
```

---

## Feature 2: Redo (`y` key)

### Key Binding

**`src/tui/keys.rs`** — Add to normal mode mapping:

```rust
KeyCode::Char('y') => Action::Redo,
```

Note: `y` is already mapped to `Action::Confirm` in popup key mapping (`map_popup_key`), so there is no conflict — Redo only fires in Normal mode.

### Action Enum

**`src/tui/keys.rs`** — Add `Redo` variant to `Action` enum.

### Redo Handler

```rust
Action::Redo => {
    if let Some(snapshot) = self.redo_stack.pop() {
        // Push current state to undo_stack directly (bypass push_undo to avoid clearing redo_stack)
        self.undo_stack.push_back(take_snapshot(&self.profile));
        if self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
        restore_snapshot(&mut self.profile, snapshot);
        self.selection.clear();
        self.rebuild_list();
        let remaining = self.redo_stack.len();
        self.message = Some(format!("Redone ({remaining} left)"));
    } else {
        self.message = Some("Nothing to redo".into());
    }
}
```

### Redo Clearing Semantics

- Any "new operation" (the 5 snapshot-triggering operations) calls `push_undo()`, which clears `redo_stack`.
- Undo and Redo themselves do NOT clear redo — they transfer between stacks.
- This follows standard editor behavior (VS Code, Vim, etc.).

---

## Feature 3: Config Delete Prompt Improvements

### New Helper Function

**`src/tui/operations.rs`** (or new utility in `src/config/`):

```rust
/// Given a resolved file path, find the raw config pattern that matches it
/// and return all files that pattern resolves to.
pub fn find_matching_config_pattern(
    config: &Config,
    shell_key: &str,
    resolved_path: &Path,
) -> Option<(String, Vec<PathBuf>)> {
    let files_config = config.files.get(shell_key)?;
    for raw_pattern in &files_config.paths {
        let resolved = resolve_paths(&[raw_pattern.clone()]);
        if resolved.iter().any(|(p, _)| p == resolved_path) {
            let all_paths: Vec<PathBuf> = resolved.into_iter().map(|(p, _)| p).collect();
            return Some((raw_pattern.clone(), all_paths));
        }
    }
    None
}
```

### Prompt Construction

**`src/tui/app.rs`** — Modify `Action::Delete` for FileHeader:

```rust
Action::Delete => {
    if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
        let fi = *fi;
        let resolved_path = &self.profile.files[fi].path;

        let (raw_pattern, affected_files) = find_matching_config_pattern(
            &self.config, &self.shell_key, resolved_path
        ).unwrap_or_else(|| {
            (resolved_path.display().to_string(), vec![resolved_path.clone()])
        });

        self.pending_remove_fi = Some(fi);
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::ConfirmRemoveFile;

        if affected_files.len() <= 1 {
            // Single file — simplified prompt showing raw pattern
            self.message = Some(format!(
                "Remove '{}' from config? (y/n) (file won't be deleted)",
                raw_pattern
            ));
        } else {
            // Multiple files — show list of all affected files
            let other_files: Vec<String> = affected_files.iter()
                .filter(|p| p.as_path() != resolved_path)
                .map(|p| format!("  {}", p.display()))
                .collect();
            self.message = Some(format!(
                "Remove '{}' from config? (y/n)\nThis will also remove:\n{}\n(files won't be deleted)",
                raw_pattern,
                other_files.join("\n")
            ));
        }
    }
}
```

### Confirmation Handler Changes

The existing `retain()` logic in `ConfirmRemoveFile` handler already removes by comparing expanded paths, so it naturally removes the entire glob pattern. When multiple files are affected, all of them should also be removed from `self.profile.files`. The current logic only removes the single file at index `fi` — this needs to be updated to remove all files whose paths match the glob pattern's expansion.

```rust
AppMode::ConfirmRemoveFile => {
    if let Some(fi) = self.pending_remove_fi.take() {
        let path = self.profile.files[fi].path.clone();
        let shell_key = self.shell_key.clone();

        // Find and remove the matching config pattern
        if let Some(files_config) = self.config.files.get_mut(&shell_key) {
            // Find affected files before removing from config
            let affected_paths: Vec<PathBuf> = files_config.paths.iter()
                .filter(|p| {
                    let expanded = expand_env_vars(&expand_tilde(p));
                    let resolved = resolve_paths(&[p.clone().to_string()]);
                    resolved.iter().any(|(rp, _)| rp == &path)
                })
                .flat_map(|p| {
                    resolve_paths(&[p.clone()]).into_iter().map(|(rp, _)| rp)
                })
                .collect();

            // Remove the pattern from config
            files_config.paths.retain(|p| {
                let resolved = resolve_paths(&[p.clone()]);
                !resolved.iter().any(|(rp, _)| rp == &path)
            });

            if let Err(e) = self.config.save() {
                self.message = Some(format!("Config save error: {}", e));
            } else {
                // Remove ALL affected files from profile
                self.profile.files.retain(|f| !affected_paths.contains(&f.path));

                // Recalculate file_index
                for (new_fi, file) in self.profile.files.iter_mut().enumerate() {
                    for entry in &mut file.entries {
                        entry.file_index = new_fi;
                    }
                }

                let count = affected_paths.len();
                self.selection.clear();
                self.rebuild_list();
                if count > 1 {
                    self.message = Some(format!("Removed {} files from config", count));
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

### Multi-line Popup Rendering

The current confirmation popup in `ui.rs` renders `app.message` as a single line. For multi-file delete prompts, the popup needs to handle multi-line messages:

- Split message by `\n` and render each line
- Dynamically size the popup height based on line count
- Cap at a reasonable max height (e.g., 10 lines) with truncation if needed

---

## Help Text Updates

**`src/tui/keys.rs`**:

| Key | Before | After |
|-----|--------|-------|
| `z` | `"Undo"` | `"Undo (multi-step)"` |
| `y` | (none) | `"Redo"` |

---

## Test Plan

### Multi-step Undo Tests (tui_logic_tests)

1. **Basic multi-step**: 3 sequential deletes → undo 3 times → all entries restored in correct order
2. **Stack overflow**: Perform 21 operations → undo stack has exactly 20 → oldest operation lost
3. **Undo message**: Verify "Undone (N left)" message shows correct count

### Redo Tests (tui_logic_tests)

4. **Basic redo**: delete → undo → redo → state matches post-delete
5. **Multi-step redo**: 3 deletes → 3 undos → 3 redos → state matches after 3 deletes
6. **Redo cleared by new op**: delete → undo → new delete → redo shows "Nothing to redo"
7. **Interleaved**: delete → delete → undo → redo → undo → undo → verify each state

### Config Delete Prompt Tests

8. **Single file prompt**: Non-glob pattern → prompt shows raw pattern, no file list
9. **Glob multi-file prompt**: Glob pattern matching 3 files → prompt shows pattern + 2 other files
10. **Multi-file removal**: After confirming glob delete → all matched files removed from profile
11. **find_matching_config_pattern**: Unit test with various pattern types ($HOME, ~, glob)
