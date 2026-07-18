# wenv

A cross-platform Terminal User Interface (TUI) for managing multiple shell configuration files.

[![Crate](https://img.shields.io/crates/v/wenv.svg)](https://crates.io/crates/wenv)
[![API](https://docs.rs/wenv/badge.svg)](https://docs.rs/wenv)
![License](https://img.shields.io/crates/l/wenv.svg)

## Features

- 🌳 **Multi-file tree view** - Manage multiple .bashrc, .zshrc, PowerShell profiles in one interface
- ✏️ **External editor integration** - Edit entries with your `$EDITOR` (vim, nano, VS Code, etc.)
- 🔍 **Fuzzy search** - Filter entries across all files with real-time search
- ✂️ **Cross-file operations** - Cut, copy, paste entries between different configuration files  
- ↩️ **Undo support** - Undo any operation to restore previous state
- 📁 **Config-based file lists** - Define which files to manage in `~/.config/wenv/config.toml`
- 🎯 **Smart parsing** - Recognizes aliases, functions, environment variables, source statements
- 💾 **Safe editing** - Only saves changes when you confirm

## Installation

### With Wenget

```bash
wenget install wenv
```

### From Cargo

```bash
cargo install wenv
```

### From Precompiled Binaries

Download the latest release for your platform from the [Releases](https://github.com/superyngo/wenv/releases) page.

### From Source

```bash
git clone https://github.com/superyngo/wenv.git
cd wenv
cargo build --release
```

## Usage

### Basic Commands

```bash
# Launch TUI (default mode)
wenv

# Launch TUI with file selection prompt
wenv .
wenv --source

# Force specific shell type
wenv --shell bash
wenv --shell zsh  
wenv --shell powershell

# Open configuration file in editor
wenv config
```

### TUI Key Bindings

| Key | Action |
|-----|--------|
| `j`/`k`, `↑`/`↓` | Navigate entries |
| `Enter`/`Space` | Toggle expand/collapse file |
| `s` | Toggle selection |
| `Shift+↑`/`↓` | Extend selection range |
| `e` | Edit entry — single-line entries edit inline; multi-line entries open $EDITOR |
| `E` | Edit entry in $EDITOR (force external) |
| `a` | Insert entry — snippet template menu, then $EDITOR. On a **directory-group header**: prompt for a new file name → create the file in the group's directory → open it in $EDITOR |
| `n` | New file path — add to config (plain file, `~`, `$VAR`/`%VAR%`, glob (`*`/`?`), or directory) |
| `d` / `Delete` | Delete entries / Remove file from config / Remove group. On a file **inside a directory group**: move the real file to the system trash (confirmed) |
| `c` | Copy — place sources (blue), navigate green target, `v`/`Enter` drops a clone |
| `x` | Cut — place sources (blue), `v`/`Enter` drops and removes them (move) |
| `m` | Move file (reorder) — on a file header; entries move via `x` |
| `r` | Toggle remark (comment/uncomment) |
| `0` | Collapse all files |
| `9` | Expand all files |
| `z` | Undo last operation |
| `y` | Redo last operation |
| `/` | Open filter input (fuzzy match; caret editing with `←/→/Home/End/Backspace/Del`) |
| `Esc` | Clear selection/exit modes |
| `w` | Save all changes |
| `?` | Show help / About (scrollable: `↑↓`/`PgUp`/`PgDn`/`Home`/`End`) |
| `q` | Quit (confirms if unsaved) |

Set the `NO_COLOR` environment variable to run the TUI in monochrome (focus and selection degrade to reverse-video).

## Shell Support

### Bash/Zsh

Supports:
- Aliases: `alias ll='ls -la'`
- Functions: `greet() { echo hello $1; }`
- Environment variables: `export PATH="/usr/bin:$PATH"`
- Source statements: `source ~/.profile`

### PowerShell

Supports:
- Aliases: `Set-Alias ll Get-ChildItem`
- Functions: `function greet { Write-Host "Hello $args" }`
- Environment variables: `$env:PATH = "/usr/bin;$env:PATH"`
- Source statements: `. ~/.profile.ps1`

## Multi-File Operations

### Cross-File Copy / Cut (placement)

1. Select entries with `s` (or just point the cursor at one)
2. Press `c` (copy) or `x` (cut) — the sources are marked **blue** and you enter placement mode
3. Navigate to the target file/position — the drop point shows as a **green** box
4. While placing, press `c`/`x` to switch between copy and move (the status bar shows the current mode)
5. Press `v` or `Enter` to drop (`Esc` to cancel)
   - **copy** leaves the sources in place
   - **cut** removes the sources (net move)

Entries are automatically updated with the correct file_index.

### Undo System

- Press `z` to undo any operation (`y` to redo)
- Restores all files to their previous state
- Handles multi-file operations atomically

### Search & Filter

- Press `/` to enter search mode
- Type to filter entries across all expanded files
- Press `Enter` to go to first match
- Press `Esc` to clear filter

## Development

### Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Fast syntax check
cargo test               # Run all tests
cargo test tui_logic_tests # Run TUI logic tests
cargo clippy             # Linting
cargo fmt                # Format code
```

### Testing

```bash
cargo test               # All tests
cargo test --lib         # Library tests only
cargo test --test integration # Integration tests
cargo test tui_logic_tests # TUI operations tests
```

## Configuration

wenv keeps two distinct pieces of configuration:

### User config — `config.toml`

UI settings and the per-shell file lists live in a single fixed location:

```
~/.config/wenv/config.toml
```

A default is created from a template the first time it's missing. The path is the same whether you run via `cargo run` or the release binary. Use `-c, --config <PATH>` to point at an alternate file (read or created there). Run `wenv config` to open the resolved file in `$EDITOR`.

### Snippets — `Resources/snippets.toml` (mandatory bundled resource)

Snippet templates for the TUI "new entry" (`n`) picker ship **with the binary** and are required at runtime — wenv refuses to start if they cannot be found, and never auto-generates them. Search order:

- **Development (`cargo run`)**: the in-repo `Resources/snippets.toml`.
- **Release binary**, in order:
  1. `<binary_dir>/Resources/snippets.toml` (primary — matches the release archive layout)
  2. `$HOME/.wenget/apps/wenv/Resources/snippets.toml`
  3. `$HOME/.local/bin/Resources/snippets.toml`
  4. `/opt/wenget/apps/wenv/Resources/snippets.toml`
  5. `/usr/local/bin/Resources/snippets.toml`

  (Windows uses the equivalent `%USERPROFILE%`, `%LOCALAPPDATA%`, `%ProgramW6432%`, and `%ProgramFiles%` locations.)

## Release tarball layout

Each release tarball unpacks to:

```
wenv-vX.Y.Z-<target>/
├── wenv (or wenv.exe)
└── Resources/
    └── snippets.toml
```

`config.toml` is **not** shipped — it is created in `~/.config/wenv/` on first run.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
