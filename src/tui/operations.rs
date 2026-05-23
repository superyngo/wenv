//! Entry manipulation operations

use std::collections::VecDeque;

use crate::model::profile::{ListItem, ProfileFile, ShellProfile};
use crate::model::Entry;
use crate::tui::state::UndoSnapshot;

pub const MAX_UNDO_HISTORY: usize = 20;

/// Push an undo snapshot onto the stack, clearing redo history.
/// If the stack exceeds MAX_UNDO_HISTORY, the oldest snapshot is discarded.
pub fn push_undo(
    undo_stack: &mut VecDeque<UndoSnapshot>,
    redo_stack: &mut Vec<UndoSnapshot>,
    snapshot: UndoSnapshot,
) {
    if undo_stack.len() >= MAX_UNDO_HISTORY {
        undo_stack.pop_front();
    }
    undo_stack.push_back(snapshot);
    redo_stack.clear();
}

/// Take an undo snapshot of all files (including dirty state)
pub fn take_snapshot(profile: &ShellProfile) -> UndoSnapshot {
    UndoSnapshot {
        file_states: profile
            .files
            .iter()
            .map(|f| {
                (
                    f.path.clone(),
                    f.content.clone(),
                    f.entries.clone(),
                    f.dirty,
                )
            })
            .collect(),
    }
}

/// Restore from an undo snapshot, fully replacing the file list.
/// Preserves UI-state fields (writable, expanded, exists) from matching files.
pub fn restore_snapshot(profile: &mut ShellProfile, snapshot: UndoSnapshot) {
    let old_files = std::mem::take(&mut profile.files);
    profile.files = snapshot
        .file_states
        .into_iter()
        .map(|(path, content, entries, dirty)| {
            // Carry over UI-state from matching old file if it exists
            let old = old_files.iter().find(|f| f.path == path);
            let mut file = ProfileFile::new(
                path.clone(),
                old.is_none_or(|f| f.exists),
                crate::config::path_resolver::tilde_collapse(&path.to_string_lossy()),
            );
            file.content = content;
            file.entries = entries;
            file.dirty = dirty;
            file.expanded = old.is_none_or(|f| f.expanded);
            file.writable = old.is_none_or(|f| f.writable);
            file
        })
        .collect();
}

/// Delete entries identified by visible-list indices.
/// Returns the deleted entries.
pub fn delete_entries(
    profile: &mut ShellProfile,
    items: &[ListItem],
    indices: &[usize],
) -> Vec<Entry> {
    // Collect (file_index, entry_index) pairs, skip FileHeaders
    let mut targets: Vec<(usize, usize)> = Vec::new();
    for &idx in indices {
        if let Some(ListItem::Entry(fi, ei)) = items.get(idx) {
            targets.push((*fi, *ei));
        }
    }

    // Group by file_index
    let mut by_file: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (fi, ei) in &targets {
        by_file.entry(*fi).or_default().push(*ei);
    }

    let mut deleted = Vec::new();
    let affected_files: Vec<usize> = by_file.keys().cloned().collect();

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

    for fi in affected_files {
        profile.files[fi].recalculate_line_numbers();
    }

    deleted
}

/// Cut entries: delete and return them in order.
pub fn cut_entries(
    profile: &mut ShellProfile,
    items: &[ListItem],
    indices: &[usize],
) -> Vec<Entry> {
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
        Some(ListItem::DirHeader(ti)) => {
            // Insert into first file of the dir group
            if let Some(crate::model::profile::TreeNode::Dir(g)) = profile.tree.get(*ti) {
                if let Some(&first_fi) = g.file_indices.first() {
                    (first_fi, 0)
                } else {
                    let fi = profile.files.len().saturating_sub(1);
                    (fi, profile.files[fi].entries.len())
                }
            } else {
                let fi = profile.files.len().saturating_sub(1);
                (fi, profile.files[fi].entries.len())
            }
        }
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
    profile.files[fi].recalculate_line_numbers();
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

        file.recalculate_line_numbers();

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

/// Replace a single entry with zero or more parsed entries at the same position.
/// Returns the number of new entries inserted.
/// If new_entries is empty, the original entry is deleted.
pub fn replace_entry_with_parsed(
    file: &mut ProfileFile,
    entry_index: usize,
    new_entries: Vec<Entry>,
    file_index: usize,
) -> usize {
    file.entries.remove(entry_index);

    let count = new_entries.len();
    for (i, mut entry) in new_entries.into_iter().enumerate() {
        entry.file_index = file_index;
        file.entries.insert(entry_index + i, entry);
    }

    file.dirty = true;
    file.recalculate_line_numbers();
    count
}

/// Add "# " to all non-blank lines (including already-commented lines).
pub fn comment_value(value: &str) -> String {
    value
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else {
                format!("# {}", line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove leading "# " or "#" from non-blank lines.
pub fn uncomment_value(value: &str) -> String {
    value
        .split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else if let Some(stripped) = line.strip_prefix("# ") {
                stripped.to_string()
            } else if let Some(stripped) = line.strip_prefix('#') {
                stripped.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Given a resolved file path, find the raw config pattern that matches it
/// and return all files that pattern resolves to.
pub fn find_matching_config_pattern(
    config: &crate::model::Config,
    shell_key: &str,
    resolved_path: &std::path::Path,
) -> Option<(String, Vec<std::path::PathBuf>)> {
    let files_config = config.files.get(shell_key)?;
    for raw_pattern in &files_config.paths {
        let resolved =
            crate::config::path_resolver::resolve_patterns(std::slice::from_ref(raw_pattern));
        let all_paths: Vec<std::path::PathBuf> = resolved
            .iter()
            .flat_map(|rp| match rp {
                crate::config::path_resolver::ResolvedPattern::File { path, .. } => {
                    vec![path.clone()]
                }
                crate::config::path_resolver::ResolvedPattern::Dir { files, .. } => {
                    files.iter().map(|(p, _)| p.clone()).collect()
                }
            })
            .collect();
        if all_paths.iter().any(|p| p == resolved_path) {
            return Some((raw_pattern.clone(), all_paths));
        }
    }
    None
}
