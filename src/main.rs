//! wenv - Shell Configuration File Manager

use anyhow::Result;
use clap::Parser;

use wenv::cli::{actions, Cli, Context};
use wenv::tui::TuiApp;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = Context::from_cli(&cli)?;

    // Quick actions: execute and exit
    if let Some(source) = &cli.import {
        return actions::import::execute(&ctx, source, cli.yes);
    }
    if let Some(output) = &cli.export {
        return actions::export::execute(&ctx, cli.r#type, output);
    }
    if cli.source {
        return actions::source::execute(&ctx);
    }

    // Default: launch TUI
    TuiApp::new(ctx.config_file, ctx.shell_type, ctx.messages)?.run()
}
