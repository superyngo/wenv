use wenv::model::EntryType;
use wenv::parser::{Parser, PowerShellParser};

#[test]
fn test_powershell_heredoc_integration() {
    let content = r#"# Test file
$env:EDITOR = "code"

$env:PATH = @"
C:\Program Files\bin
D:\tools
"@

$env:SHELL = "pwsh"

Set-Alias ll Get-ChildItem
"#;

    let parser = PowerShellParser::new();
    let result = parser.parse(content);

    // Check env vars
    let envs: Vec<_> = result
        .entries
        .iter()
        .filter(|e| e.entry_type == EntryType::EnvVar)
        .collect();

    assert_eq!(envs.len(), 3, "Should parse 3 environment variables");

    // Verify single-line env var
    let editor = envs.iter().find(|e| e.name == "EDITOR").unwrap();
    assert_eq!(editor.value, "code");
    assert!(
        editor.end_line.is_none(),
        "Single-line should not have end_line"
    );

    // Verify multi-line env var
    let path = envs.iter().find(|e| e.name == "PATH").unwrap();
    assert_eq!(path.value, "C:\\Program Files\\bin\nD:\\tools");
    assert!(path.end_line.is_some(), "Multi-line should have end_line");

    // Verify alias still works
    let aliases: Vec<_> = result
        .entries
        .iter()
        .filter(|e| e.entry_type == EntryType::Alias)
        .collect();

    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "ll");
}

#[test]
fn test_powershell_heredoc_formatter_integration() {
    use wenv::formatter::{Formatter, PowerShellFormatter};
    use wenv::model::Entry;

    let formatter = PowerShellFormatter::new();

    // Single-line env var
    let entry1 = Entry::new(EntryType::EnvVar, "EDITOR".into(), "code".into());
    assert_eq!(formatter.format_entry(&entry1), "$env:EDITOR = \"code\"");

    // Multi-line env var
    let entry2 = Entry::new(
        EntryType::EnvVar,
        "PATH".into(),
        "C:\\bin\nD:\\tools".into(),
    );
    let formatted = formatter.format_entry(&entry2);
    assert!(formatted.starts_with("$env:PATH = @\""));
    assert!(formatted.contains("C:\\bin"));
    assert!(formatted.contains("D:\\tools"));
    assert!(formatted.ends_with("\"@"));
}
