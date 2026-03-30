# Snippet Template Menu Design

**Date:** 2026-03-30
**Status:** Draft

## Overview

When pressing 'n' (new entry) in the TUI, show a scrollable popup menu listing snippet templates for the active shell type. User selects a template, the temp file is pre-filled with the template content, then `$EDITOR` opens as before.

## Config Structure

### Inline Snippets

Added to `config.toml` as `[[snippets.<shell>]]` arrays:

```toml
[[snippets.zsh]]
name = "Empty"
description = "Blank entry"
# No template field = empty file

[[snippets.zsh]]
name = "source"
description = "Source a file"
template = "# Source a shell file\nsource PATH"

[[snippets.zsh]]
name = "alias"
description = "Define an alias"
template = "# Set alias name and value\nalias NAME='VALUE'"
```

### External Template Files

Top-level `[template_paths]` section in config.toml points to external TOML files:

```toml
[template_paths]
paths = [
    "~/.config/wenv/snippets/extra.toml",
    "~/dotfiles/wenv-snippets.toml",
]
```

External TOML files contain `[[snippets.<shell>]]` arrays for multiple shells:

```toml
[[snippets.zsh]]
name = "bindkey"
description = "Bind a key"
template = "# Bind key to widget\nbindkey KEY WIDGET"

[[snippets.pwsh]]
name = "enum"
description = "Define an enum"
template = "# Define enum type\nenum NAME {\n    VALUE1\n    VALUE2\n}"
```

### Loading Order

1. Load inline `[[snippets.<shell>]]` from config.toml
2. Load all files from `[template_paths]` paths
3. Merge: external snippets append after inline snippets
4. Deduplicate by `name` — first occurrence wins (inline priority)

### Initialization

- On first run or when `[snippets.<active_shell>]` is missing, populate defaults for the active shell type only (consistent with file path initialization)
- Once present, config is the single source of truth — user can add/remove/reorder freely

## Default Snippets

### Bash/Zsh

| Name | Description | Template |
|------|-------------|----------|
| Empty | Blank entry | *(none)* |
| source | Source a file | `# Source a shell file\nsource PATH` |
| export | Set environment variable | `# Set variable name and value\nexport NAME='VALUE'` |
| alias | Define an alias | `# Set alias name and value\nalias NAME='VALUE'` |
| function | Define a function | `# Define function name and body\nNAME() {\n    # body\n}` |

### Zsh-only additional

| Name | Description | Template |
|------|-------------|----------|
| bindkey | Bind a key | `# Bind key to widget\nbindkey KEY WIDGET` |

### PowerShell

| Name | Description | Template |
|------|-------------|----------|
| Empty | Blank entry | *(none)* |
| source | Source a file | `# Source a PowerShell file\n. PATH` |
| env | Set environment variable | `# Set environment variable\n$env:NAME = "VALUE"` |
| alias | Define an alias | `# Set alias name and command\nSet-Alias -Name NAME -Value COMMAND` |
| function | Define a function | `# Define function name and body\nfunction NAME {\n    # body\n}` |
| enum | Define an enum | `# Define enum type\nenum NAME {\n    VALUE1\n    VALUE2\n}` |
| class | Define a class | `# Define class\nclass NAME {\n    # properties and methods\n}` |
| scriptblock | Script block | `# Script block\n{\n    # code\n}` |

## TUI Popup

### New Mode

`AppMode::SelectingSnippet` added to the `AppMode` enum.

### Flow

1. User presses 'n'
2. Check file is writable (same as current)
3. Load snippets for active shell (if not already loaded)
4. If no snippets exist → skip menu, fall back to empty flow
5. Enter `AppMode::SelectingSnippet`, show popup
6. User navigates with j/k or arrow keys, presses Enter to select
7. "Empty" selected → `run_add_entry` with blank temp file (current behavior)
8. Template selected → `run_add_entry` with pre-filled template content
9. Esc cancels, returns to Normal mode

### Popup Rendering

```
┌─ New Entry ─────────────────────────┐
│  Empty                              │
│  source  — Source a file            │
│  alias   — Define an alias          │
│  export  — Set environment var      │
│  function — Define a function       │
│  bindkey — Bind a key               │
│                                     │
│  ↑↓ navigate  Enter select  Esc     │
└─────────────────────────────────────┘
```

- First line ("Empty") default selected, highlighted
- Width: content-based, max 80% terminal width
- Height: `min(snippet_count + 2, 80% height)` — scrollable if overflow
- Border title: "New Entry"
- Bottom line: key hints
- Only shows snippets for active shell type

### Key Bindings in SelectingSnippet Mode

| Key | Action |
|-----|--------|
| `j`/`k`, Up/Down | Navigate |
| `Enter` | Select, proceed to editor |
| `Esc` | Cancel, return to Normal |

## Data Model Changes

### New Structs

```rust
pub struct Snippet {
    pub name: String,
    pub description: String,
    pub template: Option<String>,  // None = Empty entry
}

pub struct TemplatePathsConfig {
    pub paths: Vec<String>,
}
```

### Config Changes

```rust
pub struct Config {
    pub ui: UiConfig,
    pub files: HashMap<String, FilesConfig>,
    pub snippets: HashMap<String, Vec<Snippet>>,  // key = "bash"/"zsh"/"powershell"
    pub template_paths: TemplatePathsConfig,
}
```

### App State Additions

```rust
// In App struct
snippet_cursor: usize,
snippet_scroll_offset: usize,
snippets: Vec<Snippet>,  // loaded for active shell
```

### run_add_entry Change

Current: `run_add_entry(&mut self, terminal, file_index)`
New: `run_add_entry(&mut self, terminal, file_index, template: Option<&str>)`

If `template` is `Some(content)`, write it to temp file before opening editor. Otherwise, current behavior (empty temp file).

## Edge Cases & Error Handling

- External TOML file not found → skip with warning log, continue
- External TOML parse error → skip with warning, continue with inline snippets
- `[template_paths]` missing → fine, inline-only
- `[snippets.<shell>]` missing for active shell → populate defaults
- No snippets at all → skip menu, fall back to empty flow
- Template is empty string → treat same as "Empty"
- Duplicate `name` inline + external → inline wins
- Duplicate `name` within same source → last occurrence wins
- User saves nothing in editor → discard, no entry created (same as current)
