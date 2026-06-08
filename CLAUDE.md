# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**wenv** is a cross-platform CLI tool for managing multiple shell configuration files (.bashrc, .zshrc, PowerShell profiles) with an interactive Terminal User Interface (TUI). It provides a tree view for multi-file management, entry editing with $EDITOR, search capabilities, and cross-file operations like cut/paste/move.

## Build and Test Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Fast syntax check
cargo test               # Run all tests
cargo test --lib         # Library tests only
cargo test tui_logic_tests # Run TUI logic tests
cargo clippy             # Linting
cargo fmt                # Format code
cargo run               # Run TUI
cargo run -- --shell bash # Force shell type
```

## Architecture

### Multi-File Data Model (`src/model/profile.rs`)

The core data structure supports multiple configuration files per shell:

```rust
// Shell session with multiple files
ShellProfile {
    shell_type: ShellType,
    files: Vec<ProfileFile>,
}

// Individual configuration file
ProfileFile {
    path: PathBuf,
    entries: Vec<Entry>,
    content: String,
    expanded: bool,   // UI tree state
    dirty: bool,      // Has unsaved changes
    exists: bool,     // File exists on disk
}

// TUI navigation items
ListItem::FileHeader(file_index)     // Tree headers
ListItem::Entry(file_index, entry_index)  // Individual entries
```

### TUI Architecture (`src/tui/`)

Interactive terminal interface with these modules:

- **`app.rs`** - Main TUI application state and event handling
- **`ui.rs`** - Terminal rendering with ratatui
- **`keys.rs`** - Key binding definitions and help text
- **`list.rs`** - Entry list rendering with tree view
- **`operations.rs`** - Entry manipulation (delete, cut, paste, undo)
- **`selection.rs`** - Multi-selection with visual indicators
- **`state.rs`** - Application modes and clipboard/undo state
- **`editor.rs`** - External $EDITOR integration
- **Inline editor** (`AppMode::InlineEdit` + `InlineEditState` in `state.rs`) - In-place single-field editor for single-line entries. Edits the entry's whole raw `value` line; commit re-parses through the same path as the external editor (`apply_edited_value` → `replace_entry_with_parsed`), so name/type are re-derived. Block cursor, `←/→/Home/End` + `Backspace/Del`, horizontal overflow scrolling (`clamp_inline_scroll`) with a status-bar position hint `col {caret}/{len}  ⟨start–end⟩` (caret column always; visible window only on overflow). Multi-line (`value.contains('\n')`) entries fall back to `$EDITOR`
- **`search.rs`** - Fuzzy filter state and matching

### Trait-Based Parsing/Formatting

Shell-specific logic implemented via traits:

- **`Parser` trait** (`src/parser/mod.rs`) - Implemented by `BashParser` and `PwshParser`
- **`Formatter` trait** (`src/formatter/mod.rs`) - Shell-specific file reconstruction

### Core Data Models (`src/model/`)

```rust
EntryType { Alias, Function, EnvVar, Source, Code, Comment, ScriptBlock }
ShellType { Bash, Zsh, PowerShell }
Entry { entry_type, name, value, line_number, end_line, file_index }
```

**Entry Field Semantics:**
- `entry_type`: Classification for UI filtering/grouping only
- `name`: Extracted identifier for UI display/search only (e.g., alias name, function name, "L10-L12" for Code)
- `value`: **Complete raw syntax** - stores the full original line(s) including keywords, options, quotes, and any merged comments/blank lines
- `line_number`: Starting line number in source file
- `end_line`: Ending line number for multi-line entries

**Value Field Architecture:**
- Alias: `value = "alias -g ll='ls -la'"` (complete syntax, not just `'ls -la'`)
- EnvVar: `value = "export PATH=\"/usr/bin\""` (complete syntax, not just `"/usr/bin"`)
- Function: `value = "foo() { echo hi; }"` (complete definition)
- Source: `value = "source ~/.profile"` (complete syntax, not just `~/.profile`)
- Comment: `value = "# This is a comment"` (including `#` prefix)
- Code: `value = "echo hello"` (raw shell code)

When Comment/blank lines precede structured entries, they merge:
- `value = "# comment\n\nalias foo='bar'"` (complete content including leading comments)

### Key Modules

- `src/parser/bash.rs` - Bash parser with control structure awareness (skips definitions inside if/while/for/case blocks)
- `src/parser/pwsh.rs` - PowerShell parser (in progress)
- `src/utils/shell_detect.rs` - Shell type detection from env, extension, filename patterns (runtime only)
- `src/config/mod.rs` - Configuration file management with `[files.*]` sections

## Important Implementation Details

### Lenient Parsing

The parser operates in "lenient mode" - it skips unparseable lines with warnings and continues processing. This is intentional to handle real-world config files with varied syntax.

### Control Structure Awareness

The Bash parser tracks control structure depth (`if`/`while`/`for`/`case`) to only extract top-level definitions, avoiding aliases and functions defined inside conditional blocks.

### Configuration System

Configuration is split across two files with different lifecycles:

**1. User config — `config.toml` (UI + file lists)**
- Single fixed location, all platforms: `~/.config/wenv/config.toml`
- Same path for `cargo run` and the release binary (no multi-location search)
- Auto-created from a template when missing
- Override location with the `-c, --config <PATH>` global flag
- Loaded by `Config::resolve_or_create(shell_key, config_override)` (`src/model/config.rs`)

Structure:
- `[ui]` - UI settings (language selection)
- `[files.<shell>]` - per-shell file path lists (`bash` / `zsh` / `powershell`)

```toml
[ui]
language = "en"

[files.bash]
paths = ["~/.bashrc", "~/.bash_aliases", "~/.profile"]

[files.zsh]
paths = ["~/.zshrc", "~/.zsh_aliases"]
```

**2. Snippets — `Resources/snippets.toml` (mandatory bundled resource)**
- Snippet templates for the TUI `n` key, shipped alongside the binary
- **Required at runtime**: if not found anywhere in the search chain, the app prints the searched paths and exits non-zero. Never auto-generated; no embedded defaults.
- Search chain (`Snippets::resolve()` in `src/model/config.rs`):
  - Debug builds: in-repo `Resources/snippets.toml` (via `CARGO_MANIFEST_DIR`)
  - `<exe_dir>/Resources/snippets.toml` (primary — matches release archive)
  - platform install fallbacks (`~/.wenget/...`, `~/.local/bin/Resources`, `/opt/...`, `/usr/local/bin/Resources`; Windows `%USERPROFILE%`/`%LOCALAPPDATA%`/`%ProgramW6432%`/`%ProgramFiles%` equivalents)
- Structure: `[[snippets.<shell>]]` arrays of `{ name, description, template? }`

```toml
[[snippets.zsh]]
name = "alias"
description = "alias NAME='VALUE'"
template = "alias NAME='VALUE'"
```

### TUI Key Bindings Reference

| Key | Action |
|-----|--------|
| `j`/`k`, `↑`/`↓` | Navigate entries |
| `Enter`/`Space` | Toggle expand/collapse file |
| `s` | Toggle selection |
| `Shift+↑`/`↓` | Extend selection range |
| `e` | Edit entry — single-line entries edit **inline** (in-place); multi-line (merged/combined) entries open `$EDITOR`. File headers open `$EDITOR` |
| `E` | Edit entry in `$EDITOR` (force external, any entry) |
| `n` | New entry — shows snippet template menu, then $EDITOR |
| `d` | Delete entries / Remove file from config |
| `x` | Cut selected entries |
| `c` | Copy selected entries |
| `v` | Paste clipboard entries |
| `m` | Enter move mode (entry or file) |
| `r` | Toggle remark (comment/uncomment) |
| `a` | Add path to config — accepts any config format: plain file, `~`, `$VAR`/`%VAR%`, glob (`*`/`?`), or a directory. Globs/dirs load as a group; a single missing file offers to create it |
| `0` | Collapse all files |
| `9` | Expand all files |
| `z` | Undo last operation |
| `/` | Open filter input (fuzzy-match; non-matching entries hidden) |
| `Enter` (in filter) | Commit filter — enter FilterActive mode for normal operations |
| `Esc` (in filter) | Clear filter and restore full list |
| `Esc` | Clear selection/exit modes |
| `w` | Save all changes |
| `?` | Show help |
| `q` | Quit (confirms if unsaved) |

### Shell Type Detection

Shell type is determined at runtime via:
1. `--shell` flag (bash/zsh/powershell)
2. Environment variable detection (`$SHELL`, `$0`)
3. No config-based shell preference (simplified)

### $EDITOR Integration

The TUI launches external editors for entry creation and editing:
- Detects `$EDITOR`, `$VISUAL`, or falls back to platform defaults
- Creates temporary files with shell syntax highlighting hints
- Parses editor output back into Entry format

### Multi-File Operations

Cross-file operations supported:
- **Cut/Paste**: Move entries between files
- **Undo**: Restores all files to previous state
- **Search**: Filters entries across all expanded files
- **Save**: Writes all dirty files atomically
A sibling `cache.toml` (written next to the resolved `config.toml`, i.e. `config.source_path.parent()/cache.toml`) stores auto-detected PowerShell profile paths:
```toml
pwsh_profile = "/path/to/pwsh/profile.ps1"
powershell_profile = "/path/to/powershell/profile.ps1"
```
- Managed by `src/config/cache.rs` (`Cache::load_or_default` / `Cache::save`)
- Auto-detected on first run when PowerShell shell type is used
- Lazy invalidation: a cached path that no longer exists on disk is dropped on load
- User-editable if manual override needed

**i18n Language Files:**
- External language files: `~/.config/wenv/i18n/{lang}.toml`
- Set language in config: `[ui] language = "zh-TW"`
- English embedded in binary as fallback

### Entry Value Semantics

All entry types store complete raw syntax in the `value` field:

**Structured Entries:**
- `Alias` - value contains full syntax: `"alias name='value'"` (not just the value part)
- `Function` - value contains complete definition: `"name() { body }"`
- `EnvVar` - value contains full syntax: `"export VAR='value'"` (not just the value part)
- `Source` - value contains full syntax: `"source path"` (not just the path)

**Raw Entries:**
- `Comment` - value contains full line: `"# comment text"`
- `Code` - value contains full line(s): `"if true; then\n  echo hi\nfi"`

**Merged Entries:**
When comments/blank lines precede structured entries, they merge into a single entry:
- `value = "# comment\n\nalias foo='bar'"` (complete content)
- `entry_type = Alias` (determined by the structured part)
- `name = "foo"` (extracted from structured part)

### 換行符格式規範（分隔符 vs 終止符）

**核心概念**：專案中存在兩種換行符語意，混淆會導致 off-by-one 錯誤。

| 格式 | 規則 | 3 行範例 | 使用場景 |
|------|------|----------|----------|
| **分隔符格式** | N 行 = N-1 個 `\n` | `"line1\n\n"` | `value`, `value_buffer` |
| **終止符格式** | N 行 = N 個 `\n` | `"line1\n\n\n"` | 文件內容 |

**關鍵規則**：
- 分割 `value` 必須用 `value.split('\n')`，不可用 `.lines()` 或 `split_lines_preserve_trailing()`
- 寫入文件時，分隔符格式內容需額外加終止符
- 判斷「是否以換行結尾」時，要分清是內容的一部分還是終止符

### TUI Comment/Code 編輯保存

使用 `replace_line_range()` 直接替換 entry 佔據的行範圍：
- `value_buffer` 是分隔符格式，寫入時無條件加 `\n` 終止符

### Regex Patterns

Due to Rust regex limitations (no backreferences), the Bash parser uses separate patterns for different quote styles:
- Single-quoted aliases: `alias name='value'`
- Double-quoted aliases: `alias name="value"`
- Unquoted aliases: `alias name=value`
