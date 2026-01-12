//! wenv - Shell Configuration File Manager
//!
//! A cross-platform tool for managing shell RC files (Bash, PowerShell).
//!
//! # Features
//!
//! - Parse and list aliases, functions, environment variables, and source files
//! - Check for duplicate definitions
//! - Add, remove, and edit entries
//! - Import from files and URLs
//! - Export entries
//! - Format configuration files
//! - Automatic backups

pub mod backup;
pub mod checker;
pub mod cli;
pub mod config;
pub mod formatter;
pub mod i18n;
pub mod model;
pub mod parser;
pub mod tui;
pub mod utils;

pub use checker::check_all;
pub use formatter::{get_formatter, Formatter};
pub use model::{Config, Entry, EntryType, ParseResult, ShellType};
pub use parser::{get_parser, Parser};
