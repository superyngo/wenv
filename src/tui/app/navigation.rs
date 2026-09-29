//! Cursor, scroll, expand/collapse, selection, and remark/toggle actions.

use anyhow::Result;

use crate::model::profile::ListItem;
use crate::tui::app::{EditorRequest, TuiApp};
use crate::tui::list;
use crate::tui::state::AppMode;

impl TuiApp {
    pub(super) fn toggle_at_cursor(&mut self) {
        if self.visible_items.is_empty() {
            return;
        }
        let item = &self.visible_items[self.cursor];
        match item {
            ListItem::FileHeader(fi) => {
                let fi = *fi;
                self.profile.files[fi].expanded = !self.profile.files[fi].expanded;
                self.rebuild_list();
            }
            ListItem::DirHeader(ti) => {
                // Toggle group expand/collapse
                if let Some(crate::model::profile::TreeNode::Dir(ref mut g)) =
                    self.profile.tree.get_mut(*ti)
                {
                    // Toggle only the group; contained files keep their own
                    // expanded state (see ToggleExpand for rationale).
                    g.expanded = !g.expanded;
                    self.rebuild_list();
                }
            }
            ListItem::Entry(fi, _) => {
                // On Entry, toggle the parent file
                let fi = *fi;
                self.profile.files[fi].expanded = !self.profile.files[fi].expanded;
                self.rebuild_list();
            }
        }
    }

    /// Enforce: scroll_offset <= cursor < scroll_offset + list_visible_height
    pub fn clamp_scroll_offset(&mut self) {
        let h = self.list_visible_height;
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if h > 0 && self.cursor >= self.scroll_offset + h {
            self.scroll_offset = self.cursor - h + 1;
        }
    }

    /// Base directory a new file should be created in for a directory group.
    /// Prefers an existing member's parent; falls back to resolving the group's
    /// source pattern (directory as-is, or the parent of a glob pattern).
    pub(super) fn group_base_dir(&self, g: &crate::model::profile::DirGroup) -> std::path::PathBuf {
        if let Some(&fi) = g.file_indices.first() {
            if let Some(parent) = self.profile.files[fi].path.parent() {
                return parent.to_path_buf();
            }
        }
        let expanded = crate::config::path_resolver::expand_env_vars(
            &crate::config::path_resolver::expand_tilde(&g.source_pattern),
        );
        let p = std::path::PathBuf::from(&expanded);
        if p.is_dir() {
            p
        } else {
            p.parent()
                .map(|x| x.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        }
    }

    /// Returns the tree index of the `Dir` group containing file `fi`, if any.
    pub(super) fn group_index_of_file(&self, fi: usize) -> Option<usize> {
        self.profile.tree.iter().position(|n| {
            matches!(n, crate::model::profile::TreeNode::Dir(g) if g.file_indices.contains(&fi))
        })
    }

    pub(super) fn current_file_index(&self) -> usize {
        match self.visible_items.get(self.cursor) {
            Some(ListItem::FileHeader(fi)) => *fi,
            Some(ListItem::Entry(fi, _)) => *fi,
            Some(ListItem::DirHeader(_)) => 0, // DirHeader doesn't map to a single file; default to 0
            None => 0,
        }
    }

    /// Check if the file under the cursor is writable
    pub(super) fn is_current_file_writable(&self) -> bool {
        let fi = self.current_file_index();
        fi < self.profile.files.len() && self.profile.files[fi].writable
    }

    /// Get visible-list indices for operation targets.
    /// Returns selected indices if any, otherwise cursor if on an entry.
    pub(super) fn get_operation_targets(&self) -> Vec<usize> {
        if !self.selection.is_empty() {
            self.selection.sorted_indices()
        } else {
            // Use cursor position if it's on an entry
            match self.visible_items.get(self.cursor) {
                Some(ListItem::Entry(_, _)) => vec![self.cursor],
                _ => vec![],
            }
        }
    }

    pub(super) fn action_navigate_up(&mut self) {
        if self.mode == AppMode::MovingFile {
            if let Some(ref mut fms) = self.file_move_state {
                if fms.insertion_cursor > 0 {
                    fms.insertion_cursor -= 1;
                }
            }
        } else if self.mode == AppMode::Moving {
            self.move_cursor_up();
            self.sync_move_target_cursor();
        } else {
            self.selection.commit_range();
            self.cursor = list::navigate_up(&self.visible_items, self.cursor);
            self.clamp_scroll_offset();
        }
    }

    pub(super) fn action_navigate_down(&mut self) {
        if self.mode == AppMode::MovingFile {
            if let Some(ref mut fms) = self.file_move_state {
                let max_idx = self.visible_items.len().saturating_sub(1);
                if fms.insertion_cursor < max_idx {
                    fms.insertion_cursor += 1;
                }
            }
        } else if self.mode == AppMode::Moving {
            self.move_cursor_down();
            self.sync_move_target_cursor();
        } else {
            self.selection.commit_range();
            self.cursor = list::navigate_down(&self.visible_items, self.cursor);
            self.clamp_scroll_offset();
        }
    }

    pub(super) fn action_page_up(&mut self) {
        let half = (self.list_visible_height / 2).max(1);
        if self.mode == AppMode::MovingFile {
            if let Some(ref mut fms) = self.file_move_state {
                fms.insertion_cursor = fms.insertion_cursor.saturating_sub(half);
            }
        } else if self.mode == AppMode::Moving {
            if let Some(ref mut ms) = self.move_state {
                let target = ms.insertion_cursor.saturating_sub(half);
                ms.insertion_cursor = target;
            }
            self.snap_move_cursor_to_non_blocked();
            self.sync_move_target_cursor();
        } else {
            self.selection.commit_range();
            self.cursor = self.cursor.saturating_sub(half);
            self.clamp_scroll_offset();
        }
    }

    pub(super) fn action_page_down(&mut self) {
        let half = (self.list_visible_height / 2).max(1);
        let max_idx = self.visible_items.len().saturating_sub(1);
        if self.mode == AppMode::MovingFile {
            if let Some(ref mut fms) = self.file_move_state {
                fms.insertion_cursor = (fms.insertion_cursor + half).min(max_idx);
            }
        } else if self.mode == AppMode::Moving {
            if let Some(ref mut ms) = self.move_state {
                ms.insertion_cursor = (ms.insertion_cursor + half).min(max_idx);
            }
            self.snap_move_cursor_to_non_blocked();
            self.sync_move_target_cursor();
        } else {
            self.selection.commit_range();
            self.cursor = (self.cursor + half).min(max_idx);
            self.clamp_scroll_offset();
        }
    }

    pub(super) fn action_home(&mut self) {
        if self.mode == AppMode::Moving {
            if let Some(ref mut ms) = self.move_state {
                ms.insertion_cursor = 0;
            }
            self.snap_move_cursor_to_non_blocked();
            self.sync_move_target_cursor();
        } else {
            self.selection.commit_range();
            self.cursor = list::navigate_home();
            self.clamp_scroll_offset();
        }
    }

    pub(super) fn action_end(&mut self) {
        if self.mode == AppMode::Moving {
            let max_idx = self.visible_items.len().saturating_sub(1);
            if let Some(ref mut ms) = self.move_state {
                ms.insertion_cursor = max_idx;
            }
            self.snap_move_cursor_to_non_blocked();
            self.sync_move_target_cursor();
        } else {
            self.selection.commit_range();
            self.cursor = list::navigate_end(&self.visible_items);
            self.clamp_scroll_offset();
        }
    }

    pub(super) fn action_toggle_expand(&mut self) {
        if let Some(item) = self.visible_items.get(self.cursor) {
            match item {
                ListItem::FileHeader(_) => {
                    if !self.is_filter_mode() {
                        self.toggle_at_cursor();
                    }
                }
                ListItem::DirHeader(ti) => {
                    // Toggle group expand/collapse
                    if self.profile.tree.get(*ti).is_some() {
                        if let crate::model::profile::TreeNode::Dir(ref mut g) =
                            &mut self.profile.tree[*ti]
                        {
                            // Toggle only the group. Contained files keep
                            // their own expanded state, so opening a group
                            // reveals just its file headers (level 2), not
                            // every entry inside them (level 3).
                            g.expanded = !g.expanded;
                            self.rebuild_list();
                        }
                    }
                }
                ListItem::Entry(_, _) => {
                    if self.mode == AppMode::ShowingDetail {
                        // Toggle close: return to previous mode
                        self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                    } else {
                        // Toggle open: save current mode and show detail
                        self.previous_mode = Some(self.mode.clone());
                        self.mode = AppMode::ShowingDetail;
                        self.detail_scroll_offset = 0;
                    }
                }
            }
        }
    }

    pub(super) fn action_range_select_up(&mut self) {
        self.selection.set_range(self.cursor, &self.visible_items);
        self.cursor = list::navigate_up(&self.visible_items, self.cursor);
        self.clamp_scroll_offset();
        self.selection.set_range(self.cursor, &self.visible_items);
    }

    pub(super) fn action_range_select_down(&mut self) {
        self.selection.set_range(self.cursor, &self.visible_items);
        self.cursor = list::navigate_down(&self.visible_items, self.cursor);
        self.clamp_scroll_offset();
        self.selection.set_range(self.cursor, &self.visible_items);
    }

    pub(super) fn action_remark(&mut self) -> Result<()> {
        let in_detail = self.mode == AppMode::ShowingDetail;

        // In ShowingDetail: operate on the single entry at cursor
        // Otherwise: use normal operation targets
        let targets = if in_detail {
            match self.visible_items.get(self.cursor) {
                Some(ListItem::Entry(_, _)) => vec![self.cursor],
                _ => vec![],
            }
        } else {
            self.get_operation_targets()
        };
        if targets.is_empty() {
            return Ok(());
        }

        // Collect target entries (skip FileHeaders)
        let target_entries: Vec<(usize, usize)> = targets
            .iter()
            .filter_map(|&idx| match self.visible_items.get(idx) {
                Some(ListItem::Entry(fi, ei)) => Some((*fi, *ei)),
                _ => None,
            })
            .collect();

        if target_entries.is_empty() {
            return Ok(());
        }

        // Check all affected files are writable
        let affected_files: std::collections::HashSet<usize> =
            target_entries.iter().map(|(fi, _)| *fi).collect();

        if affected_files
            .iter()
            .any(|&fi| !self.profile.files[fi].writable)
        {
            self.message = Some("File is read-only".into());
            return Ok(());
        }

        // Remember whether there was a pre-existing selection (Feature 3 fix)
        let had_selection = !self.selection.is_empty();

        // Determine if all targets are Comments
        let all_comment = target_entries.iter().all(|(fi, ei)| {
            self.profile.files[*fi].entries[*ei].entry_type == crate::model::EntryType::Comment
        });

        // Take undo snapshot
        let snapshot = crate::tui::operations::take_snapshot(&self.profile);
        crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);

        // Track original range for selection restoration
        let first_visible = targets[0];
        let last_visible = *targets.last().unwrap();

        if all_comment {
            // UNCOMMENT: process in reverse order for stable indices
            let mut reversed = target_entries.clone();
            reversed.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

            let mut new_entry_pairs: Vec<(usize, usize)> = Vec::new();

            for (fi, ei) in reversed {
                let value = self.profile.files[fi].entries[ei].value.clone();
                let uncommented = crate::tui::operations::uncomment_value(&value);
                let parser = crate::parser::get_parser(self.profile.shell_type);
                let parsed = parser.parse(&uncommented);
                let count = parsed.entries.len();
                let new_entries: Vec<_> = parsed
                    .entries
                    .into_iter()
                    .map(|mut e| {
                        e.file_index = fi;
                        e
                    })
                    .collect();
                crate::tui::operations::replace_entry_with_parsed(
                    &mut self.profile.files[fi],
                    ei,
                    new_entries,
                    fi,
                );
                for j in 0..count {
                    new_entry_pairs.push((fi, ei + j));
                }
            }
            self.message = Some("Uncommented".into());

            self.rebuild_list();
            self.selection.clear();
            // Only restore selection if there was one before (Feature 3 fix)
            if had_selection {
                for (idx, item) in self.visible_items.iter().enumerate() {
                    if let ListItem::Entry(fi, ei) = item {
                        if new_entry_pairs.contains(&(*fi, *ei)) {
                            self.selection.toggle(idx, &self.visible_items);
                        }
                    }
                }
            }
        } else {
            // COMMENT: add "# " to non-Comment entries
            for (fi, ei) in &target_entries {
                if self.profile.files[*fi].entries[*ei].entry_type
                    != crate::model::EntryType::Comment
                {
                    let value = self.profile.files[*fi].entries[*ei].value.clone();
                    let commented = crate::tui::operations::comment_value(&value);
                    let entry = &mut self.profile.files[*fi].entries[*ei];
                    entry.value = commented;
                    entry.entry_type = crate::model::EntryType::Comment;
                    self.profile.files[*fi].dirty = true;
                }
            }
            for &fi in &affected_files {
                self.profile.files[fi].recalculate_line_numbers();
            }
            self.message = Some("Commented".into());

            self.rebuild_list();
            self.selection.clear();
            // Only restore selection if there was one before (Feature 3 fix)
            if had_selection {
                let new_end = last_visible.min(self.visible_items.len().saturating_sub(1));
                for idx in first_visible..=new_end {
                    if matches!(self.visible_items.get(idx), Some(ListItem::Entry(_, _))) {
                        self.selection.toggle(idx, &self.visible_items);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn action_edit(&mut self) -> Result<EditorRequest> {
        // In ShowingDetail: close popup first, then edit
        if self.mode == AppMode::ShowingDetail {
            if let Some(ListItem::Entry(fi, ei)) = self.visible_items.get(self.cursor) {
                let fi = *fi;
                let ei = *ei;
                if !self.profile.files[fi].writable {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }
                self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                return Ok(EditorRequest::EditEntry(fi, ei));
            }
            return Ok(EditorRequest::None);
        }
        if let Some(item) = self.visible_items.get(self.cursor) {
            match item {
                ListItem::FileHeader(fi) => return Ok(EditorRequest::EditFile(*fi)),
                ListItem::DirHeader(_) => return Ok(EditorRequest::None),
                ListItem::Entry(fi, ei) => {
                    let (fi, ei) = (*fi, *ei);
                    if !self.profile.files[fi].writable {
                        self.message = Some("File is read-only".into());
                        return Ok(EditorRequest::None);
                    }
                    // Single-line entries edit in-place; multi-line (merged
                    // comments / combined) fall back to the external editor.
                    if self.profile.files[fi].entries[ei].value.contains('\n') {
                        return Ok(EditorRequest::EditEntry(fi, ei));
                    }
                    self.begin_inline_edit(fi, ei);
                    return Ok(EditorRequest::None);
                }
            }
        }
        Ok(EditorRequest::None)
    }

    pub(super) fn action_edit_external(&mut self) -> Result<EditorRequest> {
        // Force the external editor regardless of single/multi-line.
        if let Some(item) = self.visible_items.get(self.cursor) {
            match item {
                ListItem::FileHeader(fi) => return Ok(EditorRequest::EditFile(*fi)),
                ListItem::DirHeader(_) => return Ok(EditorRequest::None),
                ListItem::Entry(fi, ei) => {
                    if !self.profile.files[*fi].writable {
                        self.message = Some("File is read-only".into());
                        return Ok(EditorRequest::None);
                    }
                    return Ok(EditorRequest::EditEntry(*fi, *ei));
                }
            }
        }
        Ok(EditorRequest::None)
    }
}
