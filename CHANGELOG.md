# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.2] - 2026-01-20

### Changed
- **Code Simplification**: Removed deprecated and unused code
  - Deleted deprecated `FunctionBuilder` and `CodeBlockBuilder` (replaced by `PendingBlock`)
  - Removed placeholder `SyntaxChecker` (always returned empty results)
  - Extracted common `find_attached_comments` function from BashFormatter and PowerShellFormatter to shared module
  - Reduced code duplication by ~100 lines
- **Parser Refactored to Unified PendingBlock State Machine**: Major refactoring of parser architecture
  - Created `src/parser/pending.rs` with unified `PendingBlock` and `BoundaryType` abstractions
  - Both BashParser and PowerShellParser now use the same pending state machine pattern
  - Removed `Entry.comment` field (redundant with independent Comment entry type)
  - Removed `pending_comment` state variables from parsers
  - BoundaryType enums: Complete, BraceCounting, QuoteCounting, KeywordTracking, AdjacentMerging
  - Core concept: "First delimit the boundary of each entry, then produce the entry"
  - All entries now pass through pending state machine before becoming Entry objects

### Fixed
- **TUI Comment/Code Save Trailing Blank Line Loss**: Fixed trailing blank lines lost when saving Comment/Code entries
  - Root cause: `replace_line_range` had complex logic to decide whether to add terminator
  - Fix: Simplified to always add `\n` after replacement (value_buffer is separator format)
- **Parser Trailing Blank Line Preservation**: Fixed trailing blank lines being lost during parsing
  - Root cause: `content.lines()` treats `\n` as line terminator, so `"#\n\n"` → 2 elements instead of 3
  - Fix: Use `content.split('\n')` and handle file terminator vs entry trailing blanks correctly
  - Applied to both BashParser and PowerShellParser
  - Also fixed `replace_line_range` in TUI to use consistent line counting logic
- **Multi-line Alias Parsing Precedence**: Fixed multi-line alias detection being bypassed by noquote pattern
  - Root cause: `ALIAS_NOQUOTE_RE` was checked before `ALIAS_MULTILINE_START_RE` in `try_parse_alias()`
  - This caused `alias test1='123` to be incorrectly parsed as complete alias with value `'123` instead of multi-line start
  - Fix: Move multi-line check before noquote check, matching the order in `try_parse_export()`
- **Multi-line Export Formatter**: Use single quotes for multi-line env vars to match parser expectations
  - Root cause: `format_export()` used double quotes for multi-line values, but parser only detects single-quote multi-line
  - Fix: Prefer single quotes in `format_export()` like `format_alias()`, fall back to double quotes only when value contains single quotes

### Fixed
- **Formatter Preserves Original Format**: Unedited Alias/EnvVar/Source entries now preserve original format using `raw_line` (2026-01-20)
  - Root cause: `format_entry()` always rebuilt from `name`/`value`, potentially losing original quoting style
  - Fix: Prioritize `raw_line` in `format_entry()` for Alias/EnvVar/Source types when available
  - Edited entries (no `raw_line`) correctly use formatting methods
- **Multiline Alias/EnvVar Writing**: Fixed multiline value formatting in Bash/PowerShell formatters (2026-01-20)
  - Added multiline detection in `format_alias()`: uses single quotes when safe, double quotes with escaping when needed
  - Added multiline detection in `format_export()`: uses double quotes with proper escaping
  - This ensures edited multiline values produce valid shell syntax
- **Trailing Empty Lines Loss Bug**: Fixed issue where trailing empty lines in Comment-prefixed merged entries would be lost on each save (2026-01-19)
  - Root cause 1: Parser used `split_lines_preserve_trailing` which incorrectly handled `raw_line` format (N lines with N-1 newlines)
  - Fix: Use `raw.split('\n')` directly for `raw_line` since newlines are separators, not terminators
  - Root cause 2: TUI update used line-based manipulation which lost line count during replacement
  - Fix: For Comment/Code entries, use direct byte-range replacement (`replace_line_range`) instead of line-based manipulation
  - This preserves exact content including all trailing empty lines without any parsing/formatting

### Changed
- **TUI Comment/Code Display Enhancement**: Use `raw_line` for Comment/Code entries in info view and delete prompts (2026-01-19)
  - Detail popup now shows full `raw_line` content for multi-line Comment/Code entries
  - Single-entry delete confirmation shows full `raw_line` for Comment/Code
  - Multi-entry delete confirmation now shows TYPE NAME LINE VALUE columns (matching main list format)
- **Parser Comment+Control Structure Merge**: Comment blocks preceding control structures now merge into single Code entry (2026-01-19)
  - Comment + (optional blank lines) + control structure (if/while/for/case) → merged Code entry
  - Completed control blocks now become pending to absorb trailing blank lines
- **Parser Merge Rules Enhancement**: Enhanced Comment/Code merge behavior (2026-01-19)
  - Comment + non-blank Code + trailing blank lines now merge as single Code entry (previously blank lines caused separation)
  - Code entry value now displays first line (Comment's content for upgraded entries) for consistent TUI list display
  - `raw_line` remains source of truth containing complete original content
- **TUI Move Logic Fix**: Refactored move operation using marker-based approach (2026-01-19)
  - Fixed issue where moving entries down would insert at wrong position due to line number invalidation after extraction
  - Uses unique marker to calculate correct insert position before content modification
- **Parser Comment/Code Raw String Simplification**: Refactored Comment/Code handling to use raw_line as source of truth (2026-01-19)
  - Comment and Code entries now store full raw line in `value` field (including leading whitespace)
  - Removed `.comment` field storage in `merge_trailing` - `raw_line` contains complete original content
  - TUI editing for Comment/Code now uses `raw_line` to preserve comments and empty lines
  - Fixed cursor navigation for trailing newlines in multi-line value editing
  - Type upgrade logic preserved: Comment + non-blank Code → Code

## [0.6.1] - 2026-01-18

### Changed
- **TUI Delete Confirmation**: Enhanced delete popup to show full entry details (Type, Line, Name, Value) for single entry deletion, and summary list for multi-select deletion. Added scroll support for reviewing long entries before deletion.

## [0.6.0] - 2026-01-18

### Added
- **Zsh Shell Support**: Auto-detect Zsh from `$SHELL` environment variable and file patterns (`.zshrc`, `.zprofile`, `.zshenv`, etc.)
- **Positional File Path Argument**: Allow specifying config file path as positional argument (e.g., `wenv /path/to/file`)

## [0.5.1] - 2026-01-17

### Fixed
- **TUI Search Mode ESC Key**: ESC now properly closes search mode (clears highlights) instead of quitting the application when search is active

## [0.5.0] - 2026-01-16

### Added
- **TUI Search Mode**: Press `f` to search entries by Name and Value with PageUp/PageDown navigation between matches
  - Search input with live match count display
  - Highlighted search matches in the entry list
  - Persistent search query even after exiting search mode
  - Navigate matches with PageUp/PageDown

### Changed
- Format function key changed from `f` to `r` to accommodate search mode

## [0.4.1] - 2026-01-15

### Fixed
- **Windows Path Encoding**: Fixed issue where non-ASCII characters in PowerShell profile path (e.g., "Documents" in other languages) were incorrectly decoded, causing "file not found" errors.

## [0.4.0] - 2026-01-15

### Added
- **Undo/Redo support**: Full undo/redo functionality with Ctrl+Z/Ctrl+Y
  - Tracks up to 50 operations in history
  - Works for all temp file modifications (add, edit, delete, move, paste, toggle comment)
  - Memory-efficient snapshot-based implementation
  - No impact on visual buffer rendering
- **Editor Shortcut**: Support `wenv .` to quickly open the configuration file in the default editor
- **Copy/Paste Shortcuts**: Added Alt+C/V as alternative keybindings for copy/paste in TUI

### Changed
- **PowerShell Detection**: Improved profile path detection to better respect standard Windows PowerShell locations
- **TUI UX**: Simplified save hotkey and visual improvements

## [0.3.0] - 2026-01-14

### Added
- **PowerShell Here-String support**: Environment variables can now use multi-line Here-String syntax (`$env:VAR = @"...@"`)
- **Comment association**: Comments immediately before entries now follow their associated entries when reordering
- **Integration tests**: Added PowerShell heredoc integration tests

### Changed
- TUI interface enhancements for better user experience
- Improved PowerShell parser to handle multi-line environment variable values
- Enhanced Bash and PowerShell formatters to preserve comment-entry relationships

## [0.2.0] - 2026-01-14

### Added
- Manual workflow dispatch trigger for GitHub Actions release builds

### Fixed
- Windows platform shell detection logic build error

## [0.1.0] - 2026-01-13

### Added
- **Interactive TUI interface**: Full-featured terminal user interface for managing shell configurations
  - Browse and search all entries (aliases, functions, environment variables, source statements, code blocks, comments)
  - Add, edit, and delete entries interactively
  - Real-time duplicate detection and syntax validation
  - Automatic backup before saving changes
- **Bash parser**: Comprehensive support for parsing `.bashrc`, `.bash_profile`, and `.profile`
  - Alias detection with single and double quote support
  - Function detection with multi-line support
  - Environment variable detection (export statements)
  - Source statement detection
  - Code block detection
  - Comment preservation
  - Control structure awareness (skip definitions inside if/while/for/case blocks)
- **PowerShell parser**: Support for PowerShell profile files
  - Alias detection (`Set-Alias`)
  - Function detection with multi-line support
  - Environment variable detection (`$env:VAR`)
  - Source statement detection (`. file.ps1`)
  - Comment preservation
- **Quick action flags**: Non-interactive command-line operations
  - `--import <SOURCE>` - Import entries from file or URL
  - `--export <OUTPUT>` - Export entries to file
  - `--source` - Open configuration file in $EDITOR
  - `--type <TYPE>` - Filter entries by type (for export)
  - `--on-conflict <STRATEGY>` - Handle import conflicts (ask/skip/overwrite)
- **Validation system**:
  - Duplicate entry detection
  - Syntax validation
  - Real-time feedback in TUI
- **Backup system**: Automatic backups before any modifications
  - Backup location: `~/.config/wenv/backups/<shell>/`
  - Timestamped backup files
  - Auto-triggered on save in TUI mode
- **Multi-language support**: English and Traditional Chinese (繁體中文)
- **Cross-platform support**: Windows, Linux, and macOS
- **Shell auto-detection**: Automatically detects shell type from file path and content
- **Lenient parsing**: Skips unparseable lines with warnings instead of failing
- **CLI argument parsing**: Powered by clap with comprehensive help messages
- **URL import support**: Import configurations directly from URLs

### Technical Details
- Trait-based architecture for extensibility (Parser, Formatter, Checker)
- Regex-based parsing with lazy_static patterns
- Builder pattern for multi-line entry construction
- Automatic backup creation before write operations
- Support for both GNU and musl libc on Linux
- TUI powered by ratatui and crossterm
