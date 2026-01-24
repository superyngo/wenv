//! wenv - Shell Configuration File Manager

use anyhow::Result;
use clap::Parser;
use dialoguer::Confirm;

use wenv::cli::{actions, Cli, Context};
use wenv::tui::TuiApp;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --clear-cache early (doesn't require config file)
    if cli.clear_cache {
        wenv::cache::PathCache::clear()?;
        println!("Cache cleared successfully.");
        return Ok(());
    }

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
