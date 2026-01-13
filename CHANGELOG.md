# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
