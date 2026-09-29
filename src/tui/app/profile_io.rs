//! Profile reload, file save, pattern registration, and undo/redo handling.

use anyhow::Result;

use std::collections::{HashMap, HashSet};

use crate::model::profile::{ListItem, ProfileFile, TreeNode};
use crate::tui::app::TuiApp;

impl TuiApp {
    /// Append a config pattern (plain file, glob, directory, or var-bearing
    /// form) and load the file(s) it resolves to. Mirrors `load_shell_profile`
    /// so the `a` key supports every format the config file accepts.
    pub(super) fn add_pattern_to_config_and_profile(
        &mut self,
        raw_path: String,
    ) -> anyhow::Result<()> {
        let shell_key = self.shell_key.clone();
        let files_config = self
            .config
            .files
            .entry(shell_key)
            .or_insert_with(|| crate::model::FilesConfig { paths: vec![] });
        files_config.paths.push(raw_path.clone());
        self.config.save()?;

        // Dedup new files against everything already loaded.
        let mut seen: std::collections::HashSet<std::path::PathBuf> =
            self.profile.files.iter().map(|f| f.path.clone()).collect();

        let parser = crate::parser::get_parser(self.profile.shell_type);
        let resolved = crate::config::path_resolver::resolve_patterns(&[raw_path]);

        let mut new_indices: Vec<usize> = Vec::new();
        for rp in resolved {
            new_indices.extend(crate::model::profile::append_resolved_pattern(
                rp,
                parser.as_ref(),
                &mut self.profile.files,
                &mut self.profile.tree,
                &mut seen,
            )?);
        }

        // Expand the freshly added node(s) and compute writability the same way
        // the initial load does (missing files are treated as read-only).
        if let Some(last) = self.profile.tree.last_mut() {
            match last {
                crate::model::profile::TreeNode::File(_) => {}
                crate::model::profile::TreeNode::Dir(g) => g.expanded = true,
            }
        }
        for &fi in &new_indices {
            let file = &mut self.profile.files[fi];
            file.expanded = true;
            file.writable = if file.exists {
                crate::utils::path::check_writable(&file.path)
            } else {
                false
            };
        }

        self.rebuild_list();
        self.message = Some(match new_indices.len() {
            0 => "Pattern added (0 files matched)".into(),
            1 => "File added to config".into(),
            n => format!("Pattern added ({} files)", n),
        });
        Ok(())
    }

    pub(super) fn reload_profile(&mut self) -> Result<()> {
        let old_file_expanded: HashMap<std::path::PathBuf, bool> = self
            .profile
            .files
            .iter()
            .map(|f| (f.path.clone(), f.expanded))
            .collect();
        let old_dir_expanded: HashMap<String, bool> = self
            .profile
            .tree
            .iter()
            .filter_map(|n| match n {
                TreeNode::Dir(g) => Some((g.source_pattern.clone(), g.expanded)),
                _ => None,
            })
            .collect();

        let old_path_set: HashSet<std::path::PathBuf> =
            self.profile.files.iter().map(|f| f.path.clone()).collect();
        let mut dirty_snapshot: HashMap<std::path::PathBuf, ProfileFile> = HashMap::new();
        let old_files = std::mem::take(&mut self.profile.files);
        for f in old_files {
            if f.dirty {
                dirty_snapshot.insert(f.path.clone(), f);
            }
        }

        let mut new_profile =
            crate::model::profile::load_shell_profile(&self.config, self.profile.shell_type)?;

        for f in &mut new_profile.files {
            if let Some(&e) = old_file_expanded.get(&f.path) {
                f.expanded = e;
            }
        }
        for n in &mut new_profile.tree {
            if let TreeNode::Dir(g) = n {
                if let Some(&e) = old_dir_expanded.get(&g.source_pattern) {
                    g.expanded = e;
                }
            }
        }

        for (new_fi, nf) in new_profile.files.iter_mut().enumerate() {
            if let Some(mut old_pf) = dirty_snapshot.remove(&nf.path) {
                for e in &mut old_pf.entries {
                    e.file_index = new_fi;
                }
                nf.entries = old_pf.entries;
                nf.content = old_pf.content;
                nf.dirty = true;
            }
        }

        let new_path_set: HashSet<std::path::PathBuf> =
            new_profile.files.iter().map(|f| f.path.clone()).collect();
        if old_path_set != new_path_set {
            self.undo_stack.clear();
            self.redo_stack.clear();
            self.message = Some("Undo cleared (file set changed)".into());
        }

        self.profile = new_profile;
        self.visible_items = self.profile.build_visible_list();
        if self.cursor >= self.visible_items.len() {
            self.cursor = self.visible_items.len().saturating_sub(1);
        }
        Ok(())
    }

    pub(super) fn action_undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop_back() {
            let current = crate::tui::operations::take_snapshot(&self.profile);
            self.redo_stack.push(current);
            crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
            self.selection.clear();
            self.rebuild_list();
            let remaining = self.undo_stack.len();
            self.message = Some(format!("Undone ({remaining} left)"));
        } else {
            self.message = Some(self.messages.tui_msg_nothing_to_undo.into());
        }
    }

    pub(super) fn action_redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = crate::tui::operations::take_snapshot(&self.profile);
            self.undo_stack.push_back(current);
            if self.undo_stack.len() > crate::tui::operations::MAX_UNDO_HISTORY {
                self.undo_stack.pop_front();
            }
            crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
            self.selection.clear();
            self.rebuild_list();
            let remaining = self.redo_stack.len();
            self.message = Some(format!("Redone ({remaining} left)"));
        } else {
            self.message = Some("Nothing to redo".into());
        }
    }

    pub(super) fn action_save(&mut self) {
        match crate::tui::operations::save_dirty_files(&mut self.profile) {
            Ok(saved) if !saved.is_empty() => {
                self.message = Some(format!("Saved: {}", saved.join(", ")));
            }
            Ok(_) => {
                self.message = Some("No unsaved changes".into());
            }
            Err(e) => {
                self.message = Some(format!("Save error: {}", e));
            }
        }
    }

    pub(super) fn action_delete(&mut self) -> Result<()> {
        if let Some(ListItem::DirHeader(ti)) = self.visible_items.get(self.cursor) {
            let ti = *ti;
            if let Some(TreeNode::Dir(g)) = self.profile.tree.get(ti) {
                let pattern = g.source_pattern.clone();
                let file_count = g.file_indices.len();
                self.pending_remove_group_pattern = Some(pattern.clone());
                self.previous_mode = Some(self.mode.clone());
                self.mode = crate::tui::state::AppMode::ConfirmRemoveGroup;
                self.message = Some(format!(
                    "Remove group '{}' from config? ({} files will be hidden) (y/n)",
                    pattern, file_count
                ));
            }
        } else if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
            let fi = *fi;
            // A file inside a directory group isn't its own config entry, so
            // "remove from config" doesn't apply — delete the real file instead
            // (moved to trash, recoverable). Standalone file headers keep the
            // config-removal behavior below.
            if self.group_index_of_file(fi).is_some() {
                let name = self.profile.files[fi]
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| self.profile.files[fi].path.display().to_string());
                self.pending_delete_file_fi = Some(fi);
                self.previous_mode = Some(self.mode.clone());
                self.mode = crate::tui::state::AppMode::ConfirmDeleteFile;
                let unsaved = if self.profile.files[fi].dirty {
                    " Unsaved edits will be lost."
                } else {
                    ""
                };
                self.message = Some(format!("Move file '{}' to trash?{} (y/n)", name, unsaved));
                return Ok(());
            }
            let resolved_path = &self.profile.files[fi].path;

            let (raw_pattern, affected_files) =
                crate::tui::operations::find_matching_config_pattern(
                    &self.config,
                    &self.shell_key,
                    resolved_path,
                )
                .unwrap_or_else(|| {
                    (
                        resolved_path.display().to_string(),
                        vec![resolved_path.clone()],
                    )
                });

            self.pending_remove_fi = Some(fi);
            self.previous_mode = Some(self.mode.clone());
            self.mode = crate::tui::state::AppMode::ConfirmRemoveFile;

            if affected_files.len() <= 1 {
                self.message = Some(format!(
                    "Remove '{}' from config? (y/n)\n(file won't be deleted)",
                    raw_pattern
                ));
            } else {
                let other_files: Vec<String> = affected_files
                    .iter()
                    .filter(|p| p.as_path() != resolved_path)
                    .map(|p| format!("  {}", p.display()))
                    .collect();
                self.message = Some(format!(
                    "Remove '{}' from config? (y/n)\nAlso removes:\n{}\n(files won't be deleted)",
                    raw_pattern,
                    other_files.join("\n")
                ));
            }
        } else {
            // Original entry deletion logic
            if !self.is_current_file_writable() {
                self.message = Some("File is read-only".into());
                return Ok(());
            }
            let targets = self.get_operation_targets();
            if !targets.is_empty() {
                self.pending_undo = Some(crate::tui::operations::take_snapshot(&self.profile));
                self.previous_mode = Some(self.mode.clone());
                self.mode = crate::tui::state::AppMode::ConfirmDelete;
                let count = targets.len();
                self.message = Some(format!("Delete {} entries? (y/n)", count));
            }
        }
        Ok(())
    }
}
