# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
