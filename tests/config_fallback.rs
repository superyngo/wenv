use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;
use wenv::model::Config;

// Serialize all tests that touch env vars so they don't race each other.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn wenv_config_dir_env_var_is_highest_priority() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(&cfg, "[ui]\nlanguage = \"en\"\n[files]\n[snippets]\n").unwrap();

    unsafe { std::env::set_var("WENV_CONFIG_DIR", dir.path()); }
    let resolved = Config::resolve_or_create("zsh").unwrap();
    unsafe { std::env::remove_var("WENV_CONFIG_DIR"); }

    assert_eq!(resolved.source_path, cfg);
}

#[test]
fn missing_all_fallbacks_creates_at_first_writable() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let target = dir.path().join("config.toml");
    assert!(!target.exists());

    unsafe { std::env::set_var("WENV_CONFIG_DIR", dir.path()); }
    let cfg = Config::resolve_or_create("zsh").unwrap();
    unsafe { std::env::remove_var("WENV_CONFIG_DIR"); }

    assert!(target.exists(), "expected created config at {}", target.display());
    assert_eq!(cfg.source_path, target);
    assert!(cfg.files.contains_key("zsh"));
}

#[test]
fn save_writes_to_source_path() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    unsafe { std::env::set_var("WENV_CONFIG_DIR", dir.path()); }
    let mut cfg = Config::resolve_or_create("zsh").unwrap();
    unsafe { std::env::remove_var("WENV_CONFIG_DIR"); }

    cfg.ui.language = "zh-TW".into();
    cfg.save().unwrap();

    let s = std::fs::read_to_string(&cfg.source_path).unwrap();
    assert!(s.contains("zh-TW"));
}

#[test]
fn cache_lives_next_to_config() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    unsafe { std::env::set_var("WENV_CONFIG_DIR", dir.path()); }
    let cfg = Config::resolve_or_create("zsh").unwrap();
    unsafe { std::env::remove_var("WENV_CONFIG_DIR"); }

    use wenv::config::cache::Cache;
    let mut cache = Cache::load_or_default(&cfg);
    cache.pwsh_profile = Some("/tmp/profile.ps1".into());
    cache.save().unwrap();

    let expected = dir.path().join("cache.toml");
    assert!(expected.exists());
    let s = std::fs::read_to_string(&expected).unwrap();
    assert!(s.contains("profile.ps1"));
}

#[cfg(unix)]
#[test]
fn save_copies_up_when_source_is_read_only() {
    use std::os::unix::fs::PermissionsExt;

    let _g = ENV_LOCK.lock().unwrap();
    let primary = tempdir().unwrap();
    let secondary = tempdir().unwrap();

    let primary_cfg = primary.path().join("config.toml");
    fs::write(&primary_cfg,
        "[ui]\nlanguage = \"en\"\n[files]\n[snippets]\n").unwrap();
    let mut perms = fs::metadata(&primary_cfg).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&primary_cfg, perms).unwrap();
    let mut perms_dir = fs::metadata(primary.path()).unwrap().permissions();
    perms_dir.set_mode(0o555);
    fs::set_permissions(primary.path(), perms_dir).unwrap();

    unsafe { std::env::set_var("WENV_CONFIG_DIR", primary.path()); }
    let saved_home = std::env::var("HOME").ok();
    unsafe { std::env::set_var("HOME", secondary.path()); }
    std::fs::create_dir_all(secondary.path().join(".wenget/apps/wenv/Resources")).unwrap();

    let mut cfg = Config::resolve_or_create("zsh").unwrap();
    assert_eq!(cfg.source_path, primary_cfg);

    cfg.ui.language = "zh-TW".into();
    cfg.save().unwrap();

    assert_ne!(cfg.source_path, primary_cfg, "source_path should have copied up");
    assert!(cfg.source_path.exists());

    // Restore env
    unsafe {
        if let Some(h) = saved_home { std::env::set_var("HOME", h); } else { std::env::remove_var("HOME"); }
        std::env::remove_var("WENV_CONFIG_DIR");
    }
    // Restore permissions so tempdir cleanup works
    let mut perms = fs::metadata(&primary_cfg).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&primary_cfg, perms).unwrap();
    let mut perms_dir = fs::metadata(primary.path()).unwrap().permissions();
    perms_dir.set_mode(0o755);
    fs::set_permissions(primary.path(), perms_dir).unwrap();
}
