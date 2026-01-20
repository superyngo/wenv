//! # Parser Module
//!
//! This module provides shell configuration file parsing capabilities.
//!
//! ## Architecture Overview
//!
//! ```text
//! parser/
//! ├── mod.rs              - This file: Parser trait + factory function
//! ├── bash/               - Bash shell parser
//! │   ├── mod.rs          - BashParser struct + Parser impl
//! │   ├── patterns.rs     - Regex patterns (ALIAS_*, FUNC_*, etc.)
//! │   ├── control.rs      - Control structure detection (if/fi, etc.)
//! │   └── parsers.rs      - Individual parse methods
//! ├── pwsh/               - PowerShell parser (same structure)
//! │   ├── mod.rs
//! │   ├── patterns.rs
//! │   ├── control.rs
//! │   └── parsers.rs
//! └── builders/           - Multi-line entry builders
//!     ├── mod.rs          - Exports + utility functions
//!     ├── function.rs     - FunctionBuilder (brace counting)
//!     ├── code_block.rs   - CodeBlockBuilder (control structures)
//!     ├── quoted.rs       - QuotedValueBuilder (multi-line quoted values)
//!     └── comment.rs      - CommentBlockBuilder (adjacent comments)
//! ```
//!
//! ## Quick Reference
//!
//! | Builder | Entry Types | Start Detection | End Detection |
//! |---------|-------------|-----------------|---------------|
//! | [`builders::FunctionBuilder`] | Function | `func() {` | brace_count = 0 |
//! | [`builders::CodeBlockBuilder`] | Code | `if`/`while`/`for` | `fi`/`done`/`esac` |
//! | [`builders::QuotedValueBuilder`] | Alias, EnvVar | Odd single quotes | Even quotes |
//! | [`builders::CommentBlockBuilder`] | Comment | `#` line | Non-`#` line |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use wenv::parser::{get_parser, Parser};
//! use wenv::model::ShellType;
//!
//! // Get appropriate parser
//! let parser = get_parser(ShellType::Bash);
//!
//! // Parse content
//! let content = std::fs::read_to_string("~/.bashrc")?;
//! let result = parser.parse(&content);
//!
//! // Access entries
//! for entry in result.entries {
//!     println!("{}: {} = {}", entry.entry_type, entry.name, entry.value);
//!     if let Some(end) = entry.end_line {
//!         println!("  Lines: {}-{}", entry.line_number.unwrap(), end);
//!     }
//! }
//! ```
//!
//! ## How to Modify
//!
//! ### Adding a new Entry type
//!
//! 1. Define the type in `src/model/entry.rs` (`EntryType` enum)
//! 2. Add regex pattern in `bash/patterns.rs` or `pwsh/patterns.rs`
//! 3. Add parse method in `bash/parsers.rs` or `pwsh/parsers.rs`
//! 4. Integrate into main loop in `bash/mod.rs` or `pwsh/mod.rs`
//!
//! ### Adding a new Builder
//!
//! 1. Create `builders/new_builder.rs` following existing patterns
//! 2. Export from `builders/mod.rs`
//! 3. Use in parser's main loop

mod bash;
pub mod builders;
pub mod pending;
mod pwsh;

pub use bash::BashParser;
pub use pwsh::PowerShellParser;

use crate::model::{ParseResult, ShellType};

/// Trait for shell configuration parsers.
///
/// Implement this trait to add support for new shell types.
///
/// ## Required Methods
///
/// - [`parse`](Parser::parse) - Parse configuration file content
/// - [`shell_type`](Parser::shell_type) - Return the shell type this parser handles
///
/// ## Example Implementation
///
/// ```rust,ignore
/// pub struct MyShellParser;
///
/// impl Parser for MyShellParser {
///     fn parse(&self, content: &str) -> ParseResult {
///         // Parse logic here
///     }
///
///     fn shell_type(&self) -> ShellType {
///         ShellType::MyShell
///     }
/// }
/// ```
pub trait Parser: Send + Sync {
    /// Parse configuration file content and extract entries.
    ///
    /// # Arguments
    ///
    /// - `content`: The full content of the configuration file
    ///
    /// # Returns
    ///
    /// A [`ParseResult`] containing:
    /// - `entries`: Vec of parsed entries (aliases, functions, etc.)
    /// - `warnings`: Vec of any parse warnings encountered
    fn parse(&self, content: &str) -> ParseResult;

    /// Get the shell type this parser handles.
    fn shell_type(&self) -> ShellType;
}

/// Factory function to get the appropriate parser for a shell type.
///
/// # Arguments
///
/// - `shell_type`: The type of shell to get a parser for
///
/// # Returns
///
/// A boxed [`Parser`] implementation for the specified shell type.
///
/// # Example
///
/// ```rust,ignore
/// let parser = get_parser(ShellType::Bash);
/// let result = parser.parse(content);
/// ```
pub fn get_parser(shell_type: ShellType) -> Box<dyn Parser> {
    match shell_type {
        ShellType::Bash | ShellType::Zsh => Box::new(BashParser::new()),
        ShellType::PowerShell => Box::new(PowerShellParser::new()),
    }
}

// Re-export commonly used items for convenience
pub use builders::{CommentBlockBuilder, QuotedValueBuilder};
