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
EntryType { Alias, Function, EnvVar, Source, Code, Comment }
ShellType { Bash, PowerShell }
Entry { entry_type, name, value, line_number, end_line }
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
- `src/backup/mod.rs` - Automatic backup system before write operations
- `src/utils/shell_detect.rs` - Shell type detection from env, extension, filename patterns

## Important Implementation Details

### Lenient Parsing

The parser operates in "lenient mode" - it skips unparseable lines with warnings and continues processing. This is intentional to handle real-world config files with varied syntax.

### Control Structure Awareness

The Bash parser tracks control structure depth (`if`/`while`/`for`/`case`) to only extract top-level definitions, avoiding aliases and functions defined inside conditional blocks.

### Backup System

Backups are automatically created before any write operation in platform-specific backup directories with timestamp naming:
- Linux: `~/.config/wenv/backups/<shell>/`
- macOS: `~/Library/Application Support/wenv/backups/<shell>/`
- Windows: `%APPDATA%\wenv\backups\<shell>\`

### Configuration System

**Config File Locations:**
- Linux: `~/.config/wenv/config.toml`
- macOS: `~/Library/Application Support/wenv/config.toml`
- Windows: `%APPDATA%\wenv\config.toml`

**Config Structure:**
- `[ui]` - UI settings (language selection)
- `[format]` - Formatting rules (indent, grouping, sorting)
- `[backup]` - Backup settings (max_count)
- `[cache]` - PowerShell profile path cache (auto-detected and user-editable)

**PowerShell Path Cache:**
The `[cache]` section stores auto-detected PowerShell profile paths:
```toml
[cache]
pwsh_profile = "/path/to/pwsh/profile.ps1"
powershell_profile = "/path/to/powershell/profile.ps1"
```
- Auto-detected on first run when PowerShell shell type is used
- Migration: Old `.path_cache.toml` files are automatically merged into `config.toml` and removed
- User-editable if manual override needed

**i18n Language Files:**
- External language files: `~/.config/wenv/i18n/{lang}.toml`
- Set language in config: `[ui] language = "zh-TW"`
- English embedded in binary as fallback

### Regex Patterns

Due to Rust regex limitations (no backreferences), the Bash parser uses separate patterns for different quote styles:
- Single-quoted aliases: `alias name='value'`
- Double-quoted aliases: `alias name="value"`
- Unquoted aliases: `alias name=value`

### Entry Value Semantics

All entry types now store complete raw syntax in the `value` field:

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

### Comment/Code Merge Rules

The parser uses a pending entry state machine to merge adjacent Comment/Code entries and structured entries:

| Scenario | Result |
|----------|--------|
| Comment + Comment | Comment (merged) |
| Comment + blank line(s) | Comment (absorbs blanks) |
| Comment + non-blank Code | Code (type upgrade, merged) |
| Comment + Alias/Function/EnvVar/Source | Structured type (merged, comment becomes prefix) |
| Comment + blank + Structured | Structured type (all merged) |
| Comment + blank + Code + blank | Code (all merged, absorbs trailing blanks) |
| Comment + (optional blanks) + control structure | Code (merged into control block) |
| blank + blank | Code (empty, merged) |
| non-blank Code + blank line(s) | Code (absorbs trailing blanks) |
| Control structure ends | Code (becomes pending, absorbs trailing blanks) |
| blank + non-blank Code | **Don't merge** (separate entries) |

Key principles:
- Blank lines can only be absorbed, never actively absorb other content
- Comments can absorb blanks downward; meeting non-blank Code upgrades to Code
- **Upgraded Code entries stay pending** to absorb subsequent blank lines
- Structured entries (Alias, Function, EnvVar, Source) can merge with preceding Comment/blank lines
- **`value` contains complete original content** after merge (all lines including comments, blanks, and syntax)
- `name` and `entry_type` are extracted from the structured/code part for UI purposes
- Formatters directly return `entry.value` (no syntax reconstruction)

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
