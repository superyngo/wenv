use std::fs;
use tempfile::tempdir;
use wenv::model::profile::{load_shell_profile, TreeNode};
use wenv::model::{Config, FilesConfig, ShellType};

fn make_cfg(td: &tempfile::TempDir, paths: Vec<String>) -> Config {
    let mut c = Config {
        source_path: td.path().join("config.toml"),
        ..Config::default()
    };
    c.files.insert("zsh".into(), FilesConfig { paths });
    c
}

#[test]
fn loading_and_reloading_keeps_indices_stable() {
    let td = tempdir().unwrap();
    fs::write(td.path().join("a.sh"), "alias a=1\n").unwrap();
    fs::write(td.path().join("b.sh"), "alias b=2\n").unwrap();
    let cfg = make_cfg(
        &td,
        vec![
            td.path().join("a.sh").to_string_lossy().to_string(),
            td.path().join("b.sh").to_string_lossy().to_string(),
        ],
    );
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    assert_eq!(prof.files.len(), 2);
    // After removing the second pattern, only a.sh remains
    let cfg2 = make_cfg(
        &td,
        vec![td.path().join("a.sh").to_string_lossy().to_string()],
    );
    let prof2 = load_shell_profile(&cfg2, ShellType::Zsh).unwrap();
    assert_eq!(prof2.files.len(), 1);
    assert_eq!(prof2.tree.len(), 1);
    assert!(matches!(prof2.tree[0], TreeNode::File(0)));
}
