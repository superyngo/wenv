//! Built-in file path templates for each shell type

use crate::{Config, FilesConfig};

pub fn default_paths(shell_key: &str) -> Option<Vec<String>> {
    match shell_key {
        "bash" => Some(vec![
            "/etc/profile".into(),
            "/etc/profile.d/*.sh".into(),
            "~/.profile".into(),
            "~/.bashrc".into(),
            "~/.bash_aliases".into(),
        ]),
        "zsh" => Some(vec![
            "/etc/zshenv".into(),
            "/etc/zprofile".into(),
            "/etc/zshrc".into(),
            "~/.zshenv".into(),
            "~/.zprofile".into(),
            "~/.zshrc".into(),
            "~/.zsh_aliases".into(),
        ]),
        "powershell" => Some(vec![
            "$PROFILE".into(),
        ]),
        _ => None,
    }
}

pub fn generate_default_config(shell_key: &str) -> String {
    let mut config = Config::default();
    if let Some(paths) = default_paths(shell_key) {
        config.files.insert(
            shell_key.to_string(),
            FilesConfig { paths },
        );
    }
    toml::to_string_pretty(&config).unwrap()
}