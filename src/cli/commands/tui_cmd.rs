//! TUI command implementation

use anyhow::Result;

use super::CommandContext;

/// Execute the TUI command
pub fn execute(ctx: &CommandContext) -> Result<()> {
    let mut app = crate::tui::TuiApp::new(ctx.config_file.clone(), ctx.shell_type, ctx.messages)?;
    app.run()?;
    Ok(())
}
