//! Confirmation and cancellation handlers per AppMode.

use anyhow::Result;

use crate::tui::app::{EditorRequest, TuiApp};
use crate::tui::state::AppMode;

impl TuiApp {
    pub(super) fn action_confirm(&mut self) -> Result<EditorRequest> {
        match &self.mode {
            AppMode::InlineEdit => {
                self.inline_commit();
            }
            AppMode::FilterInput => {
                // Commit: enter browsing mode with the active filter
                self.mode = AppMode::FilterActive;
            }
            AppMode::MovingFile => {
                self.execute_file_move();
            }
            AppMode::Moving => {
                self.execute_drop();
            }
            AppMode::ConfirmDelete => {
                let targets = self.get_operation_targets();
                if let Some(snapshot) = self.pending_undo.take() {
                    crate::tui::operations::push_undo(
                        &mut self.undo_stack,
                        &mut self.redo_stack,
                        snapshot,
                    );
                }
                crate::tui::operations::delete_entries(
                    &mut self.profile,
                    &self.visible_items,
                    &targets,
                );
                self.selection.clear();
                let return_to_filter = matches!(
                    self.previous_mode,
                    Some(AppMode::FilterInput) | Some(AppMode::FilterActive)
                );
                self.mode = if return_to_filter {
                    AppMode::FilterActive
                } else {
                    AppMode::Normal
                };
                self.previous_mode = None;
                if return_to_filter {
                    self.update_filter();
                } else {
                    self.rebuild_list();
                }
                self.message = Some("Deleted".into());
            }
            AppMode::ConfirmQuit => {
                self.should_quit = true;
            }
            AppMode::ConfirmRemoveFile => {
                if let Some(fi) = self.pending_remove_fi.take() {
                    let path = self.profile.files[fi].path.clone();
                    let shell_key = self.shell_key.clone();

                    // Find the matching config pattern to remove (if any).
                    let raw_pattern = crate::tui::operations::find_matching_config_pattern(
                        &self.config,
                        &shell_key,
                        &path,
                    )
                    .map(|(pat, _paths)| pat);

                    if let Some(files_config) = self.config.files.get_mut(&shell_key) {
                        // Remove the matching pattern from config
                        if let Some(ref pat) = raw_pattern {
                            files_config.paths.retain(|p| p != pat);
                        }

                        if let Err(e) = self.config.save() {
                            self.message = Some(format!("Config save error: {}", e));
                        } else {
                            // Rebuild the profile from the updated config. This keeps
                            // `profile.files` and `profile.tree` in sync (a manual
                            // retain would leave stale indices in the tree and panic
                            // in build_visible_list).
                            let before = self.profile.files.len();
                            self.selection.clear();
                            self.reload_profile()?;
                            let removed_count = before.saturating_sub(self.profile.files.len());

                            if removed_count > 1 {
                                self.message =
                                    Some(format!("Removed {} files from config", removed_count));
                            } else {
                                self.message = Some("Removed from config".into());
                            }
                        }
                    }
                }
                self.mode = AppMode::Normal;
                self.previous_mode = None;
            }
            AppMode::ConfirmRemoveGroup => {
                if let Some(pattern) = self.pending_remove_group_pattern.take() {
                    let shell_key = self.shell_key.clone();
                    if let Some(files_config) = self.config.files.get_mut(&shell_key) {
                        files_config.paths.retain(|p| p != &pattern);
                    }
                    if let Err(e) = self.config.save() {
                        self.message = Some(format!("Config save error: {}", e));
                    } else {
                        self.selection.clear();
                        self.reload_profile()?;
                        self.message = Some(format!("Group '{}' removed from config", pattern));
                    }
                }
                self.mode = AppMode::Normal;
                self.previous_mode = None;
            }
            AppMode::ConfirmCreateFile => {
                if let Some((raw_path, path)) = self.pending_create_path.take() {
                    if let Some(parent) = path.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            self.message = Some(format!("Failed to create directory: {}", e));
                            self.mode = AppMode::Normal;
                            return Ok(EditorRequest::None);
                        }
                    }
                    match std::fs::File::create(&path) {
                        Ok(_) => {
                            self.add_pattern_to_config_and_profile(raw_path)?;
                        }
                        Err(e) => {
                            self.message = Some(format!("Failed to create: {}", e));
                        }
                    }
                }
                self.mode = AppMode::Normal;
            }
            AppMode::ConfirmDeleteFile => {
                if let Some(fi) = self.pending_delete_file_fi.take() {
                    let path = self.profile.files[fi].path.clone();
                    match trash::delete(&path) {
                        Ok(()) => {
                            self.selection.clear();
                            self.reload_profile()?;
                            self.message = Some(format!("Moved to trash: {}", path.display()));
                        }
                        Err(e) => {
                            self.message =
                                Some(format!("Trash unavailable, file left in place: {}", e));
                        }
                    }
                }
                self.mode = AppMode::Normal;
                self.previous_mode = None;
            }
            AppMode::ShowingDetail | AppMode::ShowingHelp => {
                self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
            }
            AppMode::TextInput => {
                if let Some(input) = self.text_input.take() {
                    match input.purpose {
                        crate::tui::state::InputPurpose::AddFilePath => {
                            let raw_path = input.value.trim().to_string();
                            if raw_path.is_empty() {
                                self.mode = AppMode::Normal;
                                return Ok(EditorRequest::None);
                            }

                            // Reject re-adding the exact same pattern string.
                            if self
                                .config
                                .files
                                .get(&self.shell_key)
                                .is_some_and(|fc| fc.paths.iter().any(|p| p == &raw_path))
                            {
                                self.message = Some("Pattern already in config".into());
                                self.mode = AppMode::Normal;
                                return Ok(EditorRequest::None);
                            }

                            let expanded = crate::config::path_resolver::expand_env_vars(
                                &crate::config::path_resolver::expand_tilde(&raw_path),
                            );
                            let path = std::path::PathBuf::from(&expanded);
                            // Glob patterns and directories resolve to a set of
                            // files; only a single concrete missing file offers
                            // the "create?" prompt.
                            let is_glob = expanded.contains('*') || expanded.contains('?');

                            if !is_glob && !path.exists() {
                                self.pending_create_path = Some((raw_path, path));
                                self.mode = AppMode::ConfirmCreateFile;
                                self.message = Some("File doesn't exist. Create? (y/n)".into());
                            } else {
                                self.add_pattern_to_config_and_profile(raw_path)?;
                                self.mode = AppMode::Normal;
                            }
                        }
                        crate::tui::state::InputPurpose::NewFileInDir { dir, pattern } => {
                            self.mode = AppMode::Normal;
                            let name = input.value.trim();
                            if name.is_empty() {
                                return Ok(EditorRequest::None);
                            }
                            let path = dir.join(name);
                            if path.exists() {
                                self.message = Some("File already exists".into());
                                return Ok(EditorRequest::None);
                            }
                            if let Some(parent) = path.parent() {
                                if let Err(e) = std::fs::create_dir_all(parent) {
                                    self.message =
                                        Some(format!("Failed to create directory: {}", e));
                                    return Ok(EditorRequest::None);
                                }
                            }
                            if let Err(e) = std::fs::File::create(&path) {
                                self.message = Some(format!("Failed to create file: {}", e));
                                return Ok(EditorRequest::None);
                            }
                            // Warn if the new name won't match the group's glob
                            // pattern (it will be created but won't rejoin the
                            // group on the next reload).
                            let warn_unmatched = {
                                let expanded = crate::config::path_resolver::expand_env_vars(
                                    &crate::config::path_resolver::expand_tilde(&pattern),
                                );
                                (expanded.contains('*') || expanded.contains('?'))
                                    && glob::Pattern::new(&expanded)
                                        .map(|p| !p.matches_path(&path))
                                        .unwrap_or(false)
                            };
                            self.selection.clear();
                            self.reload_profile()?;
                            let new_fi = self.profile.files.iter().position(|f| f.path == path);
                            if let Some(fi) = new_fi {
                                self.profile.files[fi].expanded = true;
                                if warn_unmatched {
                                    self.message = Some(
                                        "Created (won't rejoin group on reload — name doesn't match pattern)"
                                            .into(),
                                    );
                                }
                                return Ok(EditorRequest::EditFile(fi));
                            }
                            self.message = Some(format!("Created: {}", path.display()));
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(EditorRequest::None)
    }

    pub(super) fn action_cancel(&mut self) {
        match &self.mode {
            AppMode::FilterInput | AppMode::FilterActive => {
                // Restore expanded state from snapshot, then clear filter
                if let Some(snap) = self.expanded_snapshot.take() {
                    for (i, file) in self.profile.files.iter_mut().enumerate() {
                        if let Some(&v) = snap.files.get(i) {
                            file.expanded = v;
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
                self.search = None;
                self.mode = AppMode::Normal;
                self.rebuild_list();
                self.message = None;
            }
            AppMode::MovingFile => {
                self.cancel_file_move();
            }
            AppMode::Moving => {
                // Placement made no mutations (snapshot is taken at drop), so
                // cancelling just exits the mode — nothing to restore.
                let from_sel = self.move_state.as_ref().is_some_and(|ms| ms.from_selection);
                self.move_state = None;
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
                if from_sel {
                    // First Esc: keep selection, user can Esc again to clear
                    self.message = Some("Cancelled (Esc again to clear selection)".into());
                } else {
                    self.selection.clear();
                    self.message = Some("Cancelled".into());
                }
            }
            AppMode::ConfirmDelete => {
                self.pending_undo = None;
                self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                self.message = Some("Cancelled".into());
            }
            AppMode::ConfirmQuit => {
                self.mode = AppMode::Normal;
                self.message = None;
            }
            AppMode::ShowingDetail | AppMode::ShowingHelp => {
                self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
            }
            AppMode::Normal => {
                // In Normal mode, Esc clears multi-selection if any
                if !self.selection.is_empty() {
                    self.selection.clear();
                    self.message = Some("Selection cleared".into());
                }
            }
            AppMode::TextInput => {
                self.text_input = None;
                self.mode = AppMode::Normal;
                self.message = None;
            }
            AppMode::ConfirmRemoveFile
            | AppMode::ConfirmRemoveGroup
            | AppMode::ConfirmCreateFile
            | AppMode::ConfirmDeleteFile => {
                self.pending_remove_fi = None;
                self.pending_remove_group_pattern = None;
                self.pending_create_path = None;
                self.pending_delete_file_fi = None;
                self.mode = AppMode::Normal;
                self.message = Some("Cancelled".into());
            }
            AppMode::SelectingSnippet => {
                let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                self.mode = if matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive) {
                    AppMode::FilterActive
                } else {
                    AppMode::Normal
                };
            }
            AppMode::InlineEdit => self.inline_cancel(),
        }
    }
}
