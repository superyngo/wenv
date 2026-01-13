# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project Overview

**wenv** is a cross-platform CLI tool for managing shell configuration files (`.bashrc`, PowerShell profiles). It parses, organizes, edits, and maintains aliases, functions, environment variables, and source statements.

## Build/Lint/Test Commands

```bash
# Build
cargo build              # Debug build
cargo build --release    # Release build
cargo check              # Fast syntax/type check (use frequently)

# Lint and Format
cargo fmt                # Format code (run before commits)
cargo clippy             # Lint with clippy (fix all warnings)
cargo fmt -- --check     # Check formatting without modifying

# Run Tests
cargo test               # Run all tests
cargo test --lib         # Library unit tests only
cargo test --test integration  # Integration tests only

# Run a Single Test
cargo test test_parse_alias_single_quote           # Run test by name
cargo test bash::tests::test_parse_alias           # Run by module path
cargo test bash_tests -- --nocapture               # Run with output

# Run Tests in Specific Module
cargo test parser::bash::tests                     # All tests in bash parser
cargo test checker::                               # All checker tests

# Run CLI
cargo run -- list                    # Run with subcommand
cargo run -- --file ~/.bashrc list   # Specify config file
cargo run -- --help                  # Show help
```

## Architecture

### Trait-Based Design

Core traits for shell extensibility:

- **`Parser`** (`src/parser/mod.rs`) - Implemented by `BashParser`, `PowerShellParser`
- **`Formatter`** (`src/formatter/mod.rs`) - Shell-specific output formatting
- **`Checker`** (`src/checker/mod.rs`) - Validation rules (duplicates, syntax)

### Command Pattern

Each CLI command lives in `src/cli/commands/<name>.rs` with an `execute()` function.
Commands share state via `CommandContext` from `src/cli/commands/mod.rs`.

### Module Structure

```
src/
├── cli/commands/     # CLI subcommands (list, add, check, etc.)
├── parser/           # Shell parsers
│   ├── bash/         # Bash parser (patterns, control, parsers modules)
│   ├── pwsh/         # PowerShell parser
│   └── builders/     # Multi-line entry builders
├── model/            # Data structures (Entry, EntryType, ShellType)
├── formatter/        # Shell-specific formatters
├── checker/          # Validation checkers
├── backup/           # Backup system
└── utils/            # Shell detection, path utilities
```

## Code Style Guidelines

### Imports

- Group imports: `std` first, then external crates, then `crate::` imports
- Use `use crate::module::Item` for internal imports
- Prefer explicit imports over glob imports (`*`)

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Types/Traits | PascalCase | `EntryType`, `BashParser` |
| Functions/Methods | snake_case | `parse_alias`, `get_shell_type` |
| Constants/Statics | SCREAMING_SNAKE | `ALIAS_SINGLE_RE` |
| Modules | snake_case | `shell_detect`, `code_block` |
| Test functions | `test_<what_it_tests>` | `test_parse_alias_single_quote` |

### Error Handling

- Use `anyhow::Result` for functions that can fail in CLI commands
- Use `thiserror` for library error types requiring pattern matching
- Prefer `?` operator over explicit `match` for error propagation
- Add context with `.context()` or `.with_context()`

### Types and Patterns

- Use builder pattern with `with_*` methods for optional fields
- Implement `Default` for types with sensible defaults
- Derive `Debug, Clone` on data types; add `Serialize, Deserialize` if needed
- Use `Option<T>` for optional fields, not sentinel values

### Documentation

- Add `//!` module docs at top of each file explaining purpose
- Document public traits and their required methods
- Use `///` doc comments for public functions

### Tests

- Place unit tests in `#[cfg(test)] mod tests` at bottom of each file
- Use descriptive test names: `test_<function>_<scenario>`
- Integration tests go in `tests/integration/`
- Use `assert_eq!` with meaningful values, `assert!` for booleans

## Key Implementation Details

### Lenient Parsing

The parser skips unparseable lines with warnings rather than failing. This handles real-world config files with varied syntax.

### Control Structure Awareness

Bash parser tracks `if`/`while`/`for`/`case` depth to only extract top-level definitions, avoiding aliases inside conditional blocks.

### Multi-line Detection

| Entry Type | Start Detection | End Detection |
|------------|-----------------|---------------|
| Function | `func() {` | brace_count = 0 |
| Code Block | `if`/`while`/`for`/`case` | `fi`/`done`/`esac` |
| Alias/Env | Odd single quotes | Even single quotes |

### Regex Patterns

Due to Rust regex limitations (no backreferences), use separate patterns for quote styles:
- `ALIAS_SINGLE_RE` for single quotes
- `ALIAS_DOUBLE_RE` for double quotes

Add new patterns in `src/parser/bash/patterns.rs` using `lazy_static!`.

### Backup System

Backups are created automatically before writes in `~/.config/wenv/backups/<shell>/`.

## Adding New Features

### New Entry Type

1. Add variant to `EntryType` enum in `src/model/entry.rs`
2. Add regex pattern in `src/parser/bash/patterns.rs`
3. Add parse method in `src/parser/bash/parsers.rs`
4. Integrate into main loop in `src/parser/bash/mod.rs`

### New CLI Command

1. Create `src/cli/commands/<name>.rs` with `execute()` function
2. Add command variant to `Commands` enum in `src/cli/args.rs`
3. Add match arm in `src/main.rs`

### New Checker

1. Create module in `src/checker/`
2. Implement `Checker` trait
3. Add to `check_all()` function in `src/checker/mod.rs`
