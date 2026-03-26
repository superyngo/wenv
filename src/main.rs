//! wenv - Shell Configuration File Manager

use anyhow::Result;
use clap::Parser;

use wenv::cli::actions;
use wenv::cli::args::Cli;
use wenv::i18n;
use wenv::model;
use wenv::tui::TuiApp;
use wenv::utils::shell_detect::get_shell_type;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Early exit: open wenv config in $EDITOR
    if cli.config {
        let config_path = wenv::Config::config_path();
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });
        std::process::Command::new(&editor)
            .arg(&config_path)
            .status()?;
        return Ok(());
    }

    // Determine shell type (runtime decision, no config dependency)
    let shell_type = get_shell_type(cli.shell.map(|s| s.into()), None);

    // Load or create config
    let mut config = wenv::config::load_or_create_config()?;
    let shell_key = shell_type.config_key();

    // Ensure file list exists for this shell
    if !config.files.contains_key(shell_key) {
        wenv::config::ensure_shell_files(&mut config, shell_key)?;
    }

    let messages = i18n::init_messages(&config.ui.language);

    // Source mode: file selection menu
    let is_source = cli.source || cli.command.as_deref() == Some(".");
    if is_source {
        return actions::source::execute(&config, shell_type, messages);
    }

    // Load shell profile and launch TUI
    let profile = model::profile::load_shell_profile(&config, shell_type)?;
    TuiApp::new(profile, messages)?.run()
}
