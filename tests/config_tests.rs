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
    std::env::set_var("WENV_TEST_VAR", "/tmp/test");
    let expanded = path_resolver::expand_env_vars("$WENV_TEST_VAR/config");
    assert_eq!(expanded, "/tmp/test/config");
    std::env::remove_var("WENV_TEST_VAR");
}

#[test]
fn test_resolve_nonexistent_path() {
    let results = path_resolver::resolve_paths(&["/nonexistent/path/file.txt".to_string()]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].1);
}
