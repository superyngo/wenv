//! wenv - Shell Configuration File Manager

use anyhow::Result;
use clap::Parser;

use wenv::cli::actions;
use wenv::cli::args::{Cli, SubCmd};
use wenv::i18n;
use wenv::model;
use wenv::model::profile::ShellProfile;
use wenv::tui::TuiApp;
use wenv::utils::shell_detect::get_shell_type;

fn startup_file_check(
    profile: &mut ShellProfile,
    config: &mut wenv::Config,
    shell_key: &str,
) -> Result<()> {
    let mut paths_to_remove: Vec<std::path::PathBuf> = Vec::new();

    for file in &mut profile.files {
        if !file.exists {
            println!("⚠ File not found: {}", file.path.display());
            let create = dialoguer::Confirm::new()
                .with_prompt("  Create this file?")
                .default(true)
                .interact_opt()?
                .unwrap_or(false);

            if create {
                if let Some(parent) = file.path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("  ✗ Failed to create directory {}: {}", parent.display(), e);
                        continue;
                    }
                }
                match std::fs::File::create(&file.path) {
                    Ok(_) => {
                        file.exists = true;
                        file.content = String::new();
                        println!("  ✓ Created: {}", file.path.display());
                    }
                    Err(e) => {
                        eprintln!("  ✗ Failed to create: {}", e);
                        let remove = dialoguer::Confirm::new()
                            .with_prompt("  Remove this path from config?")
                            .default(false)
                            .interact_opt()?
                            .unwrap_or(false);
                        if remove {
                            paths_to_remove.push(file.path.clone());
                        }
                    }
                }
            }
        }
    }

    if !paths_to_remove.is_empty() {
        if let Some(files_config) = config.files.get_mut(shell_key) {
            files_config.paths.retain(|p| {
                let expanded = wenv::config::path_resolver::expand_env_vars(
                    &wenv::config::path_resolver::expand_tilde(p),
                );
                !paths_to_remove
                    .iter()
                    .any(|r| r == &std::path::PathBuf::from(&expanded))
            });
        }
        config.save()?;
        profile.files.retain(|f| !paths_to_remove.contains(&f.path));
    }

    for file in &mut profile.files {
        file.writable = if file.exists {
            wenv::utils::path::check_writable(&file.path)
        } else {
            false
        };
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine shell type (runtime decision, no config dependency)
    let shell_type = get_shell_type(cli.shell.map(|s| s.into()), None);

    // Load or create config
    let mut config = wenv::model::Config::resolve_or_create(shell_type.config_key())?;

    // Early exit: open resolved wenv config in $EDITOR
    if matches!(cli.subcommand, Some(SubCmd::Config)) {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });
        std::process::Command::new(&editor)
            .arg(&config.source_path)
            .status()?;
        return Ok(());
    }

    // Spec §4.6: warn if exe_dir-resolved config shadows a pre-existing
    // ~/.config/wenv/config.toml.
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        let exe_cfg = exe_dir.join("Resources").join("config.toml");
        if config.source_path == exe_cfg {
            if let Some(home) = dirs::home_dir() {
                let legacy = home.join(".config").join("wenv").join("config.toml");
                if legacy.exists() {
                    eprintln!(
                        "Note: {} exists but is shadowed by {}",
                        legacy.display(),
                        config.source_path.display()
                    );
                }
            }
        }
    }

    let shell_key = shell_type.config_key();

    // Ensure file list exists for this shell
    if !config.files.contains_key(shell_key) {
        wenv::config::ensure_shell_files(&mut config, shell_key)?;
    }
    wenv::config::ensure_shell_snippets(&mut config, shell_key)?;

    let messages = i18n::init_messages(&config.ui.language);

    // Source mode: file selection menu
    let is_source = cli.source || cli.command.as_deref() == Some(".");
    if is_source {
        return actions::source::execute(&config, shell_type, messages);
    }

    // Load shell profile and launch TUI
    let mut profile = model::profile::load_shell_profile(&config, shell_type)?;
    startup_file_check(&mut profile, &mut config, shell_key)?;
    TuiApp::new(profile, messages, config, shell_key.to_string())?.run()
}
