//! CLI command implementations

pub mod add;
pub mod backup;
pub mod check;
pub mod edit;
pub mod export;
pub mod format;
pub mod import;
pub mod info;
pub mod list;
pub mod remove;
pub mod tui_cmd;

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::cli::args::{Cli, ConflictStrategy};
use crate::i18n::{init_messages, Language, Messages};
use crate::model::{Config, ShellType};
use crate::utils::shell_detect::get_shell_type;

/// Common context for command execution
pub struct CommandContext {
    pub config: Config,
    pub shell_type: ShellType,
    pub config_file: PathBuf,
    pub on_conflict: ConflictStrategy,
    pub messages: &'static Messages,
}

impl CommandContext {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let config = crate::config::load_or_create_config()?;

        // Initialize i18n based on config
        let lang: Language = config.ui.language.parse().unwrap_or_default();
        let messages = init_messages(lang);

        let shell_type = get_shell_type(cli.shell.map(|s| s.into()), cli.file.as_deref());

        let config_file = cli
            .file
            .clone()
            .unwrap_or_else(|| shell_type.default_config_path());

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
            ShellType::Bash => format!("source {}", self.config_file.display()),
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

    /// Get a BackupManager instance
    pub fn get_backup_manager(&self) -> crate::backup::BackupManager {
        crate::backup::BackupManager::new(self.shell_type, &self.config)
    }

    /// Find an entry by type and name
    pub fn find_entry<'a>(
        &self,
        entries: &'a [crate::model::Entry],
        entry_type: crate::model::EntryType,
        name: &str,
    ) -> Option<&'a crate::model::Entry> {
        entries
            .iter()
            .find(|e| e.entry_type == entry_type && e.name == name)
    }

    /// Color an entry type for display
    pub fn color_entry_type(&self, entry_type: crate::model::EntryType) -> colored::ColoredString {
        use crate::model::EntryType;
        let type_str = format!("{}", entry_type);
        match entry_type {
            EntryType::Alias => type_str.green(),
            EntryType::Function => type_str.blue(),
            EntryType::EnvVar => type_str.yellow(),
            EntryType::Source => type_str.magenta(),
            EntryType::Code => type_str.cyan(),
            EntryType::Comment => type_str.white(),
        }
    }
}
