//! CLI argument definitions

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wenv")]
#[command(about = "Shell configuration file manager")]
#[command(version, author)]
pub struct Cli {
    /// Specify configuration file path
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Specify shell type
    #[arg(short = 'S', long)]
    pub shell: Option<ShellArg>,

    /// Conflict handling strategy (for import)
    #[arg(long, default_value = "ask")]
    pub on_conflict: ConflictStrategy,

    /// Import entries from file or URL
    #[arg(short, long, value_name = "SOURCE", group = "action")]
    pub import: Option<String>,

    /// Export entries to file
    #[arg(short, long, value_name = "OUTPUT", group = "action")]
    pub export: Option<PathBuf>,

    /// Open source file in $EDITOR
    #[arg(short, long, group = "action")]
    pub source: bool,

    /// Skip confirmation prompts (for import)
    #[arg(short, long)]
    pub yes: bool,

    /// Filter by entry type (for export)
    #[arg(short, long)]
    pub r#type: Option<EntryTypeArg>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellArg {
    Bash,
    Pwsh,
}

impl From<ShellArg> for crate::model::ShellType {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => crate::model::ShellType::Bash,
            ShellArg::Pwsh => crate::model::ShellType::PowerShell,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum EntryTypeArg {
    #[value(alias = "a")]
    Alias,
    #[value(alias = "f")]
    Func,
    #[value(alias = "e")]
    Env,
    #[value(alias = "s")]
    Source,
    #[value(alias = "c")]
    Code,
    #[value(alias = "cm")]
    Comment,
}

impl From<EntryTypeArg> for crate::model::EntryType {
    fn from(arg: EntryTypeArg) -> Self {
        match arg {
            EntryTypeArg::Alias => crate::model::EntryType::Alias,
            EntryTypeArg::Func => crate::model::EntryType::Function,
            EntryTypeArg::Env => crate::model::EntryType::EnvVar,
            EntryTypeArg::Source => crate::model::EntryType::Source,
            EntryTypeArg::Code => crate::model::EntryType::Code,
            EntryTypeArg::Comment => crate::model::EntryType::Comment,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Default)]
pub enum ConflictStrategy {
    #[default]
    Ask,
    Skip,
    Overwrite,
}
