# TUI Enhancements Batch 2 — Design Spec

## Overview

Four TUI improvements: FileHeader move mode, info popup actions, remark selection fix, and move-mode blocked-file skipping.

---

## Feature 1: FileHeader Move Mode

**Trigger:** Press `m` while cursor is on a `FileHeader` item.

**New state:** `AppMode::MovingFile` with `FileMovingState`:
```rust
pub struct FileMovingState {
    pub original_fi: usize,              // file index being moved
    pub insertion_cursor: usize,         // visible-list index (FileHeader) for drop target
    pub saved_expanded: Vec<bool>,       // original expanded state per file
}
```

**Enter behavior:**
1. Save each file's `expanded` state into `saved_expanded`.
2. Collapse all files (only FileHeaders visible).
3. Set `insertion_cursor` to the current cursor position.
4. Display message: "File move: ↑↓ to position, Enter to drop, Esc to cancel".

**Navigation:**
- `↑`/`↓`/`k`/`j`: Move `insertion_cursor` among FileHeader items (since all collapsed, every visible item is a FileHeader).
- `PageUp`/`PageDown`: Jump by half visible height.

**Confirm (Enter):**
1. Determine `target_fi` from `insertion_cursor` (the FileHeader index).
2. If `target_fi == original_fi`: no-op, restore expanded states, exit.
3. Swap the file's position in both `config.files[shell_key].paths` and `profile.files`.
4. Fix `file_index` on all entries in all files.
5. Save config to disk (`config.save()`).
6. Restore original expanded states (mapped to new positions).
7. Rebuild visible list, set cursor to moved file's new FileHeader position.
8. Exit to `AppMode::Normal`.

**Cancel (Esc):**
1. Restore `saved_expanded` states.
2. Rebuild visible list.
3. Set cursor to `original_fi`'s FileHeader.
4. Exit to `AppMode::Normal`.

**Key mapping:** Reuse `map_moving_key()` since navigation keys are identical.

---

## Feature 2: Info Popup (ShowingDetail) Actions

**Modify `map_detail_key()`** to add:
- `e` → `Action::Edit`
- `r` → `Action::Remark`

### `e` (Edit) in Detail mode:
1. Close Detail popup: set `mode = previous_mode`.
2. Check writable; if not, show "File is read-only", stay in previous_mode.
3. Return `EditorRequest::EditEntry(fi, ei)` to trigger normal edit flow.
4. After editor returns, if `previous_mode` was `Searching`, update search results.

### `r` (Remark) in Detail mode:
1. **Do NOT close Detail popup.** Stay in `ShowingDetail`.
2. Execute remark toggle on the single entry at cursor `(fi, ei)`.
3. The popup re-renders next frame showing updated value/type/name.
4. If file not writable: show "File is read-only", remain in Detail.
5. Take undo snapshot before toggling.

### Search mode support:
- `previous_mode` tracks `Searching` state.
- `e`: after edit, returns to Searching and calls `update_search_and_navigate()`.
- `r`: stays in Detail; search updates when Detail is eventually closed.

---

## Feature 3: Remark Selection State Fix

**Problem:** `Action::Remark` always selects the toggled entries after operation, even when no entries were originally selected.

**Fix:** Before executing remark logic, capture `let had_selection = !self.selection.is_empty()`.

After remark toggle completes:
- If `had_selection`: execute existing selection restoration logic (select new entries).
- If `!had_selection`: only call `self.selection.clear()`, do NOT toggle-select new entries.

This applies to both comment and uncomment code paths.

---

## Feature 4: Move Mode Skips Blocked Files

**Definition:** A file is "blocked" if `!file.exists || !file.writable`.

**New helper:** `fn is_file_blocked(&self, fi: usize) -> bool`

### Move cursor skipping:
In `AppMode::Moving` navigation (NavigateUp/Down/PageUp/PageDown):
- After computing tentative new `insertion_cursor`, check if the target position belongs to a blocked file.
- If blocked: continue in the same direction to find the next non-blocked position.
- If no non-blocked position found in that direction: keep `insertion_cursor` unchanged and show "No writable file to move to".

### Boundary handling:
- If first/last file is blocked, cursor skips past it.
- If all reachable files in one direction are blocked, cursor stays put with message.
- Source file is already validated writable by `is_current_file_writable()` check in `StartMove`.

---

## Files to modify

| File | Changes |
|------|---------|
| `src/tui/state.rs` | Add `MovingFile` variant, `FileMovingState` struct |
| `src/tui/keys.rs` | Add `e`/`r` to `map_detail_key()`, handle `MovingFile` in `map_key()` |
| `src/tui/app.rs` | FileHeader move logic, Detail `e`/`r` handling, remark selection fix, blocked-file skipping |
| `src/tui/ui.rs` | Update detail popup title to show available keys, MovingFile rendering |
