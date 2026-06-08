use std::fs;
use tempfile::tempdir;
use wenv::model::profile::{load_shell_profile, ListItem, TreeNode};
use wenv::model::{Config, FilesConfig, ShellType};

fn cfg_with(shell_key: &str, paths: Vec<String>) -> Config {
    let mut c = Config::default();
    c.files.insert(shell_key.to_string(), FilesConfig { paths });
    c.source_path = std::path::PathBuf::from("/tmp/test-source-path.toml");
    c
}

#[test]
fn glob_pattern_produces_group_with_sorted_files() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("b.sh"), "echo b\n").unwrap();
    fs::write(sub.join("a.sh"), "echo a\n").unwrap();

    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    assert_eq!(prof.tree.len(), 1);
    match &prof.tree[0] {
        TreeNode::Dir(g) => {
            assert_eq!(g.file_indices.len(), 2);
            assert!(!g.expanded);
            assert!(prof.files[g.file_indices[0]].path.ends_with("a.sh"));
            assert!(prof.files[g.file_indices[1]].path.ends_with("b.sh"));
        }
        _ => panic!("expected Dir"),
    }
    // Default-collapsed: visible list contains only the dir header.
    let visible = prof.build_visible_list();
    assert_eq!(visible.len(), 1);
    assert!(matches!(visible[0], ListItem::DirHeader(0)));
}

#[test]
fn top_level_file_and_group_interleave_in_config_order() {
    let dir = tempdir().unwrap();
    let zrc = dir.path().join(".zshrc");
    fs::write(&zrc, "alias x=1\n").unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("k.sh"), "echo\n").unwrap();

    let cfg = cfg_with(
        "zsh",
        vec![
            zrc.to_string_lossy().to_string(),
            format!("{}/*", sub.display()),
        ],
    );
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    assert_eq!(prof.tree.len(), 2);
    assert!(matches!(prof.tree[0], TreeNode::File(_)));
    assert!(matches!(prof.tree[1], TreeNode::Dir(_)));
}

#[test]
fn expanded_group_with_collapsed_file_shows_only_file_header() {
    // Regression: opening a group must reveal its file headers (level 2) only,
    // not auto-expand each file's entries (level 3).
    let dir = tempdir().unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("k.sh"), "alias a=1\nalias b=2\n").unwrap();
    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let mut prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();

    // Expand the group only; the contained file stays collapsed (load default).
    if let TreeNode::Dir(g) = &mut prof.tree[0] {
        g.expanded = true;
    } else {
        panic!("expected Dir");
    }
    assert!(!prof.files[0].expanded, "file should remain collapsed");

    let visible = prof.build_visible_list();
    assert_eq!(visible.len(), 2, "only dir header + file header expected");
    assert!(matches!(visible[0], ListItem::DirHeader(0)));
    assert!(matches!(visible[1], ListItem::FileHeader(0)));
    assert!(
        !visible.iter().any(|it| matches!(it, ListItem::Entry(_, _))),
        "no entries should be visible while the file is collapsed"
    );
}

#[test]
fn toggle_all_flips_dir_and_file() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("k.sh"), "x\n").unwrap();
    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let mut prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();

    prof.toggle_all(true);
    assert!(matches!(&prof.tree[0], TreeNode::Dir(g) if g.expanded));
    assert!(prof.files[0].expanded);

    prof.toggle_all(false);
    assert!(matches!(&prof.tree[0], TreeNode::Dir(g) if !g.expanded));
    assert!(!prof.files[0].expanded);
}

#[test]
fn filtered_list_hides_nonmatching_entries_in_matched_file() {
    // Regression: a file inside a group is "matched" because one entry matches,
    // but the other entries in the same file must still be hidden by the filter.
    use std::collections::HashSet;

    let dir = tempdir().unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(
        sub.join("k.sh"),
        "alias keep='echo'\nalias drop='echo'\nalias keepalso='echo'\n",
    )
    .unwrap();
    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let mut prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();

    // Filter expands the group and the matched file.
    if let TreeNode::Dir(g) = &mut prof.tree[0] {
        g.expanded = true;
    } else {
        panic!("expected Dir");
    }
    prof.files[0].expanded = true;

    // Simulate a "keep" query: entries 0 and 2 match, entry 1 does not.
    let matched_files: HashSet<usize> = HashSet::from([0]);
    let matched_entries: HashSet<(usize, usize)> = HashSet::from([(0, 0), (0, 2)]);

    let visible = prof.build_visible_list_filtered(&matched_files, &matched_entries);

    let entries: Vec<_> = visible
        .iter()
        .filter_map(|it| match it {
            ListItem::Entry(fi, ei) => Some((*fi, *ei)),
            _ => None,
        })
        .collect();
    assert_eq!(
        entries,
        vec![(0, 0), (0, 2)],
        "non-matching entry (0,1) must be hidden"
    );
}

#[test]
fn binary_file_filtered_from_group() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("ok.sh"), "echo\n").unwrap();
    fs::write(sub.join("bin.dat"), [0u8, 1, 2]).unwrap();
    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    match &prof.tree[0] {
        TreeNode::Dir(g) => assert_eq!(g.file_indices.len(), 1),
        _ => panic!(),
    }
}
