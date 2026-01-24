//! Command execution context

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::cli::args::{Cli, ConflictStrategy};
use crate::i18n::{init_messages, Messages};
use crate::model::{Config, ShellType};
use crate::utils::shell_detect::get_shell_type;

/// Common context for command execution
pub struct Context {
    pub config: Config,
    pub shell_type: ShellType,
    pub config_file: PathBuf,
    pub on_conflict: ConflictStrategy,
    pub messages: &'static Messages,
}

impl Context {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let config = crate::config::load_or_create_config()?;
        let messages = init_messages(&config.ui.language);

        // Get path from -f option or positional argument (except ".")
        let provided_path: Option<PathBuf> = cli.file.clone().or_else(|| {
            cli.command
                .as_ref()
                .filter(|c| c.as_str() != ".")
                .map(PathBuf::from)
        });

        let shell_type = get_shell_type(cli.shell.map(|s| s.into()), provided_path.as_deref());
        let config_file = provided_path.unwrap_or_else(|| shell_type.default_config_path());

        Ok(Self {
            config,
            shell_type,
            config_file,
            on_conflict: cli.on_conflict,
            messages,
        })
    }

    /// Read and parse the configuration file
    pub fn parse_config_file(&self) -> Result<crate::model::ParseResult> {
        let content = crate::utils::path::read_file(&self.config_file)?;
        let parser = crate::parser::get_parser(self.shell_type);
        Ok(parser.parse(&content))
    }

    /// Print a success message
    pub fn print_success(&self, message: &str) {
        println!("{} {}", "✓".green(), message);
    }

    /// Print a warning message
    pub fn print_warning(&self, message: &str) {
        println!("{} {}", "⚠".yellow(), message);
    }

    /// Print an error message
    pub fn print_error(&self, message: &str) {
        eprintln!("{} {}", "✗".red(), message);
    }

    /// Print reload hint after modifying configuration
    pub fn print_reload_hint(&self) {
        let reload_cmd = match self.shell_type {
            ShellType::Bash | ShellType::Zsh => format!("source {}", self.config_file.display()),
            ShellType::PowerShell => format!(". {}", self.config_file.display()),
        };
        println!(
            "{} {}",
            "→".cyan(),
            self.messages
                .reload_hint
                .replace("{}", &reload_cmd)
                .dimmed()
        );
    }
}
