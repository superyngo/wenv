//! wenv - Shell Configuration File Manager CLI
//!
//! Entry point for the wenv command-line tool.

use anyhow::Result;
use clap::Parser;

use wenv::cli::args::{Cli, Commands};
use wenv::cli::commands::{self, CommandContext};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create command context
    let ctx = CommandContext::from_cli(&cli)?;

    // Execute command
    match &cli.command {
        Commands::List { entry_type } => {
            commands::list::execute(&ctx, *entry_type)?;
        }
        Commands::Check => {
            commands::check::execute(&ctx)?;
        }
        Commands::Info { entry_type, name } => {
            commands::info::execute(&ctx, *entry_type, name)?;
        }
        Commands::Add { add_command } => {
            commands::add::execute(&ctx, add_command)?;
        }
        Commands::Remove { entry_type, name } => {
            commands::remove::execute(&ctx, *entry_type, name)?;
        }
        Commands::Edit { entry_type, name } => {
            // Special case: "wenv edit ." or "wenv edit" opens config file directly
            if entry_type.as_deref() == Some(".") || (entry_type.is_none() && name.is_none()) {
                commands::edit::edit_config_file_directly(&ctx)?;
            } else if let (Some(et_str), Some(n)) = (entry_type, name) {
                // Parse the entry type string
                let et = et_str
                    .parse::<wenv::cli::args::EntryTypeArg>()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                commands::edit::execute(&ctx, et, n)?;
            } else {
                anyhow::bail!("Edit command requires both entry type and name, or use 'edit .' to open config file");
            }
        }
        Commands::Import { source, yes } => {
            commands::import::execute(&ctx, source, *yes)?;
        }
        Commands::Export { entry_type, output } => {
            commands::export::execute(&ctx, *entry_type, output)?;
        }
        Commands::Format { dry_run } => {
            commands::format::execute(&ctx, *dry_run)?;
        }
        Commands::Backup { backup_command } => {
            commands::backup::execute(&ctx, backup_command)?;
        }
        Commands::Tui => {
            commands::tui_cmd::execute(&ctx)?;
        }
    }

    Ok(())
}
