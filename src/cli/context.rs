//! Command execution context

use anyhow::Result;
use colored::Colorize;

use crate::cli::args::Cli;
use crate::i18n::{init_messages, Messages};
use crate::model::{Config, ShellType};
use crate::utils::shell_detect::get_shell_type;

pub struct Context {
    pub config: Config,
    pub shell_type: ShellType,
    pub messages: &'static Messages,
}

impl Context {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let config = crate::config::load_or_create_config()?;
        let messages = init_messages(&config.ui.language);
        let shell_type = get_shell_type(cli.shell.map(|s| s.into()), None);

        Ok(Self {
            config,
            shell_type,
            messages,
        })
    }

    pub fn print_success(&self, message: &str) {
        println!("{} {}", "✓".green(), message);
    }

    pub fn print_warning(&self, message: &str) {
        println!("{} {}", "⚠".yellow(), message);
    }

    pub fn print_error(&self, message: &str) {
        eprintln!("{} {}", "✗".red(), message);
    }

    pub fn print_reload_hint(&self) {
        let reload_cmd = match self.shell_type {
            ShellType::Bash | ShellType::Zsh => "source <file>".to_string(),
            ShellType::PowerShell => ". <file>".to_string(),
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
