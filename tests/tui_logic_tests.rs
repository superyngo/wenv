//! TUI operations logic tests (without rendering)

use std::path::PathBuf;
use wenv::model::profile::{ListItem, ProfileFile, ShellProfile};
use wenv::model::{Entry, EntryType, ShellType};
use wenv::tui::operations;
use wenv::tui::selection::SelectionState;

fn make_test_entry(name: &str, value: &str, entry_type: EntryType, fi: usize) -> Entry {
    Entry {
        entry_type,
        name: name.to_string(),
        value: value.to_string(),
        line_number: None,
        end_line: None,
        file_index: fi,
    }
}

fn make_test_profile() -> ShellProfile {
    let file1 = ProfileFile::new_with_entries(
        "/tmp/test1.sh".into(),
        vec![
            make_test_entry("ll", "alias ll='ls -la'", EntryType::Alias, 0),
            make_test_entry("gs", "alias gs='git status'", EntryType::Alias, 0),
            make_test_entry(
                "PATH",
                "export PATH=\"/usr/bin:$PATH\"",
                EntryType::EnvVar,
                0,
            ),
        ],
        true,
    );
    let file2 = ProfileFile::new_with_entries(
        "/tmp/test2.sh".into(),
        vec![
            make_test_entry("greet", "greet() { echo hello; }", EntryType::Function, 1),
            make_test_entry("EDITOR", "export EDITOR=vim", EntryType::EnvVar, 1),
        ],
        true,
    );
    ShellProfile {
        shell_type: ShellType::Bash,
        files: vec![file1, file2],
    }
}

#[test]
fn test_build_visible_list() {
    let profile = make_test_profile();
    let items = profile.build_visible_list();
    // 2 file headers + 3 entries + 2 entries = 7
    assert_eq!(items.len(), 7);
    assert!(matches!(items[0], ListItem::FileHeader(0)));
    assert!(matches!(items[1], ListItem::Entry(0, 0)));
    assert!(matches!(items[2], ListItem::Entry(0, 1)));
    assert!(matches!(items[3], ListItem::Entry(0, 2)));
    assert!(matches!(items[4], ListItem::FileHeader(1)));
    assert!(matches!(items[5], ListItem::Entry(1, 0)));
    assert!(matches!(items[6], ListItem::Entry(1, 1)));
}

#[test]
fn test_build_visible_list_collapsed() {
    let mut profile = make_test_profile();
    profile.files[0].expanded = false;
    let items = profile.build_visible_list();
    // 1 collapsed header + 1 expanded header + 2 entries = 4
    assert_eq!(items.len(), 4);
    assert!(matches!(items[0], ListItem::FileHeader(0)));
    assert!(matches!(items[1], ListItem::FileHeader(1)));
    assert!(matches!(items[2], ListItem::Entry(1, 0)));
    assert!(matches!(items[3], ListItem::Entry(1, 1)));
}

#[test]
fn test_delete_entries() {
    let mut profile = make_test_profile();
    let items = profile.build_visible_list();
    // Delete entry at visible index 1 (file 0, entry 0 = "ll")
    let deleted = operations::delete_entries(&mut profile, &items, &[1]);
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].name, "ll");
    assert_eq!(profile.files[0].entries.len(), 2);
    assert!(profile.files[0].dirty);
}

#[test]
fn test_delete_multiple_entries_across_files() {
    let mut profile = make_test_profile();
    let items = profile.build_visible_list();
    // Delete indices 2 (file0/entry1="gs") and 5 (file1/entry0="greet")
    let deleted = operations::delete_entries(&mut profile, &items, &[2, 5]);
    assert_eq!(deleted.len(), 2);
    assert_eq!(profile.files[0].entries.len(), 2); // was 3, removed 1
    assert_eq!(profile.files[1].entries.len(), 1); // was 2, removed 1
    assert!(profile.files[0].dirty);
    assert!(profile.files[1].dirty);
}

#[test]
fn test_cut_and_paste() {
    let mut profile = make_test_profile();
    let items = profile.build_visible_list();
    // items[0] = FileHeader(0), items[1] = Entry(0,0), items[2] = Entry(0,1), items[3] = Entry(0,2)
    // items[4] = FileHeader(1), items[5] = Entry(1,0), items[6] = Entry(1,1)

    // Cut entry at index 1 (file0/entry0 = "ll")
    let cut = operations::cut_entries(&mut profile, &items, &[1]);
    assert_eq!(cut.len(), 1);
    assert_eq!(cut[0].name, "ll");
    assert_eq!(profile.files[0].entries.len(), 2); // file0 now has 2 entries

    // After cutting, rebuild list:
    // items[0] = FileHeader(0), items[1] = Entry(0,0), items[2] = Entry(0,1)
    // items[3] = FileHeader(1), items[4] = Entry(1,0), items[5] = Entry(1,1)
    let items = profile.build_visible_list();

    // Now index 3 is FileHeader(1), paste there to insert at position 0 in file1
    operations::paste_entries(&mut profile, &items, 3, &cut);
    assert_eq!(profile.files[1].entries.len(), 3);
    assert_eq!(profile.files[1].entries[0].name, "ll"); // pasted at start
    assert_eq!(profile.files[1].entries[0].file_index, 1); // file_index updated
    assert!(profile.files[1].dirty);
}

#[test]
fn test_paste_at_entry_position() {
    let mut profile = make_test_profile();
    let items = profile.build_visible_list();
    // Cut entry at index 1 (file0/entry0 = "ll")
    let cut = operations::cut_entries(&mut profile, &items, &[1]);

    // After cutting, rebuild list and paste at first entry in file1
    let items = profile.build_visible_list();
    // items[4] should be Entry(1,0)="greet", paste there to insert after it
    operations::paste_entries(&mut profile, &items, 4, &cut);
    assert_eq!(profile.files[1].entries.len(), 3);
    assert_eq!(profile.files[1].entries[0].name, "greet"); // original first entry
    assert_eq!(profile.files[1].entries[1].name, "ll"); // pasted after first entry
    assert_eq!(profile.files[1].entries[1].file_index, 1); // file_index updated
}

#[test]
fn test_snapshot_and_restore() {
    let mut profile = make_test_profile();
    let snapshot = operations::take_snapshot(&profile);

    // Modify profile
    profile.files[0].entries.remove(0);
    profile.files[0].dirty = true;
    assert_eq!(profile.files[0].entries.len(), 2);

    // Restore
    operations::restore_snapshot(&mut profile, snapshot);
    assert_eq!(profile.files[0].entries.len(), 3);
    assert!(!profile.files[0].dirty);
}

#[test]
fn test_selection_toggle() {
    let profile = make_test_profile();
    let items = profile.build_visible_list();
    let mut sel = SelectionState::new();

    // Toggle on entry (index 1)
    sel.toggle(1, &items);
    assert!(sel.is_selected(1));
    assert_eq!(sel.selected_count(), 1);

    // Toggle off
    sel.toggle(1, &items);
    assert!(!sel.is_selected(1));
    assert_eq!(sel.selected_count(), 0);

    // Can't select FileHeader (index 0)
    sel.toggle(0, &items);
    assert!(!sel.is_selected(0));
}

#[test]
fn test_selection_range() {
    let profile = make_test_profile();
    let items = profile.build_visible_list();
    let mut sel = SelectionState::new();

    // Range select from 1 to 3 (entries in file 0)
    // set_range is called multiple times: first sets anchor, second extends to cursor
    sel.set_range(1, &items); // Set anchor at 1
    sel.set_range(3, &items); // Extend to 3
    assert!(sel.is_selected(1));
    assert!(sel.is_selected(2));
    assert!(sel.is_selected(3));
    assert!(!sel.is_selected(0)); // FileHeader not selected
    assert_eq!(sel.selected_count(), 3);
}

#[test]
fn test_any_dirty() {
    let mut profile = make_test_profile();
    assert!(!profile.any_dirty());
    profile.files[0].dirty = true;
    assert!(profile.any_dirty());
}

#[test]
fn test_total_entries() {
    let profile = make_test_profile();
    assert_eq!(profile.total_entries(), 5);
}

#[test]
fn test_recalculate_line_numbers_single_line_entries() {
    let mut profile = make_test_profile();
    // File 0 has 3 single-line entries
    profile.files[0].recalculate_line_numbers();

    assert_eq!(profile.files[0].entries[0].line_number, Some(1));
    assert_eq!(profile.files[0].entries[0].end_line, Some(1));
    assert_eq!(profile.files[0].entries[1].line_number, Some(2));
    assert_eq!(profile.files[0].entries[1].end_line, Some(2));
    assert_eq!(profile.files[0].entries[2].line_number, Some(3));
    assert_eq!(profile.files[0].entries[2].end_line, Some(3));
}

#[test]
fn test_recalculate_line_numbers_multiline_entries() {
    let mut file = ProfileFile::new(PathBuf::from("/test"), true);
    file.entries = vec![
        make_test_entry("foo", "foo() {\n  echo hi\n}", EntryType::Function, 0),
        make_test_entry("bar", "alias bar='baz'", EntryType::Alias, 0),
    ];
    file.recalculate_line_numbers();

    assert_eq!(file.entries[0].line_number, Some(1));
    assert_eq!(file.entries[0].end_line, Some(3)); // 3 lines: foo() {\n  echo hi\n}
    assert_eq!(file.entries[1].line_number, Some(4));
    assert_eq!(file.entries[1].end_line, Some(4));
}

#[test]
fn test_recalculate_updates_code_comment_names() {
    let mut file = ProfileFile::new(PathBuf::from("/test"), true);
    file.entries = vec![
        make_test_entry("alias1", "alias a='b'", EntryType::Alias, 0),
        {
            let mut e = make_test_entry("L99", "echo hello", EntryType::Code, 0);
            e.line_number = Some(99); // stale
            e
        },
        {
            let mut e = make_test_entry("#L99-L100", "# comment\n# more", EntryType::Comment, 0);
            e.line_number = Some(99); // stale
            e
        },
    ];
    file.recalculate_line_numbers();

    assert_eq!(file.entries[1].name, "L2");
    assert_eq!(file.entries[1].line_number, Some(2));
    assert_eq!(file.entries[2].name, "#L3-L4");
    assert_eq!(file.entries[2].line_number, Some(3));
    assert_eq!(file.entries[2].end_line, Some(4));
}

#[test]
fn test_replace_entry_with_parsed_multiple() {
    use wenv::tui::operations::replace_entry_with_parsed;

    let mut file = ProfileFile::new(PathBuf::from("/tmp/test"), true);
    file.entries = vec![
        make_test_entry("a", "alias a='1'", EntryType::Alias, 0),
        make_test_entry("b", "alias b='2'", EntryType::Alias, 0),
        make_test_entry("c", "alias c='3'", EntryType::Alias, 0),
    ];

    let replacements = vec![
        make_test_entry("x", "alias x='10'", EntryType::Alias, 0),
        make_test_entry("y", "alias y='20'", EntryType::Alias, 0),
    ];

    let count = replace_entry_with_parsed(&mut file, 1, replacements, 0);

    assert_eq!(count, 2);
    assert_eq!(file.entries.len(), 4); // was 3, removed 1, added 2
    assert_eq!(file.entries[0].name, "a");
    assert_eq!(file.entries[1].name, "x");
    assert_eq!(file.entries[2].name, "y");
    assert_eq!(file.entries[3].name, "c");
    assert!(file.dirty);
}

#[test]
fn test_replace_entry_with_empty_deletes() {
    use wenv::tui::operations::replace_entry_with_parsed;

    let mut file = ProfileFile::new(PathBuf::from("/tmp/test"), true);
    file.entries = vec![
        make_test_entry("a", "alias a='1'", EntryType::Alias, 0),
        make_test_entry("b", "alias b='2'", EntryType::Alias, 0),
    ];

    let count = replace_entry_with_parsed(&mut file, 0, vec![], 0);

    assert_eq!(count, 0);
    assert_eq!(file.entries.len(), 1);
    assert_eq!(file.entries[0].name, "b");
    assert!(file.dirty);
}

#[test]
fn test_copy_entries() {
    let profile = make_test_profile();
    let items = profile.build_visible_list();

    // Find an entry index (first Entry in visible_items)
    let entry_idx = items.iter().position(|item| matches!(item, ListItem::Entry(_, _))).unwrap();

    // Copy the entry
    let copied: Vec<Entry> = vec![entry_idx]
        .iter()
        .filter_map(|&idx| match items.get(idx) {
            Some(ListItem::Entry(fi, ei)) => Some(profile.files[*fi].entries[*ei].clone()),
            _ => None,
        })
        .collect();

    assert_eq!(copied.len(), 1);
    // Original entries should be unchanged (not deleted like cut)
    let original_count: usize = profile.files.iter().map(|f| f.entries.len()).sum();
    assert!(original_count > 0);
    // No file should be dirty after copy
    assert!(!profile.files.iter().any(|f| f.dirty));
}
