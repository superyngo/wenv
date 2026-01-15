//! wenv - Shell Configuration File Manager

use anyhow::Result;
use clap::Parser;
use dialoguer::Confirm;

use wenv::cli::{actions, Cli, Context};
use wenv::tui::TuiApp;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = Context::from_cli(&cli)?;

    // Check if config file exists, prompt to create if missing
    if !ctx.config_file.exists() {
        if Confirm::new()
            .with_prompt(format!(
                "Config file '{}' not found. Create it?",
                ctx.config_file.display()
            ))
            .default(true)
            .interact()?
        {
            wenv::utils::path::write_file(&ctx.config_file, "")?;
            ctx.print_success(&format!("Created: {}", ctx.config_file.display()));
        } else {
            anyhow::bail!("Config file not found. Use --file to specify a different path.");
        }
    }

    // Quick actions: execute and exit
    if let Some(source) = &cli.import {
        return actions::import::execute(&ctx, source, cli.yes);
    }
    if let Some(output) = &cli.export {
        return actions::export::execute(&ctx, cli.r#type, output);
    }
    if cli.command.as_deref() == Some(".") || cli.source {
        return actions::source::execute(&ctx);
    }

    // Default: launch TUI
    TuiApp::new(ctx.config_file, ctx.shell_type, ctx.messages)?.run()
}
