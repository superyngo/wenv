use std::path::PathBuf;
use wenv::model::profile::{ListItem, ProfileFile, ShellProfile};
use wenv::model::{Entry, EntryType, ShellType};

#[test]
fn test_build_visible_list_collapsed() {
    let profile = ShellProfile::from_files(ShellType::Bash, vec![ProfileFile::new_with_entries(
        PathBuf::from("/etc/profile"),
        vec![Entry::new(
            EntryType::Alias,
            "ll".into(),
            "alias ll='ls -la'".into(),
        )],
        false, // collapsed
    )]);
    let list = profile.build_visible_list();
    assert_eq!(list.len(), 1); // only file header, entries hidden
    assert!(matches!(list[0], ListItem::FileHeader(0)));
}

#[test]
fn test_build_visible_list_expanded() {
    let profile = ShellProfile::from_files(ShellType::Bash, vec![ProfileFile::new_with_entries(
        PathBuf::from("~/.bashrc"),
        vec![
            Entry::new(EntryType::Alias, "ll".into(), "alias ll='ls -la'".into()),
            Entry::new(
                EntryType::Function,
                "greet".into(),
                "greet() { echo hi; }".into(),
            ),
        ],
        true, // expanded
    )]);
    let list = profile.build_visible_list();
    assert_eq!(list.len(), 3); // header + 2 entries
    assert!(matches!(list[0], ListItem::FileHeader(0)));
    assert!(matches!(list[1], ListItem::Entry(0, 0)));
    assert!(matches!(list[2], ListItem::Entry(0, 1)));
}

#[test]
fn test_build_visible_list_multiple_files() {
    let profile = ShellProfile::from_files(ShellType::Bash, vec![
        ProfileFile::new_with_entries(
            PathBuf::from("/etc/profile"),
            vec![Entry::new(
                EntryType::EnvVar,
                "PATH".into(),
                "export PATH=/usr/bin".into(),
            )],
            true,
        ),
        ProfileFile::new_with_entries(
            PathBuf::from("~/.bashrc"),
            vec![Entry::new(
                EntryType::Alias,
                "ll".into(),
                "alias ll='ls -la'".into(),
            )],
            false, // collapsed
        ),
    ]);
    let list = profile.build_visible_list();
    // File 0 header + 1 entry + File 1 header (collapsed, no entries)
    assert_eq!(list.len(), 3);
}

#[test]
fn test_toggle_all() {
    let mut profile = ShellProfile::from_files(ShellType::Bash, vec![
        ProfileFile::new_with_entries(PathBuf::from("/etc/profile"), vec![], true),
        ProfileFile::new_with_entries(PathBuf::from("~/.bashrc"), vec![], false),
    ]);
    profile.toggle_all(false);
    assert!(!profile.files[0].expanded);
    assert!(!profile.files[1].expanded);
    profile.toggle_all(true);
    assert!(profile.files[0].expanded);
    assert!(profile.files[1].expanded);
}

#[test]
fn test_any_dirty() {
    let mut profile = ShellProfile::from_files(ShellType::Bash, vec![ProfileFile::new_with_entries(
        PathBuf::from("~/.bashrc"),
        vec![],
        true,
    )]);
    assert!(!profile.any_dirty());
    profile.files[0].dirty = true;
    assert!(profile.any_dirty());
}
