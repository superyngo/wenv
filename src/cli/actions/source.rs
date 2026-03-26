//! Source action: open shell config file in editor

use anyhow::Result;
use dialoguer::Select;

use crate::config::path_resolver;
use crate::i18n::Messages;
use crate::model::{Config, ShellType};

pub fn execute(config: &Config, shell_type: ShellType, _messages: &'static Messages) -> Result<()> {
    let shell_key = shell_type.config_key();
    let file_configs = config
        .files
        .get(shell_key)
        .ok_or_else(|| anyhow::anyhow!("No file list configured for shell: {}", shell_key))?;

    let resolved = path_resolver::resolve_paths(&file_configs.paths);
    if resolved.is_empty() {
        println!("No files configured for {}", shell_key);
        return Ok(());
    }

    let items: Vec<String> = resolved
        .iter()
        .map(|(path, exists)| {
            let display = path.display();
            if *exists {
                format!("{}", display)
            } else {
                format!("{} (not found)", display)
            }
        })
        .collect();

    let selection = Select::new()
        .with_prompt(format!("Select file to edit ({})", shell_key))
        .items(&items)
        .default(0)
        .interact()?;

    let (path, exists) = &resolved[selection];
    if !exists {
        println!("File does not exist: {}", path.display());
        return Ok(());
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) {
            "notepad".into()
        } else {
            "vi".into()
        }
    });
    std::process::Command::new(&editor).arg(path).status()?;
    Ok(())
}
