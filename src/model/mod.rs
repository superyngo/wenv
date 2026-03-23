//! Core data models for wenv

mod config;
mod entry;
mod shell;

pub use config::{Config, FilesConfig, UiConfig};
pub use entry::{Entry, EntryType, ParseResult, ParseWarning};
pub use shell::ShellType;
