use std::fs;
use tempfile::tempdir;
use wenv::model::Config;

#[test]
fn config_override_reads_existing_file() {
    let dir = tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(&cfg, "[ui]\nlanguage = \"zh-TW\"\n[files]\n").unwrap();

    let resolved = Config::resolve_or_create("zsh", Some(cfg.clone())).unwrap();

    assert_eq!(resolved.source_path, cfg);
    assert_eq!(resolved.ui.language, "zh-TW");
}

#[test]
fn config_override_creates_default_when_missing() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("nested").join("config.toml");
    assert!(!target.exists());

    let cfg = Config::resolve_or_create("zsh", Some(target.clone())).unwrap();

    assert!(
        target.exists(),
        "expected created config at {}",
        target.display()
    );
    assert_eq!(cfg.source_path, target);
    assert!(cfg.files.contains_key("zsh"));
}

#[test]
fn save_writes_to_source_path() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("config.toml");
    let mut cfg = Config::resolve_or_create("zsh", Some(target.clone())).unwrap();

    cfg.ui.language = "zh-TW".into();
    cfg.save().unwrap();

    let s = fs::read_to_string(&target).unwrap();
    assert!(s.contains("zh-TW"));
}

#[test]
fn cache_lives_next_to_config() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("config.toml");
    let cfg = Config::resolve_or_create("zsh", Some(target)).unwrap();

    use wenv::config::cache::Cache;
    let mut cache = Cache::load_or_default(&cfg);
    cache.pwsh_profile = Some("/tmp/profile.ps1".into());
    cache.save().unwrap();

    let expected = dir.path().join("cache.toml");
    assert!(expected.exists());
    let s = fs::read_to_string(&expected).unwrap();
    assert!(s.contains("profile.ps1"));
}

#[test]
fn snippets_resource_is_bundled_and_loadable() {
    // Debug builds resolve the in-repo Resources/snippets.toml.
    let snippets = wenv::model::Snippets::resolve().expect("bundled snippets.toml must be found");
    assert!(!snippets.for_shell("zsh").is_empty());
    assert!(!snippets.for_shell("bash").is_empty());
    assert!(!snippets.for_shell("powershell").is_empty());
}
