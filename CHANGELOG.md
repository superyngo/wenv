# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **I18n: External Language File Support**: Support loading translations from `~/.config/wenv/i18n/{lang}.toml` (2026-01-24)
  - Binary only embeds English (reduces size, keeps fallback)
  - Set `language = "zh-TW"` in config to load external zh-TW.toml
  - Falls back to English if external file missing or invalid
  - Community can provide translations without recompiling
  - Add `language` field to `UiConfig` (defaults to "en")

### Added
- **I18n: Traditional Chinese Translation**: Add complete zh-TW translation file with all 230+ message keys (2026-01-23)
  - Full UI localization including format preview and toggle comment messages
  - File location: `~/.config/wenv/i18n/zh-TW.toml` or `assets/i18n/zh-TW.toml`
- **I18n: Format Preview \u0026 Toggle Comment Messages**: Add i18n support for format preview and toggle comment UI messages (2026-01-23)
  - Format preview sorting messages (aliases, functions, environment variables, sources)
  - Grouping entries message
  - "No changes needed" message
  - Toggle comment status messages (single/multiple entries)
- **CLI: Config File Editor**: Add `-c/--config` flag to open wenv config file in $EDITOR (2026-01-23)
  - Uses $EDITOR environment variable (defaults to vi/notepad)
  - Quick access to edit language, format, and backup settings
- **I18n: TOML Language Files**: Refactor i18n from hardcoded Rust to TOML-based language files (2026-01-23)
  - English messages embedded in binary as fallback (`assets/i18n/en.toml`)
  - External language files loaded from `~/.config/wenv/i18n/{lang}.toml`
  - Supports dynamic loading without recompilation
  - **TUI fully localized**: All UI elements now use i18n
    - Status bars for all modes (Normal, Searching, Editing, Moving, etc.)
    - All popups (Search, Detail, Help, Confirm, Type Selection, Edit)
    - Dynamic status messages (Selection cleared, Moving entries, etc.)
    - Help popup with complete keyboard shortcuts
    - Over 60+ new message keys added for comprehensive coverage
  - Sample Chinese (zh-TW) translation included
  - Note: CLI command output messages still use hardcoded English
- **I18n: Popup Translations & Key Consolidation** (2026-01-23)
  - 彈窗翻譯改善：Add Entry、編輯值、條目詳情的快捷鍵提示完整中文化
  - 新增多個 i18n 鍵值支援彈窗元素國際化
  - 合併重複的 i18n 鍵值：
    - `tui_detail_type/name/value` + `tui_edit_field_type/name/value` → `label_type/name/value`
    - `tui_entry_added` + `tui_msg_entry_added` → `msg_entry_added`
    - `file_formatted` + `tui_file_formatted` → `msg_file_formatted`

### Fixed
- **I18n: TOML Duplicate Keys**: Fix TOML parsing errors caused by duplicate keys (2026-01-23)
  - Removed duplicate help field definitions from TOML files
  - Cleaned up old "TUI help text" section that conflicted with new detailed shortcuts
  - Synchronized Messages/MessagesToml structs with actual TOML file contents
  - Updated ui.rs to use correct new field names (tui_help_format_file, tui_help_help_key)
- **Language Config**: Add `cht`/`chs` as recognized language abbreviations (2026-01-23)
  - `cht` now maps to Traditional Chinese (zh-TW)
  - `chs` now maps to Simplified Chinese (zh-CN)
- **Format Order Config**: Fix `format.order.types` config being ignored (2026-01-23)
  - Format command now respects configured type order (e.g., `["env", "alias", "func", "source"]`)
  - Previously used file's original order of first occurrence instead of config

## [0.7.2] - 2026-01-23

### Changed
- **Performance: Binary Size Reduction**: Optimized release build configuration and replaced heavy dependencies (2026-01-23)
  - Binary size reduced from 9.5MB to 4.1MB (56.8% reduction)
  - Replaced `reqwest` with `ureq` for lighter HTTP handling
  - Replaced `chrono` with `time` crate for smaller footprint
  - Added release profile with `opt-level='z'`, `lto=true`, `strip=true`

### Added
- **Performance: PowerShell Path Caching**: Cache PowerShell profile paths to avoid slow shell startup on Windows (2026-01-23)
  - First run queries PowerShell for $PROFILE path and caches result in `~/.config/wenv/.path_cache.toml`
  - Subsequent runs read from cache (expected ~10x speedup on Windows)
  - Cache validates path existence and re-queries if invalid
  - New `--clear-cache` flag to force cache refresh
- **Performance: Backup Cleanup Frequency Control**: Reduce backup cleanup overhead (2026-01-23)
  - Cleanup now runs every 10 backups or after 1 hour (whichever comes first)
  - Previously ran on every backup operation

## [0.7.1] - 2026-01-22

### Changed
- **TUI Unified Selection Mode**: Simplified selection system with single unified mode (2026-01-22)
  - 's' key and Shift+arrow both enter selection mode
  - Arrow keys move cursor without clearing selection in selection mode
  - ESC is the only way to exit selection mode and clear selections
  - Shift+arrow now extends selection without clearing existing non-contiguous selections
  - Releasing Shift and pressing again from new position establishes new anchor
  - Removed distinction between "contiguous" and "non-contiguous" selection modes

### Fixed
- **TUI Move Mode Selection State**: Fixed selection state tracking when cancelling move operation (2026-01-22)
  - ESC in move mode now correctly restores selection to original entries instead of moved positions
  - Example: Selecting entries 1,3,5 → moving to position 8 → ESC now keeps 1,3,5 selected (not 8,9,10)
  - Move confirmation now properly clears saved state to prevent memory leaks

## [0.7.0] - 2026-01-21

### Fixed
- **Bash EnvVar Empty Value Formatting**: Fixed empty environment variables to use quoted format `export VAR=''` instead of `export VAR=` (2026-01-21)
- **TUI Editing Stability**: Fixed potential crashes when editing Function/EnvVar entries to empty values (2026-01-21)
  - Use `split_lines_preserve_trailing()` to correctly handle line counts
  - Add safety check to prevent empty formatter output
- **TUI Source NAME Auto-Extraction**: Fixed formatter to write name as comment suffix when saving Source entries (2026-01-21)
  - Source entries with custom names are now saved as `source path # name`
  - Applies to both Bash (`source path # name`) and PowerShell (`. path # name`)
  - Line number pattern names (e.g., "L10") are ignored and not written as comments
- **TUI External Editor Dirty Flag**: Fixed ESC confirmation popup not appearing after editing with 'o' key (2026-01-21)
  - `dirty` flag is now set when external editor modifies the temp file
  - ESC now correctly prompts for save confirmation after external edits
- **TUI Multi-Select Move**: Fixed entries disappearing when moving multiple selected entries (2026-01-21)
  - `confirm_move` now regenerates file from current entry order using formatter
  - Avoids issues with outdated line numbers by rebuilding entire file content

### Added
- **TUI Source Path Validation**: Validate Source entry paths for empty and invalid characters (2026-01-21)
- **TUI Source NAME Auto-Extraction**: Automatically extract filename as NAME from Source path when NAME is empty (2026-01-21)
- **TUI PowerShell Alias Value Validation**: Validate that PowerShell alias values are not empty (2026-01-21)
- **TUI Non-Contiguous Selection Mode**: Press 's' to enter selection mode and toggle individual entries (2026-01-21)
  - 's' key toggles non-contiguous selection mode and toggles current entry in/out of selection
  - Navigate with arrow keys without clearing selections
  - Delete/move operations work on all selected entries
  - Escape key exits non-contiguous mode
  - Switching to search mode ('f') automatically clears non-contiguous selections
- **TUI Open Temp File**: Press 'o' to open the complete temp file in $EDITOR (2026-01-21)
  - Opens full temp file (not limited to specific entry)
  - TUI suspends during editing and resumes after editor exits
  - Changes are automatically reloaded into TUI
- **TUI Non-Contiguous Move Consolidation**: When moving multiple non-contiguous entries, they are automatically consolidated into a continuous block (2026-01-21)
  - All selected entries move to follow the first selected entry
  - Entries become continuous and can be moved as a block
  - ESC key uses undo stack to restore original positions

### Changed
- **TUI List Value Display Length**: Increased value display length from 40 to 100 characters (2026-01-21)
- **PowerShell Alias Format**: Wrap alias values with single quotes for consistent quoting (2026-01-21)
  - `Set-Alias ll Get-ChildItem` → `Set-Alias ll 'Get-ChildItem'`
- **TUI List Rendering**: Refactored header offset to use `LIST_HEADER_OFFSET` constant (2026-01-21)
- **CLI Short Flag for --shell**: Changed from `-S` to `-s` (BREAKING) (2026-01-21)
  - `-s` now maps to `--shell` instead of `--source`
  - To open editor, use `--source` (full flag) or `wenv .` (positional argument)
- **TUI Save Keybinding**: Changed from 's' to 'w' (2026-01-21)
  - 'w' now saves to original file
  - Ctrl+S still works as alternative
  - 's' key freed for non-contiguous selection mode
- **TUI Column Order**: Reordered columns from TYPE→NAME→LINE→VALUE to NAME→TYPE→LINE→VALUE (2026-01-21)
  - Applies to main list view and delete confirmation popup
  - NAME column now appears first for better readability
- **Parser Signature Standardization**: Unified parse method signatures across all shell parsers (2026-01-21)
  - Introduced `ParseEvent` enum as standardized return type for all `try_parse_*` functions
  - ParseEvent variants: `Complete(Entry)`, `Started { entry_type, name, boundary, first_line }`, `None`
  - Renamed Bash `try_parse_export` → `try_parse_env` for consistency with PowerShell
  - Removed custom result enums (`AliasParseResult`, `ExportParseResult`) in favor of unified `ParseEvent`
  - PowerShell `try_parse_env` now returns `ParseEvent::Started` for Here-String detection (previously separate `detect_env_heredoc_start`)
  - All parsers now follow standard signatures: `try_parse_alias/env/source(line, line_num) -> ParseEvent`
  - Benefits: Easier to add new shell parsers (e.g., ZshParser, FishParser) with clear contract
  - Internal change only - no user-facing API modifications
- **TUI Selection Unification**: Unified selection mechanism to use only `selected_indices` for both continuous and non-contiguous selections (2026-01-21)
  - Removed `selected_range` field in favor of single `selected_indices` HashSet
  - Shift+Up/Down now populates `selected_indices` instead of maintaining separate range
  - Simplified selection logic and eliminated inconsistencies between selection modes

### Fixed
- **PowerShell Source Detection**: Allow any file path for dot-sourcing, not just `.ps1` files (2026-01-21)
  - `. .\config` is now correctly recognized as Source entry
- **TUI Screen Tearing on Shell Validation Failure**: Fixed screen corruption when shell validation command fails (2026-01-21)
  - Root cause: `eprintln!()` output to stderr during TUI mode corrupts display
  - Fix: Return error through UI message popup instead of printing to stderr
- **PowerShell Parser Comment/Code Merge**: Fixed missing Comment/Code merging and upgrade logic in PowerShell parser (2026-01-21)
  - PowerShell parser now correctly merges Comment + non-blank Code → Code (type upgrade)
  - Non-blank Code entries now absorb trailing blank lines
  - Control structures now absorb trailing blank lines
  - Comments can merge into control structures (seed block with pending content)
  - Aligned PowerShell parser behavior with Bash parser for consistent Comment/Code handling
- **TUI Screen Artifacts After External Editor**: Fixed black borders and artifacts after returning from external editor ('o' key) (2026-01-21)
  - Added full terminal redraw after resuming from editor
  - Ensures clean screen without leftover UI artifacts

### Improved
- **TUI Temp File Change Detection**: External editor changes are now detected before reloading (2026-01-21)
  - Compares file modification time before and after editor invocation
  - Only reloads if file was actually modified
  - Shows appropriate status message ("No changes detected" vs "Temp file reloaded after editing")

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
