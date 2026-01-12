# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**wenv** is a cross-platform CLI tool for managing shell configuration files (`.bashrc`, PowerShell profiles). It parses, organizes, edits, and maintains aliases, functions, environment variables, and source statements.

## Build and Test Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Fast syntax check
cargo test               # Run all tests
cargo test --lib         # Library tests only
cargo test bash_tests    # Run specific test module
cargo clippy             # Linting
cargo fmt                # Format code
cargo run -- list        # Run with arguments
```

## Architecture

### Trait-Based Design

The codebase uses traits for extensibility across shell types:

- **`Parser` trait** (`src/parser/mod.rs`) - Implemented by `BashParser` and `PwshParser` for shell-specific parsing
- **`Formatter` trait** (`src/formatter/mod.rs`) - Shell-specific formatting of configuration files
- **`Checker` trait** (`src/checker/mod.rs`) - Validation rules (duplicate detection, syntax checking)

### Command Pattern

Each CLI command has its own module in `src/cli/commands/` with an `execute()` function. Commands share context via `CommandContext`.

### Core Data Models (`src/model/`)

```rust
EntryType { Alias, Function, EnvVar, Source }
ShellType { Bash, PowerShell }
Entry { entry_type, name, value, line_number, comment, raw_line }
```

### Key Modules

- `src/parser/bash.rs` - Bash parser with control structure awareness (skips definitions inside if/while/for/case blocks)
- `src/parser/pwsh.rs` - PowerShell parser (in progress)
- `src/backup/mod.rs` - Automatic backup system before write operations
- `src/utils/shell_detect.rs` - Shell type detection from env, extension, filename patterns

## Important Implementation Details

### Lenient Parsing

The parser operates in "lenient mode" - it skips unparseable lines with warnings and continues processing. This is intentional to handle real-world config files with varied syntax.

### Control Structure Awareness

The Bash parser tracks control structure depth (`if`/`while`/`for`/`case`) to only extract top-level definitions, avoiding aliases and functions defined inside conditional blocks.

### Backup System

Backups are automatically created before any write operation in `~/.config/wenv/backups/<shell>/` with timestamp naming.

### Regex Patterns

Due to Rust regex limitations (no backreferences), the Bash parser uses separate patterns for different quote styles:
- Single-quoted aliases: `alias name='value'`
- Double-quoted aliases: `alias name="value"`
- Unquoted aliases: `alias name=value`
