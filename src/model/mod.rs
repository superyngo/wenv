//! Core data models for wenv

mod config;
mod entry;
pub mod profile;
mod shell;

pub use config::{Config, FilesConfig, Snippet, TemplatePathsConfig, UiConfig};
pub use entry::{Entry, EntryType, ParseResult, ParseWarning};
pub use shell::ShellType;
