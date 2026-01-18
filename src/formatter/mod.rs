//! Formatter module for shell configuration files

mod bash;
pub mod indent;
mod pwsh;

pub use bash::BashFormatter;
pub use pwsh::PowerShellFormatter;

use crate::model::{Config, Entry, ShellType};

/// Trait for shell configuration formatters
pub trait Formatter {
    /// Format entries into shell configuration format
    fn format(&self, entries: &[Entry], config: &Config) -> String;

    /// Format a single entry
    fn format_entry(&self, entry: &Entry) -> String;

    /// Get the shell type this formatter handles
    fn shell_type(&self) -> ShellType;
}

/// Get a formatter for the specified shell type
pub fn get_formatter(shell_type: ShellType) -> Box<dyn Formatter> {
    match shell_type {
        ShellType::Bash | ShellType::Zsh => Box::new(BashFormatter::new()),
        ShellType::PowerShell => Box::new(PowerShellFormatter::new()),
    }
}
