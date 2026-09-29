use wenv::config::path_resolver;

#[test]
fn test_default_config_has_no_file_lists() {
    let config = wenv::Config::default();
    assert!(config.files.is_empty());
}

#[test]
fn test_config_with_bash_files() {
    let toml_str = r#"
[ui]
language = "en"

[files.bash]
paths = ["~/.bashrc", "~/.profile"]
"#;
    let config: wenv::Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.files.get("bash").unwrap().paths.len(), 2);
}

#[test]
fn test_config_roundtrip() {
    let mut config = wenv::Config::default();
    config.files.insert(
        "bash".to_string(),
        wenv::FilesConfig {
            paths: vec!["~/.bashrc".to_string(), "/etc/profile".to_string()],
        },
    );
    let serialized = toml::to_string_pretty(&config).unwrap();
    let deserialized: wenv::Config = toml::from_str(&serialized).unwrap();
    assert_eq!(deserialized.files.get("bash").unwrap().paths.len(), 2);
}

#[test]
fn test_expand_tilde() {
    let expanded = path_resolver::expand_tilde("~/test");
    assert!(!expanded.starts_with("~"));
    assert!(expanded.ends_with("/test"));
}

#[test]
fn test_expand_env_vars() {
    // Read-only: use a variable every environment already defines (no set_var races)
    let path = std::env::var("PATH").expect("PATH is set");
    let expanded = path_resolver::expand_env_vars("$PATH/config");
    assert_eq!(expanded, format!("{}/config", path));
}

#[test]
fn test_resolve_nonexistent_path() {
    use path_resolver::ResolvedPattern;
    let results = path_resolver::resolve_patterns(&["/nonexistent/path/file.txt".to_string()]);
    assert_eq!(results.len(), 1);
    match &results[0] {
        ResolvedPattern::File { exists, .. } => assert!(!exists),
        _ => panic!("expected File variant"),
    }
}

#[test]
fn test_expand_env_vars_unknown_var_unchanged() {
    // When an env var is not set and has no special handling, it stays as-is
    assert!(std::env::var_os("NONEXISTENT_VAR_XYZ").is_none());
    let result = path_resolver::expand_env_vars("$NONEXISTENT_VAR_XYZ/foo");
    // Should remain unchanged since var doesn't exist
    assert_eq!(result, "$NONEXISTENT_VAR_XYZ/foo");
}
