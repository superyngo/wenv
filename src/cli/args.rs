//! CLI argument definitions

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "wenv")]
#[command(about = "Shell configuration file manager")]
#[command(version, author)]
pub struct Cli {
    /// Specify shell type
    #[arg(short, long, global = true)]
    pub shell: Option<ShellArg>,

    /// Use an alternate config.toml location (default: ~/.config/wenv/config.toml)
    #[arg(short, long, value_name = "PATH", global = true)]
    pub config: Option<std::path::PathBuf>,

    /// UI language for this run only (e.g. en, zh-TW); overrides config.toml
    #[arg(long, value_name = "LANG", global = true)]
    pub lang: Option<String>,

    /// Disable colors for this run (same as setting NO_COLOR)
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Open source file in $EDITOR (same as "wenv .")
    #[arg(long)]
    pub source: bool,

    /// "." to open editor
    #[arg(value_name = "COMMAND")]
    pub command: Option<String>,

    #[command(subcommand)]
    pub subcommand: Option<SubCmd>,
}

#[derive(Subcommand)]
pub enum SubCmd {
    /// Open wenv config file in $EDITOR
    Config,
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
