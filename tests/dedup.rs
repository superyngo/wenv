use std::fs;
use tempfile::tempdir;
use wenv::model::profile::{load_shell_profile, TreeNode};
use wenv::model::{Config, FilesConfig, ShellType};

#[test]
fn glob_and_literal_overlap_dedups_keeping_first() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("d");
    fs::create_dir_all(&sub).unwrap();
    let a = sub.join("a.sh");
    fs::write(&a, "echo a\n").unwrap();

    let mut cfg = Config {
        source_path: std::path::PathBuf::from("/tmp/x.toml"),
        ..Config::default()
    };
    cfg.files.insert(
        "zsh".into(),
        FilesConfig {
            paths: vec![
                format!("{}/*", sub.display()),  // captures a.sh
                a.to_string_lossy().to_string(), // duplicate — should be dropped
            ],
        },
    );

    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    // First node: Dir(a.sh). Second node: File but the path was already
    // consumed by the Dir → the File node has no backing ProfileFile.
    // Implementation: duplicate File entirely dropped from tree.
    assert_eq!(prof.tree.len(), 1, "second node should have been dropped");
    match &prof.tree[0] {
        TreeNode::Dir(g) => assert_eq!(g.file_indices.len(), 1),
        _ => panic!(),
    }
}
