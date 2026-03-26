# TUI Enhancements Design Spec

**Date**: 2026-03-26
**Status**: Draft
**Scope**: 11 features covering key rebinding, new operations (copy/remark), parser improvements, config management, startup validation, and platform fixes.

---

## Problem Statement

The wenv TUI needs several enhancements to improve usability, correctness, and cross-platform support:

1. No startup validation of configured file paths (existence, permissions)
2. Key bindings don't follow common editor conventions
3. No copy operation (only cut)
4. No way to manage config file paths from within the TUI
5. Line numbers become stale after entry mutations
6. Editing an entry can only produce one entry (discards multi-entry content)
7. Adjacent code lines aren't merged (noisy display)
8. No comment/uncomment toggle
9. New entries on FileHeaders append to end instead of beginning
10. Windows `$PROFILE` variable not expanded outside PowerShell sessions

## Implementation Order

Based on dependency analysis:

```
#2 (key rebind) → #3 (copy) → #10 (insert top) → #11 ($PROFILE fix)
→ #6 (line recalc) → #8 (code merge) → #7 (multi-parse edit)
→ #1 (startup check) → #4 (remove file from config) → #5 (add file to config)
→ #9 (remark toggle)
```

Dependency graph:
```
#2 ─── base for all new key bindings
#3 ─── independent (uses clipboard)
#10 ── independent (insert_pos change)
#11 ── independent (path_resolver fix)
#6 ─── required by #7, #8, #9
#8 ─── affects parser output for #7, #9
#7 ─── required by #9 (multi-entry replacement)
#1 ─── independent but needs writable field
#4 ─── needs config save/reload mechanism (shared with #5)
#5 ─── needs TextInput mode + config mechanism
#9 ─── depends on #6, #7; affected by #8
```

## Strategy

**Approach: Shared infrastructure first, then features.**

Build 4 core infrastructure pieces before feature work:
1. `ProfileFile::recalculate_line_numbers()` — line tracking
2. `replace_entry_with_parsed()` — multi-entry replacement
3. `TextInputState` + `AppMode::TextInput` — TUI text input
4. `check_writable()` — file permission checking

---

## Infrastructure Design

### 1a. Line Number Recalculation

**File**: `src/model/profile.rs`

New method on `ProfileFile`:

```rust
impl ProfileFile {
    /// Recalculate line_number, end_line, and name for all entries.
    /// Call after any mutation (add, delete, move, paste, edit, remark).
    pub fn recalculate_line_numbers(&mut self) {
        let mut current_line = 1usize;
        for entry in &mut self.entries {
            let line_count = entry.value.split('\n').count();
            entry.line_number = Some(current_line);
            let end = current_line + line_count - 1;
            entry.end_line = if end > current_line { Some(end) } else { entry.line_number };

            // Update Code/Comment display names to reflect new line positions
            match entry.entry_type {
                EntryType::Comment => {
                    entry.name = if end > current_line {
                        format!("#L{}-L{}", current_line, end)
                    } else {
                        format!("#L{}", current_line)
                    };
                }
                EntryType::Code => {
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
}
```

**Value format**: Entry values use separator format (N lines = N-1 `\n`). `split('\n').count()` correctly gives the number of lines. When written to file, each entry's value gets an additional `\n` terminator.

**Call sites**: Every function in `operations.rs` that mutates entries, plus `run_edit_entry()`, `run_add_entry()`, `execute_move()`, and the new remark handler.

### 1b. Multi-Entry Replacement

**File**: `src/tui/operations.rs`

```rust
/// Replace a single entry with zero or more parsed entries at the same position.
/// Returns the number of new entries inserted.
/// If new_entries is empty, the original entry is deleted.
pub fn replace_entry_with_parsed(
    file: &mut ProfileFile,
    entry_index: usize,
    new_entries: Vec<Entry>,
    file_index: usize,
) -> usize {
    // Remove original entry
    file.entries.remove(entry_index);

    // Insert new entries at the same position
    let count = new_entries.len();
    for (i, mut entry) in new_entries.into_iter().enumerate() {
        entry.file_index = file_index;
        file.entries.insert(entry_index + i, entry);
    }

    file.dirty = true;
    count
}
```

### 1c. Text Input State

**File**: `src/tui/state.rs`

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

New app mode in `AppMode`:
```rust
pub enum AppMode {
    // ... existing variants ...
    TextInput,           // Generic text input mode
    ConfirmRemoveFile,   // Confirm removing file from config
    ConfirmCreateFile,   // Confirm creating a missing file
}
```

New key mapping in `keys.rs`:
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

New `Action` variants: `TextInputChar(char)`, `TextInputBackspace`, `TextInputLeft`, `TextInputRight`.

### 1d. Writable Check Utility

**File**: `src/utils/path.rs`

```rust
/// Check if a file path is writable.
/// For existing files: check write permission.
/// For non-existent files: check parent directory write permission.
pub fn check_writable(path: &Path) -> bool {
    if path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = path.metadata() {
                let mode = meta.mode();
                let uid = meta.uid();
                let gid = meta.gid();
                let euid = unsafe { libc::geteuid() };
                let egid = unsafe { libc::getegid() };
                if euid == 0 { return true; } // root
                if euid == uid { return mode & 0o200 != 0; }
                if egid == gid { return mode & 0o020 != 0; }
                return mode & 0o002 != 0;
            }
            false
        }
        #[cfg(not(unix))]
        {
            // Windows: try opening for write
            std::fs::OpenOptions::new().write(true).open(path).is_ok()
        }
    } else {
        // Check parent directory
        path.parent().map_or(false, |p| {
            p.exists() && check_writable(p)
        })
    }
}
```

---

## Feature Designs

### Feature #2: Key Rebinding

**File**: `src/tui/keys.rs`

Changes to `map_normal_key()`:

Removed bindings:
```
Char('a') => Action::Add         // REMOVED (moved to 'n')
Char('p') => Action::Paste       // REMOVED (moved to 'v')
Char('u') => Action::Undo        // REMOVED (moved to 'z')
```

New/changed bindings:
```
Char('n') => Action::Add         // was 'a'
Char('v') => Action::Paste       // was 'p'
Char('z') => Action::Undo        // was 'u'
Char('c') => Action::Copy        // NEW
Char('r') => Action::Remark      // NEW
Char('a') => Action::AddFile     // NEW (repurposed)
```

Unchanged bindings (kept as-is):
```
Char('e') => Action::Edit
Char('d') => Action::Delete
Char('s') => Action::ToggleSelect
Char('x') => Action::Cut
Char('m') => Action::StartMove
Char('/') => Action::Search
Char('?') => Action::Help
Char('w') => Action::Save
Char('q') => Action::Quit
```

New `Action` enum variants: `Copy`, `Remark`, `AddFile`.

**File**: `src/tui/ui.rs` — Update help screen key descriptions.
**File**: `CLAUDE.md` — Update TUI Key Bindings Reference table to reflect all changes.

### Feature #3: Copy Operation

**File**: `src/tui/app.rs`

```rust
Action::Copy => {
    let targets = self.get_operation_targets();
    if !targets.is_empty() {
        let copied: Vec<Entry> = targets.iter()
            .filter_map(|&idx| match self.visible_items.get(idx) {
                Some(ListItem::Entry(fi, ei)) =>
                    Some(self.profile.files[*fi].entries[*ei].clone()),
                _ => None,
            })
            .collect();
        let count = copied.len();
        self.clipboard.entries = copied;
        self.message = Some(format!("Copied {} entries", count));
    }
}
```

No undo snapshot needed. No dirty flag change. Clipboard is shared with cut — paste works the same way for both.

### Feature #10: New Entry at File Beginning

**File**: `src/tui/app.rs`, `run_add_entry()` method.

Change insert position logic:
```rust
// Before:
// _ => self.profile.files[fi].entries.len(),  // at end

// After:
Some(ListItem::FileHeader(_)) => 0,  // at beginning of file
```

### Feature #11: Windows $PROFILE Expansion

**File**: `src/config/path_resolver.rs`

Modify `expand_env_vars()` to handle `$PROFILE` specially:

```rust
pub fn expand_env_vars(path: &str) -> String {
    let mut result = path.to_string();
    let re = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in re.captures_iter(path) {
        let var_name = &cap[1];
        if let Ok(val) = std::env::var(var_name) {
            result = result.replace(&cap[0], &val);
        } else if var_name == "PROFILE" {
            // PowerShell $PROFILE not available in current env — query PowerShell
            if let Some(val) = query_powershell_profile() {
                result = result.replace(&cap[0], &val);
            }
        }
    }
    result
}

fn query_powershell_profile() -> Option<String> {
    // Try pwsh first, then powershell
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

### Feature #6: Line Number Auto-Recalculation

See Infrastructure 1a above for the `recalculate_line_numbers()` method.

**Call site additions**:

In `operations.rs`:
- `delete_entries()`: After deletion loop, call `profile.files[fi].recalculate_line_numbers()` for each affected file
- `paste_entries()`: After insertion, call on target file
- `save_dirty_files()`: Before writing, call on each dirty file (safety net)

In `app.rs`:
- `execute_move()`: After move, call on both source and target files
- `run_edit_entry()`: After entry replacement
- `run_add_entry()`: After insertion
- Remark handler: After toggle

### Feature #8: Adjacent Code Entry Merging

**File**: `src/parser/pending.rs`

Add tracking field to `PendingBlock`:
```rust
pub struct PendingBlock {
    // ... existing fields ...
    pub has_absorbed_blanks: bool,  // NEW: tracks if any blank lines were absorbed
}
```

Initialize to `false` in all factory methods. Set to `true` when a blank line is absorbed by `CodeWithBlanks`.

**File**: `src/parser/bash/mod.rs`

Modify the pending entry handling for `CodeWithBlanks`:

Current behavior:
- Code line → new pending `CodeWithBlanks`
- Next code line → flush pending, start new pending (each code line = separate entry)

New behavior:
```
Code line → new pending CodeWithBlanks (has_absorbed_blanks = false)

Next line is Code:
  if !has_absorbed_blanks → add to pending (merge adjacent code)
  if has_absorbed_blanks  → flush pending, start new pending (blank gap = separator)

Next line is Blank:
  → absorb into pending, set has_absorbed_blanks = true

Next line is Comment/Structured:
  → flush pending, handle new line normally
```

**Interaction with existing merge logic**: Code entries are NOT structured entries (Alias/Function/EnvVar/Source), so they don't trigger `merge_pending_with_structured()`. The comment merge logic is unaffected. A single comment line pending + next Code line → the comment flushes as its own entry (existing behavior, unchanged).

### Feature #7: Multi-Entry Parse from Editor

**File**: `src/tui/app.rs`, `run_edit_entry()` method.

Replace current single-entry logic:

```rust
// Current (take only first entry):
if let Some(new_entry) = parsed.entries.into_iter().next() { ... }

// New (use all parsed entries):
let new_entries: Vec<_> = parsed.entries.into_iter()
    .map(|mut e| { e.file_index = fi; e })
    .collect();

if new_entries.is_empty() {
    // Empty edit = delete entry
    self.profile.files[fi].entries.remove(ei);
} else {
    operations::replace_entry_with_parsed(
        &mut self.profile.files[fi], ei, new_entries, fi
    );
}
self.profile.files[fi].recalculate_line_numbers();
self.profile.files[fi].dirty = true;
self.rebuild_list();
```

### Feature #1: Startup File Path Validation

**File**: `src/main.rs`

New function called between `load_shell_profile()` and `TuiApp::new()`:

```rust
fn startup_file_check(
    profile: &mut ShellProfile,
    config: &mut Config,
    shell_key: &str,
) -> Result<()> {
    let mut paths_to_remove: Vec<PathBuf> = Vec::new();

    for file in &mut profile.files {
        if !file.exists {
            println!("File not found: {}", file.path.display());
            let create = dialoguer::Confirm::new()
                .with_prompt("Create this file?")
                .default(true)
                .interact()?;

            if create {
                // Ensure parent directory exists
                if let Some(parent) = file.path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::File::create(&file.path) {
                    Ok(_) => {
                        file.exists = true;
                        file.content = String::new();
                        println!("  Created: {}", file.path.display());
                    }
                    Err(e) => {
                        eprintln!("  Failed to create: {}", e);
                        let remove = dialoguer::Confirm::new()
                            .with_prompt("Remove this path from config?")
                            .default(false)
                            .interact()?;
                        if remove {
                            paths_to_remove.push(file.path.clone());
                        }
                    }
                }
            }
        }
    }

    // Remove paths from config and profile
    if !paths_to_remove.is_empty() {
        if let Some(files_config) = config.files.get_mut(shell_key) {
            files_config.paths.retain(|p| {
                let expanded = path_resolver::expand_env_vars(
                    &path_resolver::expand_tilde(p)
                );
                !paths_to_remove.iter().any(|r| r == &PathBuf::from(&expanded))
            });
        }
        config.save()?;
        profile.files.retain(|f| !paths_to_remove.contains(&f.path));
    }

    // Check write permissions for all remaining files
    for file in &mut profile.files {
        if file.exists {
            file.writable = crate::utils::path::check_writable(&file.path);
        } else {
            file.writable = false;
        }
    }

    Ok(())
}
```

**New field**: `ProfileFile::writable: bool` (default `true`).

**UI changes** (`src/tui/ui.rs`):
- If `!file.writable`: render FileHeader and all its entries in `Color::DarkGray`
- No other style changes needed (greyed out = read-only visual cue)

**Operation blocking** (`src/tui/app.rs`):
- In handlers for Edit, Add, Delete, Cut, Paste, Move, Remark:
  - Check if target file is `writable`
  - If not, set `self.message = Some("File is read-only")` and return

### Feature #4: Remove File from Config (d on FileHeader)

**File**: `src/tui/app.rs`

Modify `Action::Delete` handler:

```rust
Action::Delete => {
    if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
        let fi = *fi;
        self.pending_remove_fi = Some(fi);  // new field on TuiApp
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::ConfirmRemoveFile;
        self.message = Some(format!(
            "Remove '{}' from config? (y/n)",
            self.profile.files[fi].display_name()
        ));
    } else {
        // Original entry deletion logic (unchanged)
        let targets = self.get_operation_targets();
        // ...
    }
}
```

In `Action::Confirm` for `AppMode::ConfirmRemoveFile`:
```rust
AppMode::ConfirmRemoveFile => {
    if let Some(fi) = self.pending_remove_fi.take() {
        // Remove from config
        let path = self.profile.files[fi].path.clone();
        remove_path_from_config(&mut self.config, &self.shell_key, &path);
        self.config.save().ok();

        // Remove from profile
        self.profile.files.remove(fi);

        // Fix file_index for remaining files' entries
        for (new_fi, file) in self.profile.files.iter_mut().enumerate() {
            for entry in &mut file.entries {
                entry.file_index = new_fi;
            }
        }

        self.selection.clear();
        self.rebuild_list();
        self.mode = AppMode::Normal;
        self.previous_mode = None;
        self.message = Some(format!("Removed from config"));
    }
}
```

**Note**: TuiApp needs access to `Config` and `shell_key`. Add these as fields:
```rust
pub struct TuiApp {
    // ... existing fields ...
    pub config: Config,
    pub shell_key: String,
}
```

### Feature #5: Add File Path to Config (a key)

**File**: `src/tui/app.rs`

```rust
Action::AddFile => {
    self.text_input = Some(TextInputState {
        prompt: "New file path: ".into(),
        value: String::new(),
        cursor_pos: 0,
        purpose: InputPurpose::AddFilePath,
    });
    self.mode = AppMode::TextInput;
}
```

Text input key handling dispatches to `map_text_input_key()`.

On confirm (Enter in TextInput mode):
```rust
AppMode::TextInput => {
    if let Some(input) = self.text_input.take() {
        match input.purpose {
            InputPurpose::AddFilePath => {
                let raw_path = input.value.trim().to_string();
                if raw_path.is_empty() {
                    self.mode = AppMode::Normal;
                    return Ok(EditorRequest::None);
                }

                let expanded = path_resolver::expand_env_vars(
                    &path_resolver::expand_tilde(&raw_path)
                );
                let path = PathBuf::from(&expanded);

                // Check duplicate
                if self.profile.files.iter().any(|f| f.path == path) {
                    self.message = Some("Path already in config".into());
                    self.mode = AppMode::Normal;
                    return Ok(EditorRequest::None);
                }

                if !path.exists() {
                    // Prompt to create
                    self.pending_create_path = Some((raw_path, path));
                    self.mode = AppMode::ConfirmCreateFile;
                    self.message = Some("File doesn't exist. Create? (y/n)".into());
                } else {
                    self.add_file_to_config_and_profile(raw_path, path)?;
                    self.mode = AppMode::Normal;
                }
            }
        }
    }
}
```

`add_file_to_config_and_profile()`:
```rust
fn add_file_to_config_and_profile(&mut self, raw_path: String, path: PathBuf) -> Result<()> {
    // Add to config
    let files_config = self.config.files
        .entry(self.shell_key.clone())
        .or_insert_with(|| FilesConfig { paths: vec![] });
    files_config.paths.push(raw_path);
    self.config.save()?;

    // Parse and add to profile
    let fi = self.profile.files.len();
    let exists = path.exists();
    let content = if exists {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    let parser = crate::parser::get_parser(self.profile.shell_type);
    let parsed = parser.parse(&content);
    let entries: Vec<_> = parsed.entries.into_iter()
        .map(|mut e| { e.file_index = fi; e })
        .collect();

    let mut file = ProfileFile::new(path, exists);
    file.entries = entries;
    file.content = content;
    file.expanded = true;
    file.writable = crate::utils::path::check_writable(&file.path);
    self.profile.files.push(file);

    self.rebuild_list();
    self.message = Some("File added to config".into());
    Ok(())
}
```

On `ConfirmCreateFile` confirm:
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
        self.mode = AppMode::Normal;
    }
}
```

UI rendering: show text input bar at bottom (same position as search bar).

### Feature #9: Remark Toggle

**File**: `src/tui/app.rs`

```rust
Action::Remark => {
    let targets = self.get_operation_targets();
    if targets.is_empty() { return Ok(EditorRequest::None); }

    // Check writable for all target files
    let target_file_indices: HashSet<usize> = targets.iter()
        .filter_map(|&idx| match self.visible_items.get(idx) {
            Some(ListItem::Entry(fi, _)) => Some(*fi),
            _ => None,
        })
        .collect();
    if target_file_indices.iter().any(|&fi| !self.profile.files[fi].writable) {
        self.message = Some("Cannot remark: file is read-only".into());
        return Ok(EditorRequest::None);
    }

    // Determine mode: all Comment → uncomment; otherwise → comment non-Comments
    let all_comment = targets.iter().all(|&idx| {
        matches!(self.visible_items.get(idx),
            Some(ListItem::Entry(fi, ei))
            if self.profile.files[*fi].entries[*ei].entry_type == EntryType::Comment
        )
    });

    self.undo_snapshot = Some(take_snapshot(&self.profile));

    // Track selection range for post-operation restoration
    let first_target = targets[0];
    let last_target = *targets.last().unwrap();

    if all_comment {
        // === UNCOMMENT ===
        // Process in reverse order to maintain stable indices
        let mut total_new = 0usize;
        let entry_targets: Vec<(usize, usize)> = targets.iter().rev()
            .filter_map(|&idx| match self.visible_items.get(idx) {
                Some(ListItem::Entry(fi, ei)) => Some((*fi, *ei)),
                _ => None,
            })
            .collect();

        for (fi, ei) in entry_targets {
            let value = &self.profile.files[fi].entries[ei].value;
            let uncommented = uncomment_value(value);
            let parser = crate::parser::get_parser(self.profile.shell_type);
            let parsed = parser.parse(&uncommented);
            let new_entries: Vec<_> = parsed.entries.into_iter()
                .map(|mut e| { e.file_index = fi; e })
                .collect();
            let count = operations::replace_entry_with_parsed(
                &mut self.profile.files[fi], ei, new_entries, fi
            );
            total_new += count;
            self.profile.files[fi].recalculate_line_numbers();
        }
    } else {
        // === COMMENT non-Comment entries ===
        for &idx in &targets {
            if let Some(ListItem::Entry(fi, ei)) = self.visible_items.get(idx) {
                let fi = *fi;
                let ei = *ei;
                if self.profile.files[fi].entries[ei].entry_type != EntryType::Comment {
                    let value = &self.profile.files[fi].entries[ei].value;
                    let commented = comment_value(value);
                    let entry = &mut self.profile.files[fi].entries[ei];
                    entry.value = commented;
                    entry.entry_type = EntryType::Comment;
                    // name will be updated by recalculate_line_numbers
                    self.profile.files[fi].dirty = true;
                }
            }
        }
        for &fi in &target_file_indices {
            self.profile.files[fi].recalculate_line_numbers();
        }
    }

    // Rebuild and restore selection
    self.rebuild_list();
    // Re-select the range from first_target to however many entries now exist
    // in that range (may have grown due to uncomment)
    self.restore_selection_range(first_target, last_target);
    self.message = Some(if all_comment { "Uncommented" } else { "Commented" }.into());

    Ok(EditorRequest::None)
}
```

**Helper functions** (new file or in `operations.rs`):

```rust
/// Remove leading "# " or "#" from non-blank lines.
fn uncomment_value(value: &str) -> String {
    value.split('\n')
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

/// Add "# " to all non-blank lines (including already-commented lines).
fn comment_value(value: &str) -> String {
    value.split('\n')
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
```

**Selection restoration**: After remark, rebuild visible_items, then select all entries from the original first target position through the equivalent ending position (accounting for any entries added by uncomment).

```rust
fn restore_selection_range(&mut self, orig_first: usize, orig_last: usize) {
    // The entries between orig_first and orig_last may have expanded.
    // We track by file_index: find all entries in the affected range.
    self.selection.clear();
    // Simple approach: select from orig_first to orig_first + (new_count - 1)
    // where new_count = number of entries now in the positions that were targets
    let end = (orig_first + (self.visible_items.len().saturating_sub(1)))
        .min(self.visible_items.len().saturating_sub(1));
    // More precise: track by file_index boundaries
    // Implementation detail to be refined during coding
}
```

---

## Data Model Changes Summary

### ProfileFile
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

### TuiApp
```rust
pub struct TuiApp {
    // ... existing fields ...
    pub config: Config,                    // NEW: for runtime config modification
    pub shell_key: String,                 // NEW: current shell's config key
    pub text_input: Option<TextInputState>, // NEW: text input state
    pub pending_remove_fi: Option<usize>,  // NEW: file index pending removal
    pub pending_create_path: Option<(String, PathBuf)>, // NEW: path pending creation
}
```

### AppMode
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

### Action
```rust
pub enum Action {
    // ... existing variants ...
    Copy,               // NEW
    Remark,             // NEW
    AddFile,            // NEW
    TextInputChar(char), // NEW
    TextInputBackspace, // NEW
    TextInputLeft,      // NEW
    TextInputRight,     // NEW
}
```

### PendingBlock
```rust
pub struct PendingBlock {
    // ... existing fields ...
    pub has_absorbed_blanks: bool,  // NEW: for adjacent code merging
}
```

---

## Key Binding Summary (After Changes)

| Key | Action | Context |
|-----|--------|---------|
| `j`/`k`, `↑`/`↓` | Navigate | All modes |
| `Space` | Toggle selection | Normal |
| `Shift+↑`/`↓` | Range select | Normal |
| `Enter` | Edit / Confirm | Normal / Popups |
| `e` | Edit entry/file | Normal |
| `n` | New entry (was `a`) | Normal |
| `d` | Delete entry / Remove file from config | Normal (context-dependent) |
| `x` | Cut | Normal |
| `c` | Copy (NEW) | Normal |
| `v` | Paste (was `p`) | Normal |
| `m` | Move mode | Normal |
| `Tab` | Toggle expand | Normal |
| `z` | Undo (was `u`) | Normal |
| `/` | Search | Normal |
| `r` | Remark toggle (NEW) | Normal |
| `a` | Add file path (NEW) | Normal |
| `Esc` | Cancel / Clear selection | All modes |
| `w` / `Ctrl+s` | Save | Normal |
| `?` | Help | Normal |
| `q` | Quit | Normal |

---

## Edge Cases and Risk Mitigations

### Remark on merged entries
A merged entry like `"# comment\nalias foo='bar'"` has type `Alias`, not `Comment`. Pressing `r` will add `# ` to all non-blank lines, turning it into a Comment. This is correct behavior — the entire entry becomes commented.

### Uncomment producing unexpected entries
If a user manually wrote `# alias foo='bar'\n# export PATH=...` as a single Comment entry, uncommenting will parse into two entries (Alias + EnvVar). This is expected and desired.

### Multi-entry edit followed by undo
The undo snapshot captures the entire file state before editing. Restoring it correctly reverts multi-entry replacements back to the original single entry.

### Adjacent code merge + existing tests
The parser change affects test expectations. Any test that expects individual Code entries for adjacent lines will need updating to expect merged entries.

### File removal with entries selected
If user has entries selected in a file they're about to remove, selection should be cleared during the removal operation.

### TextInput with special characters
Path input must handle: `/`, `~`, `.`, `$`, spaces. All printable characters should be accepted.

### Race condition on file creation
Between checking existence and creating the file, another process could create it. Use `OpenOptions::create_new()` or handle the "already exists" case gracefully.

### Dependencies
- `libc` crate needed for Unix permission checks in `check_writable()` (Feature #1). Alternative: use simpler `std::fs::OpenOptions::write(true).open()` test on all platforms to avoid the `libc` dependency.
- `dialoguer` is already a dependency (used in `src/cli/actions/source.rs`), so Feature #1's startup prompts are covered.

### Documentation
After all features are implemented, update `CLAUDE.md`'s TUI Key Bindings Reference table to match the new bindings.
