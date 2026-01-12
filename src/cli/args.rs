//! CLI argument definitions using Clap

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wenv")]
#[command(about = "Shell configuration file manager")]
#[command(version)]
#[command(author)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Specify configuration file path
    #[arg(short, long, global = true)]
    pub file: Option<PathBuf>,

    /// Specify shell type
    #[arg(short, long, global = true)]
    pub shell: Option<ShellArg>,

    /// Conflict handling strategy
    #[arg(long, global = true, default_value = "ask")]
    pub on_conflict: ConflictStrategy,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List entries
    #[command(visible_alias = "ls")]
    List {
        /// Entry type: alias|func|env|source (a/f/e/s)
        entry_type: Option<EntryTypeArg>,
    },

    /// Check for issues
    Check,

    /// Show entry details
    #[command(visible_alias = "i")]
    Info {
        /// Entry type: a/f/e/s
        entry_type: EntryTypeArg,
        /// Entry name
        name: String,
    },

    /// Add entry
    Add {
        #[command(subcommand)]
        add_command: AddCommands,
    },

    /// Remove entry
    #[command(visible_alias = "rm")]
    Remove {
        /// Entry type: a/f/e/s
        entry_type: EntryTypeArg,
        /// Entry name
        name: String,
    },

    /// Edit entry
    /// Use "edit ." to open config file in editor
    Edit {
        /// Entry type: a/f/e/s, or use "." to edit config file directly
        entry_type: Option<String>,
        /// Entry name
        name: Option<String>,
    },

    /// Import entries
    Import {
        /// File path or URL
        source: String,
        /// Skip preview confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Export entries
    Export {
        /// Entry type: a/f/e/s
        entry_type: Option<EntryTypeArg>,
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Format configuration file
    Format {
        /// Dry run - show changes without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Backup management
    Backup {
        #[command(subcommand)]
        backup_command: BackupCommands,
    },

    /// Interactive TUI mode
    Tui,
}

#[derive(Subcommand)]
pub enum AddCommands {
    /// Add alias
    #[command(visible_alias = "a")]
    Alias {
        /// NAME=VALUE format
        definition: String,
    },
    /// Add function
    #[command(visible_alias = "f")]
    Func {
        /// Function name
        name: String,
        /// Function body
        body: String,
    },
    /// Add environment variable
    #[command(visible_alias = "e")]
    Env {
        /// NAME=VALUE format
        definition: String,
    },
    /// Add source
    #[command(visible_alias = "s")]
    Source {
        /// File path
        path: String,
    },
}

#[derive(Subcommand)]
pub enum BackupCommands {
    /// List backups
    List,
    /// Restore backup
    Restore {
        /// Backup ID
        id: String,
    },
    /// Clean old backups
    Clean {
        /// Number to keep
        #[arg(long, default_value = "20")]
        keep: usize,
    },
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

impl std::str::FromStr for EntryTypeArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "alias" | "a" => Ok(EntryTypeArg::Alias),
            "func" | "function" | "f" => Ok(EntryTypeArg::Func),
            "env" | "envvar" | "e" => Ok(EntryTypeArg::Env),
            "source" | "s" => Ok(EntryTypeArg::Source),
            "code" | "c" => Ok(EntryTypeArg::Code),
            "comment" | "cm" => Ok(EntryTypeArg::Comment),
            _ => Err(format!(
                "Invalid entry type '{}'. Must be one of: alias (a), func (f), env (e), source (s), code (c), comment (cm)",
                s
            )),
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
