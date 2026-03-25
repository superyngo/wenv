//! CLI argument definitions

use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(name = "wenv")]
#[command(about = "Shell configuration file manager")]
#[command(version, author)]
pub struct Cli {
    /// Specify shell type
    #[arg(short, long)]
    pub shell: Option<ShellArg>,

    /// Open source file in $EDITOR (same as "wenv .")
    #[arg(long, group = "action")]
    pub source: bool,

    /// Open wenv config file in $EDITOR
    #[arg(short = 'c', long, group = "action")]
    pub config: bool,

    /// "." to open editor
    #[arg(value_name = "COMMAND")]
    pub command: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellArg {
    Bash,
    Zsh,
    Pwsh,
}

impl From<ShellArg> for crate::model::ShellType {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => crate::model::ShellType::Bash,
            ShellArg::Zsh => crate::model::ShellType::Zsh,
            ShellArg::Pwsh => crate::model::ShellType::PowerShell,
        }
    }
}
