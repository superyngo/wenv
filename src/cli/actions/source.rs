//! Open source file in editor

use anyhow::Result;
use colored::Colorize;
use std::env;
use std::process::Command;

use crate::cli::context::Context;

/// Execute the source action (open config file in editor)
pub fn execute(ctx: &Context) -> Result<()> {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    println!(
        "Opening {} in {}...",
        ctx.config_file.display().to_string().cyan(),
        editor.yellow()
    );

    let status = Command::new(&editor).arg(&ctx.config_file).status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    ctx.print_success("Config file edited successfully");
    ctx.print_reload_hint();

    Ok(())
}
