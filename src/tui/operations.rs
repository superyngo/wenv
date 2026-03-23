//! Entry manipulation operations

use crate::model::profile::{ShellProfile, ListItem};
use crate::model::Entry;
use crate::tui::state::UndoSnapshot;

/// Take an undo snapshot of all files
pub fn take_snapshot(profile: &ShellProfile) -> UndoSnapshot {
    UndoSnapshot {
        file_states: profile.files.iter().map(|f| {
            (f.path.clone(), f.content.clone(), f.entries.clone())
        }).collect(),
    }
}

/// Restore from an undo snapshot
pub fn restore_snapshot(profile: &mut ShellProfile, snapshot: UndoSnapshot) {
    for (i, (path, content, entries)) in snapshot.file_states.into_iter().enumerate() {
        if i < profile.files.len() {
            profile.files[i].path = path;
            profile.files[i].content = content;
            profile.files[i].entries = entries;
            profile.files[i].dirty = false;
        }
    }
}

/// Delete entries identified by visible-list indices.
/// Returns the deleted entries.
pub fn delete_entries(profile: &mut ShellProfile, items: &[ListItem], indices: &[usize]) -> Vec<Entry> {
    // Collect (file_index, entry_index) pairs, skip FileHeaders
    let mut targets: Vec<(usize, usize)> = Vec::new();
    for &idx in indices {
        if let Some(ListItem::Entry(fi, ei)) = items.get(idx) {
            targets.push((*fi, *ei));
        }
    }

    // Group by file_index
    let mut by_file: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for (fi, ei) in &targets {
        by_file.entry(*fi).or_default().push(*ei);
    }

    let mut deleted = Vec::new();

    // Process each file, removing entries in reverse index order
    for (fi, mut entry_indices) in by_file {
        entry_indices.sort();
        entry_indices.dedup();
        // Remove in reverse order to preserve indices
        for &ei in entry_indices.iter().rev() {
            if ei < profile.files[fi].entries.len() {
                deleted.push(profile.files[fi].entries.remove(ei));
            }
        }
        profile.files[fi].dirty = true;
    }

    deleted
}

/// Cut entries: delete and return them in order.
pub fn cut_entries(profile: &mut ShellProfile, items: &[ListItem], indices: &[usize]) -> Vec<Entry> {
    delete_entries(profile, items, indices)
}

/// Paste entries at cursor position.
pub fn paste_entries(profile: &mut ShellProfile, items: &[ListItem], at: usize, entries: &[Entry]) {
    if entries.is_empty() {
        return;
    }

    // Determine target file and insert position
    let (fi, insert_pos) = match items.get(at) {
        Some(ListItem::Entry(fi, ei)) => (*fi, ei + 1),
        Some(ListItem::FileHeader(fi)) => (*fi, 0),
        None => {
            // Past end — insert at end of last file
            let fi = profile.files.len().saturating_sub(1);
            (fi, profile.files[fi].entries.len())
        }
    };

    // Insert entries, updating file_index
    for (i, entry) in entries.iter().enumerate() {
        let mut e = entry.clone();
        e.file_index = fi;
        profile.files[fi].entries.insert(insert_pos + i, e);
    }
    profile.files[fi].dirty = true;
}

/// Save all dirty files. Reconstructs content from entry values.
/// Entry.value is in separator format (N lines = N-1 \n).
/// File content needs terminator format (each entry value terminated by \n).
pub fn save_dirty_files(profile: &mut ShellProfile) -> anyhow::Result<Vec<String>> {
    let mut saved = Vec::new();
    for file in &mut profile.files {
        if !file.dirty {
            continue;
        }

        // Reconstruct file content from entries
        let mut content = String::new();
        for entry in &file.entries {
            content.push_str(&entry.value);
            content.push('\n');
        }

        std::fs::write(&file.path, &content)?;
        file.content = content;
        file.dirty = false;
        saved.push(file.path.display().to_string());
    }
    Ok(saved)
}