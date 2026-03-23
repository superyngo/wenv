//! wenv - Shell Configuration File Manager

use anyhow::Result;
use clap::Parser;

use wenv::cli::{actions, Cli, Context};
use wenv::tui::TuiApp;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --config early (opens wenv config file in editor)
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

    let ctx = Context::from_cli(&cli)?;

    // Quick actions: execute and exit
    if cli.command.as_deref() == Some(".") || cli.source {
        return actions::source::execute(&ctx);
    }

    // Default: launch TUI
    let config_file = ctx.shell_type.default_config_path();
    TuiApp::new(config_file, ctx.shell_type, ctx.messages)?.run()
}
