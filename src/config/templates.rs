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

pub fn default_snippets(shell_key: &str) -> Option<Vec<Snippet>> {
    match shell_key {
        "bash" => Some(vec![
            Snippet { name: "Empty".into(), description: "Blank entry".into(), template: None },
            Snippet { name: "source".into(), description: "Source a file".into(),
                template: Some("# Source a shell file\nsource PATH".into()) },
            Snippet { name: "export".into(), description: "Set environment variable".into(),
                template: Some("# Set variable name and value\nexport NAME='VALUE'".into()) },
            Snippet { name: "alias".into(), description: "Define an alias".into(),
                template: Some("# Set alias name and value\nalias NAME='VALUE'".into()) },
            Snippet { name: "function".into(), description: "Define a function".into(),
                template: Some("# Define function name and body\nNAME() {\n    # body\n}".into()) },
        ]),
        "zsh" => Some(vec![
            Snippet { name: "Empty".into(), description: "Blank entry".into(), template: None },
            Snippet { name: "source".into(), description: "Source a file".into(),
                template: Some("# Source a shell file\nsource PATH".into()) },
            Snippet { name: "export".into(), description: "Set environment variable".into(),
                template: Some("# Set variable name and value\nexport NAME='VALUE'".into()) },
            Snippet { name: "alias".into(), description: "Define an alias".into(),
                template: Some("# Set alias name and value\nalias NAME='VALUE'".into()) },
            Snippet { name: "function".into(), description: "Define a function".into(),
                template: Some("# Define function name and body\nNAME() {\n    # body\n}".into()) },
            Snippet { name: "bindkey".into(), description: "Bind a key".into(),
                template: Some("# Bind key to widget\nbindkey KEY WIDGET".into()) },
        ]),
        "powershell" => Some(vec![
            Snippet { name: "Empty".into(), description: "Blank entry".into(), template: None },
            Snippet { name: "source".into(), description: "Source a file".into(),
                template: Some("# Source a PowerShell file\n. PATH".into()) },
            Snippet { name: "env".into(), description: "Set environment variable".into(),
                template: Some("# Set environment variable\n$env:NAME = \"VALUE\"".into()) },
            Snippet { name: "alias".into(), description: "Define an alias".into(),
                template: Some("# Set alias name and command\nSet-Alias -Name NAME -Value COMMAND".into()) },
            Snippet { name: "function".into(), description: "Define a function".into(),
                template: Some("# Define function name and body\nfunction NAME {\n    # body\n}".into()) },
            Snippet { name: "enum".into(), description: "Define an enum".into(),
                template: Some("# Define enum type\nenum NAME {\n    VALUE1\n    VALUE2\n}".into()) },
            Snippet { name: "class".into(), description: "Define a class".into(),
                template: Some("# Define class\nclass NAME {\n    # properties and methods\n}".into()) },
            Snippet { name: "scriptblock".into(), description: "Script block".into(),
                template: Some("# Script block\n{\n    # code\n}".into()) },
        ]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_snippets_bash() {
        let snippets = default_snippets("bash").unwrap();
        assert!(snippets.len() >= 5);
        assert_eq!(snippets[0].name, "Empty");
        assert!(snippets[0].template.is_none());
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"source"));
        assert!(names.contains(&"alias"));
        assert!(names.contains(&"export"));
        assert!(names.contains(&"function"));
        assert!(!names.contains(&"bindkey"));
    }

    #[test]
    fn test_default_snippets_zsh() {
        let snippets = default_snippets("zsh").unwrap();
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"bindkey"));
        assert!(names.contains(&"source"));
        assert!(names.contains(&"alias"));
    }

    #[test]
    fn test_default_snippets_pwsh() {
        let snippets = default_snippets("powershell").unwrap();
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Empty"));
        assert!(names.contains(&"source"));
        assert!(names.contains(&"env"));
        assert!(names.contains(&"alias"));
        assert!(names.contains(&"function"));
        assert!(names.contains(&"enum"));
        assert!(names.contains(&"class"));
        assert!(names.contains(&"scriptblock"));
    }

    #[test]
    fn test_default_snippets_unknown_shell() {
        assert!(default_snippets("fish").is_none());
    }
}
