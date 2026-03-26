//! PowerShell configuration file formatter

#[cfg(test)]
use crate::model::EntryType;
use crate::model::{Config, Entry, ShellType};

use super::Formatter;

/// PowerShell configuration file formatter
pub struct PowerShellFormatter;

impl PowerShellFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PowerShellFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for PowerShellFormatter {
    fn format(&self, entries: &[Entry], _config: &Config) -> String {
        entries
            .iter()
            .map(|e| self.format_entry(e))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_entry(&self, entry: &Entry) -> String {
        // With the new architecture, value already contains complete raw syntax
        entry.value.clone()
    }

    fn shell_type(&self) -> ShellType {
        ShellType::PowerShell
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_alias() {
        let formatter = PowerShellFormatter::new();
        let entry = Entry::new(
            EntryType::Alias,
            "ll".into(),
            "Set-Alias ll 'Get-ChildItem'".into(),
        );
        assert_eq!(
            formatter.format_entry(&entry),
            "Set-Alias ll 'Get-ChildItem'"
        );
    }

    #[test]
    fn test_format_env() {
        let formatter = PowerShellFormatter::new();
        let entry = Entry::new(
            EntryType::EnvVar,
            "EDITOR".into(),
            "$env:EDITOR = \"code\"".into(),
        );
        assert_eq!(formatter.format_entry(&entry), "$env:EDITOR = \"code\"");
    }

    #[test]
    fn test_format_env_multiline() {
        let formatter = PowerShellFormatter::new();
        // Entry with complete syntax (Raw Value Architecture)
        let value = r#"$env:PATH = @"
C:\Program Files\bin
D:\tools
E:\bin
"@"#;
        let entry = Entry::new(EntryType::EnvVar, "PATH".into(), value.into());
        // Formatter returns value directly
        assert_eq!(formatter.format_entry(&entry), value);
    }

    #[test]
    fn test_format_source() {
        let formatter = PowerShellFormatter::new();
        // Entry with complete syntax (Raw Value Architecture)
        let entry = Entry::new(EntryType::Source, "L10".into(), ". .\\aliases.ps1".into());
        assert_eq!(formatter.format_entry(&entry), ". .\\aliases.ps1");
    }

    #[test]
    fn test_format_source_with_name() {
        let formatter = PowerShellFormatter::new();
        // Entry with complete syntax (Raw Value Architecture)
        let entry = Entry::new(
            EntryType::Source,
            "aliases".into(),
            ". .\\aliases.ps1".into(),
        );
        assert_eq!(formatter.format_entry(&entry), ". .\\aliases.ps1");
    }
}
