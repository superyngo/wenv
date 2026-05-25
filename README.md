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
| `Space` | Toggle selection |
| `Shift+↑`/`↓` | Extend selection range |
| `Enter` | Edit entry with $EDITOR |
| `a` | Add new entry with $EDITOR |
| `d` | Delete selected entries |
| `x` | Cut selected entries |
| `p` | Paste clipboard entries |
| `m` | Enter move mode |
| `Tab` | Toggle file expanded/collapsed |
| `Shift+Tab` | Toggle all files |
| `u` | Undo last operation |
| `/` | Search/filter entries |
| `Esc` | Clear selection/exit modes |
| `r` | Refresh from disk |
| `s` | Save all changes |
| `?` | Show help |
| `q` | Quit (confirms if unsaved) |

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

### Cross-File Cut/Paste

1. Select entries with `Space` 
2. Cut with `x`
3. Navigate to target file/position
4. Paste with `p`

Entries are automatically updated with the correct file_index.

### Undo System

- Press `u` to undo any operation
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
