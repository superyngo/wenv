//! Parser module for shell configuration files

mod bash;
pub mod common;
mod pwsh;

pub use bash::BashParser;
pub use pwsh::PowerShellParser;

use crate::model::{ParseResult, ShellType};

/// Trait for shell configuration parsers
pub trait Parser {
    /// Parse configuration file content
    fn parse(&self, content: &str) -> ParseResult;

    /// Get the shell type this parser handles
    fn shell_type(&self) -> ShellType;
}

/// Get a parser for the specified shell type
pub fn get_parser(shell_type: ShellType) -> Box<dyn Parser> {
    match shell_type {
        ShellType::Bash => Box::new(BashParser::new()),
        ShellType::PowerShell => Box::new(PowerShellParser::new()),
    }
}
