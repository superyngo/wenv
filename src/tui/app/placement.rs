//! Placement operations (copy/cut/drop) and file move reordering.

use crate::model::profile::ListItem;
use crate::tui::app::TuiApp;
use crate::tui::state::{AppMode, FileMovingState, MoveState};

impl TuiApp {
    /// Drop the placement (c/x): clone sources and insert at the target. For a cut
    /// (`ms.cut`), also remove the sources, so the net effect is a move; for a copy,
    /// the sources are left in place. Single drop → return to the resting mode.
    pub(super) fn execute_drop(&mut self) {
        if let Some(ms) = self.move_state.take() {
            // Snapshot for undo (placement itself made no changes until now).
            let snapshot = crate::tui::operations::take_snapshot(&self.profile);
            crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
            // Determine target file and position from insertion_cursor
            let (target_fi, target_pos) = match self.visible_items.get(ms.insertion_cursor) {
                Some(ListItem::Entry(fi, ei)) => (*fi, ei + 1), // Insert after this entry
                Some(ListItem::FileHeader(fi)) => (*fi, 0),     // Insert at start of file
                Some(ListItem::DirHeader(ti)) => {
                    if let Some(crate::model::profile::TreeNode::Dir(g)) =
                        self.profile.tree.get(*ti)
                    {
                        if let Some(&first_fi) = g.file_indices.first() {
                            (first_fi, 0)
                        } else {
                            let fi = self.profile.files.len().saturating_sub(1);
                            (fi, self.profile.files[fi].entries.len())
                        }
                    } else {
                        let fi = self.profile.files.len().saturating_sub(1);
                        (fi, self.profile.files[fi].entries.len())
                    }
                }
                None => {
                    let fi = self.profile.files.len().saturating_sub(1);
                    (fi, self.profile.files[fi].entries.len())
                }
            };

            // Clone the source entries (for copy they stay; for cut they're removed below).
            let mut entries_to_move: Vec<crate::model::Entry> = Vec::new();
            for &(fi, ei) in &ms.source_items {
                if fi < self.profile.files.len() && ei < self.profile.files[fi].entries.len() {
                    entries_to_move.push(self.profile.files[fi].entries[ei].clone());
                }
            }

            // Group sources by file (indices descending) for removal / accounting.
            let mut by_file: std::collections::HashMap<usize, Vec<usize>> =
                std::collections::HashMap::new();
            for &(fi, ei) in &ms.source_items {
                by_file.entry(fi).or_default().push(ei);
            }
            let source_files: Vec<usize> = by_file.keys().cloned().collect();

            // Cut removes the sources (reverse order preserves indices); copy keeps them.
            if ms.cut {
                for (&fi, indices) in &mut by_file {
                    indices.sort();
                    indices.dedup();
                    for &ei in indices.iter().rev() {
                        if ei < self.profile.files[fi].entries.len() {
                            self.profile.files[fi].entries.remove(ei);
                        }
                    }
                    self.profile.files[fi].dirty = true;
                }
            }

            // Adjust target_pos only when a cut removed sources before it in the same file.
            let removed_before_target = if ms.cut {
                by_file
                    .get(&target_fi)
                    .map(|indices| indices.iter().filter(|&&ei| ei < target_pos).count())
                    .unwrap_or(0)
            } else {
                0
            };
            let adjusted_pos = (target_pos - removed_before_target)
                .min(self.profile.files[target_fi].entries.len());

            // Insert at target
            let inserted_count = entries_to_move.len();
            for (i, mut entry) in entries_to_move.into_iter().enumerate() {
                entry.file_index = target_fi;
                self.profile.files[target_fi]
                    .entries
                    .insert(adjusted_pos + i, entry);
            }
            self.profile.files[target_fi].dirty = true;

            // Recalculate line numbers for affected files
            let mut affected_files: std::collections::HashSet<usize> =
                source_files.into_iter().collect();
            affected_files.insert(target_fi);
            for fi in affected_files {
                if fi < self.profile.files.len() {
                    self.profile.files[fi].recalculate_line_numbers();
                }
            }

            self.selection.clear();
            let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
            let in_filter = matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive);
            self.mode = if in_filter {
                AppMode::FilterActive
            } else {
                AppMode::Normal
            };
            if in_filter {
                self.update_filter();
            } else {
                self.rebuild_list();
            }
            // Land on the last inserted entry by identity: rows removed above the target
            // by a cut shift visible indices, so `insertion_cursor` can point elsewhere.
            let last_inserted = adjusted_pos + inserted_count.saturating_sub(1);
            self.cursor = self
                .visible_items
                .iter()
                .position(|it| {
                    matches!(it, ListItem::Entry(fi, ei)
                    if *fi == target_fi && *ei == last_inserted)
                })
                .unwrap_or_else(|| {
                    ms.insertion_cursor
                        .min(self.visible_items.len().saturating_sub(1))
                });
            self.clamp_scroll_offset();
            self.message = Some(if ms.cut { "Moved" } else { "Copied" }.into());
        }
    }

    /// Begin a copy/cut placement: record the selected/cursor entries as sources
    /// (rendered blue), snap the target to a writable position, and enter Moving
    /// mode. Nothing is mutated until the drop (`execute_drop`).
    pub(super) fn begin_placement(&mut self, cut: bool) {
        // Cut removes from the source file, so it must be writable. Copy doesn't.
        if cut && !self.is_current_file_writable() {
            self.message = Some("File is read-only".into());
            return;
        }
        let targets = self.get_operation_targets();
        let source_items: Vec<(usize, usize)> = targets
            .iter()
            .filter_map(|&idx| match self.visible_items.get(idx) {
                Some(ListItem::Entry(fi, ei)) => Some((*fi, *ei)),
                _ => None,
            })
            .collect();
        if source_items.is_empty() {
            self.message = Some("Select an entry to copy/move".into());
            return;
        }
        let has_selection = !self.selection.is_empty();
        // Start the target at the cursor, snapped to a writable (non-blocked) row.
        let mut target = self.cursor;
        if Self::is_position_blocked(&self.visible_items, &self.profile.files, target) {
            let max_idx = self.visible_items.len().saturating_sub(1);
            while target < max_idx
                && Self::is_position_blocked(&self.visible_items, &self.profile.files, target)
            {
                target += 1;
            }
        }
        self.cursor = target;
        self.clamp_scroll_offset();
        self.move_state = Some(MoveState {
            source_items,
            insertion_cursor: target,
            from_selection: has_selection,
            cut,
        });
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::Moving;
        self.message = Some(if cut {
            "Move: ↑↓ position · v/Enter drop · Esc cancel".into()
        } else {
            "Copy: ↑↓ position · v/Enter drop · Esc cancel".into()
        });
    }

    /// Switch the active placement between copy (`cut=false`) and cut (`cut=true`)
    /// without leaving Moving mode. Switching to cut requires all source files to be
    /// writable; otherwise the placement stays copy. The status bar reflects the mode.
    pub(super) fn set_placement_cut(&mut self, cut: bool) {
        let blocked = cut
            && self.move_state.as_ref().is_some_and(|ms| {
                ms.source_items
                    .iter()
                    .any(|&(fi, _)| !self.profile.files[fi].writable)
            });
        if blocked {
            return; // keep copy; the unchanged "Copy:" hint signals it can't cut
        }
        if let Some(ms) = &mut self.move_state {
            ms.cut = cut;
        }
    }

    /// Execute file move: reorder file in config and profile
    pub(super) fn execute_file_move(&mut self) {
        if let Some(fms) = self.file_move_state.take() {
            let target_fi = match self.visible_items.get(fms.insertion_cursor) {
                Some(ListItem::FileHeader(fi)) => *fi,
                _ => fms.original_fi,
            };

            if target_fi == fms.original_fi {
                // No change — just restore expanded states
                self.restore_expanded(&fms.saved_expanded);
                let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                let in_filter = matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive);
                self.mode = if in_filter {
                    AppMode::FilterActive
                } else {
                    AppMode::Normal
                };
                if in_filter {
                    self.update_filter();
                } else {
                    self.rebuild_list();
                }
                self.message = Some("No change".into());
                return;
            }

            // Map both files to their config patterns; the config order is the only
            // source of truth (dirs/globs expand to several files).
            let shell_key = self.shell_key.clone();
            let src_path = self.profile.files[fms.original_fi].path.clone();
            let tgt_path = self.profile.files[target_fi].path.clone();
            let src_pat = crate::tui::operations::find_matching_config_pattern(
                &self.config,
                &shell_key,
                &src_path,
            );
            let tgt_pat = crate::tui::operations::find_matching_config_pattern(
                &self.config,
                &shell_key,
                &tgt_path,
            );
            let abort_msg = match (&src_pat, &tgt_pat) {
                (Some((sp, _)), Some((tp, _))) if sp == tp => {
                    Some("Files inside a group follow the group's order")
                }
                (Some(_), Some(_)) => None,
                _ => Some("File is not in the config; cannot reorder"),
            };
            if let Some(msg) = abort_msg {
                self.restore_expanded(&fms.saved_expanded);
                self.exit_file_move_mode();
                self.message = Some(msg.into());
                return;
            }
            let (sp, tp) = (src_pat.unwrap().0, tgt_pat.unwrap().0);
            if let Some(files_config) = self.config.files.get_mut(&shell_key) {
                let paths = &mut files_config.paths;
                if let (Some(from), Some(to)) = (
                    paths.iter().position(|p| *p == sp),
                    paths.iter().position(|p| *p == tp),
                ) {
                    let item = paths.remove(from);
                    let tp_now = paths.iter().position(|p| *p == tp).unwrap_or(to);
                    // Moving down lands after the target pattern, moving up before it
                    paths.insert(if from < to { tp_now + 1 } else { tp_now }, item);
                }
            }
            if let Err(e) = self.config.save() {
                self.message = Some(format!("Failed to save config: {}", e));
            }
            // Expanded states were saved by index; carry them across the reload by identity
            let file_exp: Vec<(std::path::PathBuf, bool)> = self
                .profile
                .files
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    (
                        f.path.clone(),
                        fms.saved_expanded
                            .files
                            .get(i)
                            .copied()
                            .unwrap_or(f.expanded),
                    )
                })
                .collect();
            let dir_patterns: Vec<String> = self
                .profile
                .tree
                .iter()
                .filter_map(|n| match n {
                    crate::model::profile::TreeNode::Dir(g) => Some(g.source_pattern.clone()),
                    _ => None,
                })
                .collect();
            self.exit_file_move_mode();
            if let Err(e) = self.reload_profile() {
                self.message = Some(format!("Reload failed: {}", e));
                return;
            }
            for f in &mut self.profile.files {
                if let Some((_, e)) = file_exp.iter().find(|(p, _)| *p == f.path) {
                    f.expanded = *e;
                }
            }
            for n in &mut self.profile.tree {
                if let crate::model::profile::TreeNode::Dir(g) = n {
                    if let Some(pos) = dir_patterns.iter().position(|p| *p == g.source_pattern) {
                        if let Some(&v) = fms.saved_expanded.dirs.get(pos) {
                            g.expanded = v;
                        }
                    }
                }
            }
            self.rebuild_list();

            // Place cursor on the moved file's new header
            let cursor_pos = self
                .visible_items
                .iter()
                .position(|item| {
                    matches!(item, ListItem::FileHeader(fi)
                        if self.profile.files[*fi].path == src_path)
                })
                .unwrap_or(0);
            self.cursor = cursor_pos;
            self.clamp_scroll_offset();
            self.message = Some("File moved".into());
        }
    }

    /// Leave file-move mode, returning to the filter or normal mode.
    pub(super) fn exit_file_move_mode(&mut self) {
        let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
        let in_filter = matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive);
        self.mode = if in_filter {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
        if in_filter {
            self.update_filter();
        } else {
            self.rebuild_list();
        }
    }

    /// Cancel file move: restore expanded states
    pub(super) fn cancel_file_move(&mut self) {
        if let Some(fms) = self.file_move_state.take() {
            self.restore_expanded(&fms.saved_expanded);
            let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
            let in_filter = matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive);
            self.mode = if in_filter {
                AppMode::FilterActive
            } else {
                AppMode::Normal
            };
            if in_filter {
                self.update_filter();
            } else {
                self.rebuild_list();
            }
            // Place cursor on original file's header
            let cursor_pos = self
                .visible_items
                .iter()
                .position(|item| matches!(item, ListItem::FileHeader(fi) if *fi == fms.original_fi))
                .unwrap_or(0);
            self.cursor = cursor_pos;
            self.clamp_scroll_offset();
            self.message = Some("File move cancelled".into());
        }
    }

    /// Restore per-file and per-dir expanded states from a snapshot
    pub(super) fn restore_expanded(&mut self, snap: &crate::tui::state::ExpandedSnapshot) {
        for (i, file) in self.profile.files.iter_mut().enumerate() {
            if let Some(&expanded) = snap.files.get(i) {
                file.expanded = expanded;
            }
        }
        let mut dir_i = 0;
        for n in &mut self.profile.tree {
            if let crate::model::profile::TreeNode::Dir(g) = n {
                if let Some(&v) = snap.dirs.get(dir_i) {
                    g.expanded = v;
                }
                dir_i += 1;
            }
        }
    }

    /// Check if a visible-list position belongs to a blocked file
    pub(super) fn is_position_blocked(
        items: &[ListItem],
        files: &[crate::model::profile::ProfileFile],
        pos: usize,
    ) -> bool {
        let fi = match items.get(pos) {
            Some(ListItem::FileHeader(fi)) | Some(ListItem::Entry(fi, _)) => *fi,
            Some(ListItem::DirHeader(_)) => return false,
            None => return true,
        };
        fi < files.len() && (!files[fi].exists || !files[fi].writable)
    }

    /// Keep `self.cursor` on the placement target so the list scrolls to follow the
    /// green box as it moves.
    pub(super) fn sync_move_target_cursor(&mut self) {
        if let Some(ic) = self.move_state.as_ref().map(|ms| ms.insertion_cursor) {
            self.cursor = ic;
            self.clamp_scroll_offset();
        }
    }

    /// Move cursor up in Moving mode, skipping blocked files
    pub(super) fn move_cursor_up(&mut self) {
        if let Some(ms) = &mut self.move_state {
            let old = ms.insertion_cursor;
            if old == 0 {
                return;
            }
            let mut pos = old - 1;
            loop {
                if !Self::is_position_blocked(&self.visible_items, &self.profile.files, pos) {
                    ms.insertion_cursor = pos;
                    return;
                }
                if pos == 0 {
                    break;
                }
                pos -= 1;
            }
            self.message = Some("No writable file to move to".into());
        }
    }

    /// Move cursor down in Moving mode, skipping blocked files
    pub(super) fn move_cursor_down(&mut self) {
        if let Some(ms) = &mut self.move_state {
            let old = ms.insertion_cursor;
            let max_idx = self.visible_items.len().saturating_sub(1);
            if old >= max_idx {
                return;
            }
            let mut pos = old + 1;
            loop {
                if !Self::is_position_blocked(&self.visible_items, &self.profile.files, pos) {
                    ms.insertion_cursor = pos;
                    return;
                }
                if pos >= max_idx {
                    break;
                }
                pos += 1;
            }
            self.message = Some("No writable file to move to".into());
        }
    }

    /// After a page-jump in Moving mode, snap to nearest non-blocked position
    pub(super) fn snap_move_cursor_to_non_blocked(&mut self) {
        if let Some(ms) = &mut self.move_state {
            let pos = ms.insertion_cursor;
            if !Self::is_position_blocked(&self.visible_items, &self.profile.files, pos) {
                return;
            }
            let max_idx = self.visible_items.len().saturating_sub(1);
            let mut fwd = pos + 1;
            while fwd <= max_idx {
                if !Self::is_position_blocked(&self.visible_items, &self.profile.files, fwd) {
                    ms.insertion_cursor = fwd;
                    return;
                }
                fwd += 1;
            }
            let mut bwd = pos.saturating_sub(1);
            loop {
                if !Self::is_position_blocked(&self.visible_items, &self.profile.files, bwd) {
                    ms.insertion_cursor = bwd;
                    return;
                }
                if bwd == 0 {
                    break;
                }
                bwd -= 1;
            }
            self.message = Some("No writable file to move to".into());
        }
    }

    pub(super) fn action_start_move(&mut self) {
        // `m` now reorders files only; entries use c/x placement.
        // DirHeader: move not supported on groups
        if matches!(
            self.visible_items.get(self.cursor),
            Some(ListItem::DirHeader(_))
        ) {
            self.message = Some("Move is not supported on groups".into());
            return;
        }
        // Check if cursor is on a FileHeader → enter file move mode
        if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
            let fi = *fi;
            let is_inside_group = self.profile.tree.iter().any(|n| {
                matches!(n, crate::model::profile::TreeNode::Dir(g) if g.file_indices.contains(&fi))
            });
            if is_inside_group {
                self.message = Some(
                    "Files inside a group are sorted alphabetically; move is not supported".into(),
                );
                return;
            }
            if self.profile.files.len() < 2 {
                self.message = Some("Only one file, nothing to move".into());
                return;
            }
            let saved_expanded_files: Vec<bool> =
                self.profile.files.iter().map(|f| f.expanded).collect();
            let saved_expanded_dirs: Vec<bool> = self
                .profile
                .tree
                .iter()
                .map(|n| match n {
                    crate::model::profile::TreeNode::Dir(g) => g.expanded,
                    _ => false,
                })
                .collect();
            let saved_expanded = crate::tui::state::ExpandedSnapshot {
                files: saved_expanded_files,
                dirs: saved_expanded_dirs,
            };
            self.profile.toggle_all(false);
            self.rebuild_list();
            // Find cursor for the file header after collapse
            let cursor_pos = self
                .visible_items
                .iter()
                .position(|item| matches!(item, ListItem::FileHeader(i) if *i == fi))
                .unwrap_or(0);
            self.cursor = cursor_pos;
            self.clamp_scroll_offset();
            self.file_move_state = Some(FileMovingState {
                original_fi: fi,
                insertion_cursor: cursor_pos,
                saved_expanded,
            });
            self.previous_mode = Some(self.mode.clone());
            self.mode = AppMode::MovingFile;
            self.message = Some("File move: ↑↓ to position, Enter to drop, Esc to cancel".into());
            return;
        }

        // On an entry: `m` no longer moves entries — point at c/x.
        self.message = Some("Use c to copy or x to move entries".into());
    }
}
