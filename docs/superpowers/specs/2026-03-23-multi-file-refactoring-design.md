# wenv Multi-File Refactoring Design

## Problem Statement

wenv is a ~14.3K LOC Rust CLI tool for managing shell configuration files. The current architecture is built around single-file operations, which limits its usefulness since shell environments typically span multiple configuration files (e.g., `/etc/profile`, `~/.bashrc`, `~/.bash_aliases`).

This refactoring simplifies the tool by removing underused features (import/export, syntax checker, formatter, backup) and rebuilds the TUI around a multi-file paradigm with external editor integration.

## Approach

**Partial rewrite (Approach B):** Preserve the battle-tested parser/formatter/model core (~5,000 LOC), rewrite the TUI from scratch (~4,300 LOC), refactor CLI and config, and remove ~2,500 LOC of features.

The parser subsystem represents the most complex and well-tested domain logic (control structure tracking, quote pairing, multi-line state machine) and must be preserved intact. The TUI, being fundamentally single-file in architecture, requires a ground-up rewrite for multi-file support.

---

## 1. Data Model

### 1.1 ShellProfile (new)

Top-level container for a shell session's configuration:

```rust
pub struct ShellProfile {
    pub shell_type: ShellType,
    pub files: Vec<ProfileFile>,
}
```

### 1.2 ProfileFile (new)

Represents one configuration file in the shell's file list:

```rust
pub struct ProfileFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub content: String,       // original file content
    pub expanded: bool,        // TUI toggle state
    pub dirty: bool,           // unsaved modifications
    pub exists: bool,          // file exists on disk
}
```

### 1.3 Entry (modified)

Existing `Entry` struct is preserved. A new field `file_index: usize` is added to track which `ProfileFile` owns this entry. All existing fields (`entry_type`, `name`, `value`, `line_number`, `end_line`) remain unchanged.

### 1.4 ListItem (new)

Unified flat-list index for TUI navigation:

```rust
pub enum ListItem {
    FileHeader(usize),           // file_index
    Entry(usize, usize),        // (file_index, entry_index)
}
```

All TUI operations (selection, movement, deletion) work against this flat list, making cross-file operations natural.

---

## 2. Config File List System

### 2.1 Config Location

All platforms use `~/.config/wenv/config.toml`. The macOS-specific `~/Library/Application Support/wenv/` path is removed.

### 2.2 Config Structure

```toml
[ui]
language = "en"

[files.bash]
paths = [
    "/etc/profile",
    "/etc/profile.d/*.sh",
    "~/.profile",
    "~/.bashrc",
    "~/.bash_aliases",
]

[files.zsh]
paths = [
    "/etc/zshenv",
    "/etc/zprofile",
    "/etc/zshrc",
    "~/.zshenv",
    "~/.zprofile",
    "~/.zshrc",
    "~/.zsh_aliases",
]

[files.powershell]
paths = [
    "$PROFILE",
]
```

### 2.3 Path Resolution Rules

- `~` expands to home directory
- `$VAR` expands environment variables (e.g., `$PROFILE`, `$ENV`, `$SHELL`)
- `*.sh` performs glob matching (e.g., `/etc/profile.d/*.sh`)
- Non-existent files remain in the list, displayed as dimmed/unavailable in the TUI

### 2.4 First-Run Flow

1. **No config.toml exists:**
   - Detect shell type at runtime
   - Prompt: "No config found. Create from template? (Y/n)"
   - Y: Generate `config.toml` with the detected shell's default file list template
   - n: Exit

2. **Config exists but missing `[files.<current_shell>]` section:**
   - Prompt: "No file list for {shell}. Add from template? (Y/n)"
   - Y: Append the shell's default `[files.<shell>]` section
   - n: Exit

### 2.5 Built-in Templates

Templates are embedded in the binary for bash, zsh, and powershell. The template system is designed to be extensible — adding a new shell requires defining its default paths and (optionally) implementing the Parser trait.

---

## 3. Shell Decision Logic

Shell type is determined at runtime without consulting config:

```
Priority:
1. CLI flag: -s/--shell <shell>
2. Platform: Windows → PowerShell
3. Environment: $SHELL → parse shell name
4. Fallback: Bash
```

Supported shell types (first phase): `bash`, `zsh`, `powershell`.

The `ShellType` enum and detection logic remain extensible for future shells (fish, sh, ksh).

---

## 4. CLI Design

### 4.1 Simplified Interface

```
wenv                     # Launch TUI (default)
wenv .                   # File selection menu → open chosen file in $EDITOR
wenv --source            # Same as 'wenv .'
wenv -s bash             # Launch TUI with specified shell
wenv --shell zsh         # Same as -s
wenv -c / --config       # Open wenv config.toml in $EDITOR
```

### 4.2 Removed Flags

| Flag | Reason for removal |
|------|-------------------|
| `-f, --file <FILE>` | Replaced by config file list |
| `-i, --import <SOURCE>` | Feature removed |
| `-e, --export <OUTPUT>` | Feature removed |
| `--on-conflict <STRATEGY>` | Import-only flag, removed with import |
| `-y, --yes` | Import-only flag, removed with import |
| `--clear-cache` | Cache system removed |
| `-t, --type <TYPE>` | Export-only flag, removed with export |

### 4.3 `wenv .` / `--source` Behavior

Displays the shell's file list using `dialoguer::Select`, with existence indicators:

```
Shell configuration files (bash):
  1. /etc/profile
  2. ~/.profile
  3. ~/.bashrc ✓
  4. ~/.bash_aliases (not found)

Select file to edit [1-4]:
```

Opens the selected file in `$EDITOR` (falls back to `vi` on Unix, `notepad` on Windows).

---

## 5. TUI Architecture

### 5.1 App Modes

```rust
pub enum AppMode {
    Normal,              // Browse file/entry list
    Searching,           // Fuzzy filter active
    ShowingDetail,       // Entry detail popup
    ShowingHelp,         // Help popup
    ConfirmDelete,       // Delete confirmation
    ConfirmQuit,         // Unsaved changes quit confirmation
    Moving,              // Drag-style move in progress
}
```

Removed modes: `Editing`, `SelectingType`, `ConfirmFormat`, `ConfirmSaveWithErrors`.

### 5.2 TUI Display Layout

```
📜 /etc/profile                    [2 entries] ▶  (collapsed)
📜 ~/.profile                      [5 entries] ▼  (expanded)
   alias ll='ls -la'               alias    L10
   export PATH="/usr/bin:$PATH"    env      L15
   my_func()                       func     L20
   source ~/.aliases               source   L30
   # setup script                  comment  L35
📜 ~/.bashrc                       [12 entries] ▼ (expanded)
   ...
```

- File headers show 📜 icon, path, entry count, and expand/collapse indicator
- Entries are indented under their parent file
- Non-existent files show dimmed with "(not found)" suffix
- Dirty files show a modification indicator (e.g., `*` or `●`)

### 5.3 Key Bindings

#### Normal Mode

| Key | Action |
|-----|--------|
| `↑`/`k`, `↓`/`j` | Navigate (files and entries) |
| `Home`/`End` | Jump to start/end |
| `Enter`/`Space` | Toggle file expand/collapse (on file header) |
| `0` | Collapse all files |
| `9` | Expand all files |
| `e` | Edit: file header → open file in $EDITOR; entry → open entry in $EDITOR |
| `a` | Add new entry at cursor position (opens $EDITOR with empty/template temp file) |
| `d` | Delete selected entry(ies) (with confirmation) |
| `s` | Toggle multi-select mode / toggle current item |
| `Shift+↑/↓` | Range selection |
| `x` | Cut selected entries |
| `p` | Paste cut entries after cursor position |
| `m` | Enter drag-style move mode |
| `/` | Enter fuzzy filter search |
| `u` | Single-step undo |
| `?` | Show help |
| `w`/`Ctrl+s` | Save all dirty files |
| `q` | Quit (confirm if dirty) |

#### Moving Mode (after pressing `m`)

- Selected entries highlighted with a distinct style
- `↑`/`↓` moves an insertion-point indicator (across files)
- `Enter` confirms the move to the target position
- `Esc` cancels, entries return to original position

#### Searching Mode (after pressing `/`)

- Fuzzy filter: entries are filtered in real-time as the user types
- Only matching entries are shown; file headers remain visible if they contain matches
- Matched characters are highlighted in a distinct color
- `Enter` exits search and focuses on the selected match
- `Esc` clears the filter, restores full list
- Search scope: entry `name` + `value` fields

### 5.4 $EDITOR Integration

#### Editing an Entry

1. Write `entry.value` to a temp file (e.g., `/tmp/.wenv_edit_XXXX`)
2. Suspend TUI (restore terminal to normal mode)
3. Execute `$EDITOR <temp_file>` and wait for exit
4. Read back modified content
5. Parse the modified content to update entry fields (name, entry_type extracted from value)
6. Resume TUI, trigger full redraw
7. Delete temp file
8. Mark file as dirty

#### Editing a File

1. Suspend TUI
2. Execute `$EDITOR <file_path>` and wait for exit
3. Re-read and re-parse the entire file
4. Resume TUI with updated entries
5. Mark file as dirty = false (user saved directly)

#### Adding an Entry

1. Determine the target file from cursor position
2. Create temp file (empty, or with a basic template comment)
3. Follow the same $EDITOR flow as editing an entry
4. Parse the result and insert after cursor position
5. Mark file as dirty

### 5.5 Cross-File Operations

#### Cut/Paste

1. Select one or more entries (single or multi-select)
2. Press `x` to cut — entries are removed from their source file(s) and stored in clipboard
3. Navigate to target position (any file)
4. Press `p` to paste — entries are inserted after cursor, assigned to the target file
5. Source and target files are both marked dirty

#### Drag-Style Move

1. Select entries, press `m`
2. A visual insertion indicator appears
3. Navigate across files with `↑`/`↓`
4. Press `Enter` to drop entries at the indicated position
5. Affected files are marked dirty

#### Single-Step Undo

One undo level: stores the previous state of all modified files. Pressing `u` restores the pre-operation state. A new operation overwrites the undo buffer.

### 5.6 Fuzzy Filter Search

Uses a fuzzy matching library (e.g., `nucleo` or `fuzzy-matcher`) for fzf-style filtering:

- Input appears in a search bar at the top or bottom of the screen
- Entries are scored and filtered in real-time
- File headers are preserved if any child entry matches
- Matching characters within entry names/values are highlighted
- Results are sorted by match score

---

## 6. Module Changes

### 6.1 Preserved (minimal changes)

| Module | LOC | Notes |
|--------|-----|-------|
| `parser/bash/` | ~2,180 | Core parsing logic untouched |
| `parser/pwsh/` | ~1,000 | Core parsing logic untouched |
| `parser/pending.rs` | ~413 | PendingBlock state machine untouched |
| `parser/builders/` | ~200 | QuotedValue/CommentBlock builders |
| `model/entry.rs` | ~350 | Add `file_index` field |
| `model/types.rs` | ~200 | EntryType, ShellType unchanged |
| `formatter/` | ~1,280 | Used for add-entry template syntax |
| `i18n/` | ~780 | Update message keys, structure unchanged |
| `utils/strings.rs` | ~100 | String utilities |
| `utils/path.rs` | ~50 | Path utilities |

### 6.2 Rewritten

| Module | Current LOC | Notes |
|--------|-------------|-------|
| `tui/app.rs` | 2,950 | New multi-file architecture |
| `tui/ui.rs` | 1,318 | New multi-file rendering with fuzzy filter UI |
| `tui/mod.rs` | 7 | Updated module organization |

### 6.3 Refactored

| Module | Notes |
|--------|-------|
| `cli/args.rs` | Remove flags, simplify to shell/source/config only |
| `cli/context.rs` | Build multi-file ShellProfile from config file list |
| `config/mod.rs` | Add `[files.*]` section support, templates, first-run flow |
| `model/config.rs` | Add FileListConfig struct |
| `utils/shell_detect.rs` | Clean up, ensure extensibility |

### 6.4 Removed

| Module | LOC | Reason |
|--------|-----|--------|
| `checker/` | 193 | Buggy, platform-dependent |
| `backup/` | 205 | Replaced by external tools (git) |
| `cli/actions/import.rs` | 188 | Feature removed |
| `cli/actions/export.rs` | 51 | Feature removed |
| `utils/http.rs` | ~80 | Only used by import |
| `utils/path_merge.rs` | 401 | Only used by format |
| `utils/dependency.rs` | ~100 | Only used by format |
| Cache system | ~70 | PowerShell path cache removed |

### 6.5 Dependency Changes

**Remove:**
- `ureq` — HTTP requests (import URL fetch)
- `url` — URL parsing (import)

**Add:**
- `nucleo` or `fuzzy-matcher` — Fuzzy filter search
- `glob` (crate) — File list `*.sh` pattern matching

**Keep:**
- `ratatui`, `crossterm` — TUI framework
- `clap` — CLI argument parsing
- `serde`, `toml` — Config serialization
- `regex`, `lazy_static` — Parser patterns
- `dirs` — Platform directory detection
- `dialoguer` — Interactive prompts (first-run, `wenv .`)
- `colored` — Terminal color output
- `anyhow`, `thiserror` — Error handling
- `time` — Timestamp utilities
- `terminal_size` — Terminal dimension detection
- `tempfile` (dev) — Test utilities

---

## 7. File Save Strategy

When the user presses `w`/`Ctrl+s`:

1. Iterate all `ProfileFile` entries where `dirty == true`
2. For each dirty file:
   a. Reconstruct file content from entries (using `entry.value` which contains complete syntax)
   b. Write to the file path
   c. Set `dirty = false`
3. Display a status message: "Saved N file(s)"

When quitting with dirty files, a confirmation prompt lists the unsaved files.

---

## 8. Scope Boundaries

### In Scope (This Refactoring)

- Multi-file TUI with all described operations
- Config file list system with templates for bash/zsh/powershell
- $EDITOR integration for entry and file editing
- Fuzzy filter search
- Cross-file cut/paste and drag-style move
- CLI simplification
- macOS config path unification
- Single-step undo

### Out of Scope (Future Work)

- Fish shell parser
- sh/ksh shell parser (can share bash parser in future)
- Multi-step undo/redo
- Format/sort functionality
- Import/export
- Syntax checking/validation
- Backup system
- URL-based operations
