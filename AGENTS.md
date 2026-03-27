# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

**wenv** — A cross-platform Rust CLI/TUI for managing shell config files (.bashrc, .zshrc, PowerShell profiles).
Rust 2021 edition. Clap for CLI, ratatui/crossterm for TUI, anyhow for errors.

## Build / Lint / Test Commands

```bash
cargo check                          # Fast syntax/type check
cargo clippy -- -D warnings          # Lint (warnings as errors)
cargo fmt -- --check                 # Check formatting (do not auto-format)
cargo fmt                            # Auto-format
cargo test                           # All tests
cargo test --lib                     # Library unit tests only
cargo test <test_name>               # Single test by name substring
cargo test --lib <test_name>         # Single lib test
cargo test --test tui_logic_tests    # Single integration test file
cargo test -- --nocapture            # Tests with stdout visible
cargo build --release                # Optimized release binary
cargo run -- --shell bash            # Run TUI with specific shell
```

**Always run `cargo clippy -- -D warnings` and `cargo fmt -- --check` after making changes.**

## Code Style

### Formatting
- No `rustfmt.toml` or `rust-toolchain.toml` — use Rust defaults (`rustfmt`).
- Soft line length ~100 chars. Break long lines at natural boundaries.
- Trailing commas in struct/vec literals; not required in match arms.
- Early returns preferred over deep nesting.

### Imports
Three groups separated by blank lines:
1. External crates (`anyhow`, `clap`, `crossterm`, `ratatui`, `regex`, ...)
2. `std` library (`std::path::PathBuf`, `std::collections::HashMap`, ...)
3. `crate::` internal imports

```rust
use anyhow::Result;
use crossterm::event::{self, Event};

use std::collections::VecDeque;
use std::io;

use crate::model::profile::{ListItem, ShellProfile};
use crate::tui::keys::Action;
```

Use multi-path imports for the same module (`use a::{X, Y, Z}`). Avoid wildcard imports.

### Naming
- **Structs/Enums/Traits**: `PascalCase` (`ShellProfile`, `EntryType`, `Parser`)
- **Functions/Methods/Variables/Fields**: `snake_case` (`load_config`, `scroll_offset`)
- **Constants/Statics**: `SCREAMING_SNAKE_CASE` (`MAX_UNDO_HISTORY`, `ALIAS_SINGLE_RE`)
- **Modules**: `snake_case` directories with `mod.rs`

### Types
- `String` for struct fields, `&str` for function parameters.
- `&'static str` for compile-time fixed strings (i18n keys, config keys).
- `PathBuf` for owned paths, `&Path` for borrowed.
- `Option<usize>` for nullable line numbers.
- Explicit annotations on signatures; inference for locals.

### Visibility
- Private submodules (`mod foo;`) with selective re-exports (`pub use foo::Bar;`).
- Public fields directly on structs — no getter/setter boilerplate.
- Module directories use `mod.rs` exclusively (no `foo.rs` + `foo/` pattern).

### Error Handling
- `anyhow::Result<T>` + `?` operator everywhere. `thiserror` is declared but unused.
- `anyhow::anyhow!()` for error messages, `anyhow::bail!()` for early returns with errors.
- `eprintln!()` for non-fatal user-facing warnings.

### Comments
- `//!` module doc on every file (brief one-liner).
- `///` doc comments on all public items (structs, enums, traits, functions, constants).
- `//` inline comments sparingly, only for non-obvious logic.
- Enum/struct field comments on the same line: `Code,    // Raw code lines`.
- Language: English.

### Patterns
- **Clap**: derive API with `#[command]` and `#[arg]` attributes.
- **Trait objects**: factory functions returning `Box<dyn Trait>` (Parser, Formatter).
- **Builder on Entry**: `Entry::new(...).with_line_number(n).with_end_line(n)`.
- **Action enum + match**: TUI key events mapped to `Action` enum, dispatched via `match`.
- **lazy_static!**: for compile-once regex patterns.
- **OnceLock**: for global single-init state (i18n messages).
- **cfg!(windows) / cfg!(unix)**: for platform-specific code.

## Testing

### Unit Tests
Inline `#[cfg(test)] mod tests` at the bottom of source files. Use `use super::*;`.

### Integration Tests
In `tests/` directory. Import from crate root: `use wenv::model::...`.

### Naming
`test_<unit_under_test>_<scenario>` — descriptive names are expected.

```rust
#[test]
fn test_entry_merge_trailing_comment_absorbs_blank() { ... }
#[test]
fn test_build_visible_list_collapsed() { ... }
```

### Assertions
`assert_eq!`, `assert!`, `matches!`. No custom assertion macros.

### Running a Single Test
```bash
cargo test test_entry_merge              # By substring match
cargo test --lib test_pending_block      # Lib tests only
cargo test --test profile_tests          # Specific integration file
```

## Architecture

```
src/
  main.rs, lib.rs         # Entry points (lib re-exports public API)
  cli/                    # Clap argument parsing and CLI actions
  config/                 # TOML config load/save, path resolution
  formatter/              # Formatter trait + Bash/PowerShell impls
  i18n/                   # Internationalization (embedded EN, external files)
  model/                  # Data types: Entry, ShellType, ShellProfile, Config
  parser/                 # Parser trait + Bash/PowerShell impls, pending blocks
  tui/                    # Full TUI: app state, rendering, keys, operations
  utils/                  # Path expansion, shell detection, string helpers
tests/                    # Integration tests (TUI logic, parser fixes, config)
```

## Key Semantics

### Entry Value Field
Stores **complete raw syntax** — never strip keywords or quotes:
- Alias: `"alias -g ll='ls -la'"` (not just the value)
- EnvVar: `"export PATH=\"/usr/bin\""` (not just the path)
- Function: full definition including `name() { ... }`
- Source: `"source ~/.profile"` (not just the path)

### Newline Formats (Critical — prevents off-by-one bugs)
| Format | Rule | Example | Used By |
|--------|------|---------|---------|
| **Separator** | N lines = N-1 `\n` | `"line1\nline2"` | `value`, `value_buffer` |
| **Terminator** | N lines = N `\n` | `"line1\nline2\n"` | File contents |

- Split `value` with `value.split('\n')`, **never** `.lines()` or `split_lines_preserve_trailing()`.
- When writing separator-format content to file, append `\n` terminator.

### Lenient Parsing
Parsers skip unparseable lines with warnings and continue. This is intentional.

## Workflow

1. Confirm you understand the intent before proceeding.
2. Check `git status` for new feature work — confirm sync with remote.
3. Implement precisely what is needed — no over-engineering.
4. Run `cargo clippy -- -D warnings && cargo fmt -- --check && cargo test` after changes.
5. Append `Unreleased Update` to `CHANGELOG.md` and update `CLAUDE.md`/`README.md` as needed.
