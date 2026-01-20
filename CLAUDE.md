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

### Entry Parsing Rules

Entries are categorized by how their `value` field is processed during parsing:

**Structured Entries (format processed):**
- `Alias` - extracts name and value, strips quotes
- `Function` - extracts name, body preserved
- `EnvVar` - extracts name and value, strips quotes
- `Source` - extracts file path

**Raw Entries (keep raw string):**
- `Comment` - value keeps full raw line including leading whitespace and `#` prefix
- `Code` - value keeps full raw line including leading whitespace

### Comment/Code Merge Rules

The parser uses a pending entry state machine to merge adjacent Comment and Code entries:

| Scenario | Result |
|----------|--------|
| Comment + Comment | Comment (merged) |
| Comment + blank line(s) | Comment (absorbs blanks) |
| Comment + non-blank Code | Code (type upgrade, merged) |
| Comment + non-blank Code + blank line(s) | Code (merged, type upgrade, absorbs blanks) |
| Comment + (optional blanks) + control structure | Code (merged into control block) |
| blank + blank | Code (empty, merged) |
| non-blank Code + blank line(s) | Code (absorbs trailing blanks) |
| Control structure ends | Code (becomes pending, absorbs trailing blanks) |
| blank + non-blank Code | **Don't merge** (separate entries) |

Key principles:
- Blank lines can only be absorbed, never actively absorb other content
- Comments can absorb blanks downward; meeting non-blank Code upgrades to Code
- **Upgraded Code entries stay pending** to absorb subsequent blank lines
- Non-blank Code absorbs trailing blank lines
- Structured entries (Alias, Function, EnvVar, Source) are never merged with Comment/Code
- **`raw_line` contains complete original content** after merge (all lines including comments, blanks, code)
- **`value` displays first line** for Code entries (original Comment content for upgraded entries)
- UI editing for Comment/Code uses `raw_line` to preserve all original formatting

### 換行符格式規範（分隔符 vs 終止符）

**核心概念**：專案中存在兩種換行符語意，混淆會導致 off-by-one 錯誤。

| 格式 | 規則 | 3 行範例 | 使用場景 |
|------|------|----------|----------|
| **分隔符格式** | N 行 = N-1 個 `\n` | `"line1\n\n"` | `raw_line`, `value_buffer` |
| **終止符格式** | N 行 = N 個 `\n` | `"line1\n\n\n"` | 文件內容 |

**關鍵規則**：
- 分割 `raw_line` 必須用 `raw.split('\n')`，不可用 `.lines()` 或 `split_lines_preserve_trailing()`
- 寫入文件時，分隔符格式內容需額外加終止符
- 判斷「是否以換行結尾」時，要分清是內容的一部分還是終止符

### TUI Comment/Code 編輯保存

使用 `replace_line_range()` 直接替換 entry 佔據的行範圍：
- `value_buffer` 是分隔符格式，寫入時無條件加 `\n` 終止符
