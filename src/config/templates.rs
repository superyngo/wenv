//! Built-in file path templates for each shell type

use crate::model::Snippet;
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
        "powershell" => Some(vec!["$PROFILE".into()]),
        _ => None,
    }
}

pub fn generate_default_config(shell_key: &str) -> String {
    let mut config = Config::default();
    if let Some(paths) = default_paths(shell_key) {
        config
            .files
            .insert(shell_key.to_string(), FilesConfig { paths });
    }
    toml::to_string_pretty(&config).unwrap()
}

/// Default snippets for POSIX shells (bash/zsh)
fn default_posix_snippets() -> Vec<Snippet> {
    vec![
        Snippet {
            name: "blank".into(),
            description: "Empty editor".into(),
            template: None,
        },
        Snippet {
            name: "alias".into(),
            description: "alias NAME='VALUE'".into(),
            template: Some("alias NAME='VALUE'".into()),
        },
        Snippet {
            name: "export".into(),
            description: "export VAR='VALUE'".into(),
            template: Some("export VAR='VALUE'".into()),
        },
        Snippet {
            name: "function".into(),
            description: "name() { ... }".into(),
            template: Some("name() {\n  \n}".into()),
        },
        Snippet {
            name: "source".into(),
            description: "source FILE".into(),
            template: Some("source ".into()),
        },
    ]
}

/// Default snippets for PowerShell
fn default_pwsh_snippets() -> Vec<Snippet> {
    vec![
        Snippet {
            name: "blank".into(),
            description: "Empty editor".into(),
            template: None,
        },
        Snippet {
            name: "alias".into(),
            description: "Set-Alias NAME VALUE".into(),
            template: Some("Set-Alias Name Value".into()),
        },
        Snippet {
            name: "function".into(),
            description: "function Name { ... }".into(),
            template: Some("function Name {\n  \n}".into()),
        },
        Snippet {
            name: "env-var".into(),
            description: "$env:VAR = 'VALUE'".into(),
            template: Some("$env:VAR = 'VALUE'".into()),
        },
        Snippet {
            name: "source".into(),
            description: ". FILE".into(),
            template: Some(". ".into()),
        },
    ]
}

/// Get default snippets for the given shell key
pub fn default_snippets(shell_key: &str) -> Vec<Snippet> {
    match shell_key {
        "bash" | "zsh" => default_posix_snippets(),
        "powershell" => default_pwsh_snippets(),
        _ => vec![],
    }
}
