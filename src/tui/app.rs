//! TUI application core

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::VecDeque;
use std::io;

use crate::i18n::Messages;
use crate::model::profile::{ListItem, ShellProfile};
use crate::tui::keys::{self, Action};
use crate::tui::list;
use crate::tui::search::SearchState;
use crate::tui::selection::SelectionState;
use crate::tui::state::{AppMode, ClipboardState, FileMovingState, MoveState};

enum EditorRequest {
    None,
    EditFile(usize),         // file index
    EditEntry(usize, usize), // file index, entry index
    AddEntry(usize),         // target file index
}

pub struct TuiApp {
    pub profile: ShellProfile,
    pub visible_items: Vec<ListItem>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub mode: AppMode,
    pub previous_mode: Option<AppMode>,
    pub should_quit: bool,
    pub message: Option<String>,
    pub messages: &'static Messages,
    pub selection: SelectionState,
    pub clipboard: ClipboardState,
    pub undo_stack: VecDeque<crate::tui::state::UndoSnapshot>,
    pub redo_stack: Vec<crate::tui::state::UndoSnapshot>,
    pub move_state: Option<MoveState>,
    pub search: Option<SearchState>,
    pub list_visible_height: usize,
    pub config: crate::model::Config,
    pub shell_key: String,
    pub pending_remove_fi: Option<usize>,
    pub text_input: Option<crate::tui::state::TextInputState>,
    pub pending_create_path: Option<(String, std::path::PathBuf)>,
    pub file_move_state: Option<FileMovingState>,
    pub detail_scroll_offset: u16,
    pub detail_page_size: u16,
}

impl TuiApp {
    pub fn new(
        profile: ShellProfile,
        messages: &'static Messages,
        config: crate::model::Config,
        shell_key: String,
    ) -> Result<Self> {
        let visible_items = profile.build_visible_list();
        Ok(Self {
            profile,
            visible_items,
            cursor: 0,
            scroll_offset: 0,
            mode: AppMode::Normal,
            previous_mode: None,
            should_quit: false,
            message: None,
            messages,
            selection: SelectionState::new(),
            clipboard: ClipboardState::new(),
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            move_state: None,
            search: None,
            list_visible_height: 20,
            config,
            shell_key,
            pending_remove_fi: None,
            text_input: None,
            pending_create_path: None,
            file_move_state: None,
            detail_scroll_offset: 0,
            detail_page_size: 10,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        // Always restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|f| {
                // Update list_visible_height before drawing (area height minus title, status, search bar, header+separator)
                let total_height = f.size().height as usize;
                let chrome = if self.search.is_some() { 4 } else { 3 }; // title + status + search? + header/separator
                self.list_visible_height = total_height.saturating_sub(chrome);
                self.clamp_scroll_offset();
                crate::tui::ui::draw(f, self);
            })?;

            if let Event::Key(key) = event::read()? {
                // Ignore key release events on Windows
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue;
                }
                let action = keys::map_key(&self.mode, key);
                let request = self.handle_action(action)?;
                match request {
                    EditorRequest::EditFile(fi) => {
                        self.run_edit_file(terminal, fi)?;
                        if self.mode == AppMode::Searching {
                            self.update_search_and_navigate();
                        }
                    }
                    EditorRequest::EditEntry(fi, ei) => {
                        self.run_edit_entry(terminal, fi, ei)?;
                        if self.mode == AppMode::Searching {
                            self.update_search_and_navigate();
                        }
                    }
                    EditorRequest::AddEntry(fi) => {
                        self.run_add_entry(terminal, fi)?;
                        if self.mode == AppMode::Searching {
                            self.update_search_and_navigate();
                        }
                    }
                    EditorRequest::None => {}
                }
            }
        }
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<EditorRequest> {
        self.message = None; // Clear message on any action

        match action {
            Action::NavigateUp => {
                if self.mode == AppMode::Searching {
                    if let Some(ref mut search) = self.search {
                        search.select_prev();
                    }
                    self.navigate_to_search_match();
                } else if self.mode == AppMode::MovingFile {
                    if let Some(ref mut fms) = self.file_move_state {
                        if fms.insertion_cursor > 0 {
                            fms.insertion_cursor -= 1;
                        }
                    }
                } else if self.mode == AppMode::Moving {
                    self.move_cursor_up();
                } else {
                    self.selection.commit_range();
                    self.cursor = list::navigate_up(&self.visible_items, self.cursor);
                    self.clamp_scroll_offset();
                }
            }
            Action::NavigateDown => {
                if self.mode == AppMode::Searching {
                    if let Some(ref mut search) = self.search {
                        search.select_next();
                    }
                    self.navigate_to_search_match();
                } else if self.mode == AppMode::MovingFile {
                    if let Some(ref mut fms) = self.file_move_state {
                        let max_idx = self.visible_items.len().saturating_sub(1);
                        if fms.insertion_cursor < max_idx {
                            fms.insertion_cursor += 1;
                        }
                    }
                } else if self.mode == AppMode::Moving {
                    self.move_cursor_down();
                } else {
                    self.selection.commit_range();
                    self.cursor = list::navigate_down(&self.visible_items, self.cursor);
                    self.clamp_scroll_offset();
                }
            }
            Action::PageUp => {
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
                } else {
                    self.selection.commit_range();
                    self.cursor = self.cursor.saturating_sub(half);
                    self.clamp_scroll_offset();
                }
            }
            Action::PageDown => {
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
                } else {
                    self.selection.commit_range();
                    self.cursor = (self.cursor + half).min(max_idx);
                    self.clamp_scroll_offset();
                }
            }
            Action::Home => {
                if self.mode == AppMode::Searching {
                    if let Some(ref mut search) = self.search {
                        search.select_first();
                    }
                    self.navigate_to_search_match();
                } else {
                    self.selection.commit_range();
                    self.cursor = list::navigate_home();
                    self.clamp_scroll_offset();
                }
            }
            Action::End => {
                if self.mode == AppMode::Searching {
                    if let Some(ref mut search) = self.search {
                        search.select_last();
                    }
                    self.navigate_to_search_match();
                } else {
                    self.selection.commit_range();
                    self.cursor = list::navigate_end(&self.visible_items);
                    self.clamp_scroll_offset();
                }
            }
            Action::ToggleExpand => {
                if let Some(item) = self.visible_items.get(self.cursor) {
                    match item {
                        ListItem::FileHeader(_) => {
                            if self.mode != AppMode::Searching {
                                self.toggle_at_cursor();
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
            Action::CollapseAll => {
                self.profile.toggle_all(false);
                self.selection.clear();
                self.rebuild_list();
            }
            Action::ExpandAll => {
                self.profile.toggle_all(true);
                self.selection.clear();
                self.rebuild_list();
            }
            Action::ToggleSelect => {
                self.selection.toggle(self.cursor, &self.visible_items);
            }
            Action::RangeSelectUp => {
                self.selection.set_range(self.cursor, &self.visible_items);
                self.cursor = list::navigate_up(&self.visible_items, self.cursor);
                self.clamp_scroll_offset();
                self.selection.set_range(self.cursor, &self.visible_items);
            }
            Action::RangeSelectDown => {
                self.selection.set_range(self.cursor, &self.visible_items);
                self.cursor = list::navigate_down(&self.visible_items, self.cursor);
                self.clamp_scroll_offset();
                self.selection.set_range(self.cursor, &self.visible_items);
            }
            Action::Edit => {
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
                        ListItem::Entry(fi, ei) => {
                            if !self.profile.files[*fi].writable {
                                self.message = Some("File is read-only".into());
                                return Ok(EditorRequest::None);
                            }
                            return Ok(EditorRequest::EditEntry(*fi, *ei));
                        }
                    }
                }
            }
            Action::Add => {
                if !self.is_current_file_writable() {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }
                let fi = self.current_file_index();
                return Ok(EditorRequest::AddEntry(fi));
            }
            Action::Delete => {
                if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
                    let fi = *fi;
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
                    self.mode = AppMode::ConfirmRemoveFile;

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
                        return Ok(EditorRequest::None);
                    }
                    let targets = self.get_operation_targets();
                    if !targets.is_empty() {
                        let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                        crate::tui::operations::push_undo(
                            &mut self.undo_stack,
                            &mut self.redo_stack,
                            snapshot,
                        );
                        self.previous_mode = Some(self.mode.clone());
                        self.mode = AppMode::ConfirmDelete;
                        let count = targets.len();
                        self.message = Some(format!("Delete {} entries? (y/n)", count));
                    }
                }
            }
            Action::Cut => {
                if !self.is_current_file_writable() {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }
                let targets = self.get_operation_targets();
                if !targets.is_empty() {
                    let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                    crate::tui::operations::push_undo(
                        &mut self.undo_stack,
                        &mut self.redo_stack,
                        snapshot,
                    );
                    let cut = crate::tui::operations::cut_entries(
                        &mut self.profile,
                        &self.visible_items,
                        &targets,
                    );
                    let count = cut.len();
                    self.clipboard.entries = cut;
                    self.selection.clear();
                    self.rebuild_list();
                    self.message = Some(format!("Cut {} entries", count));
                }
            }
            Action::Copy => {
                let targets = self.get_operation_targets();
                if !targets.is_empty() {
                    let copied: Vec<crate::model::Entry> = targets
                        .iter()
                        .filter_map(|&idx| match self.visible_items.get(idx) {
                            Some(ListItem::Entry(fi, ei)) => {
                                Some(self.profile.files[*fi].entries[*ei].clone())
                            }
                            _ => None,
                        })
                        .collect();
                    let count = copied.len();
                    self.clipboard.entries = copied;
                    self.message = Some(format!("Copied {} entries", count));
                }
            }
            Action::StartMove => {
                // Check if cursor is on a FileHeader → enter file move mode
                if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
                    let fi = *fi;
                    if self.profile.files.len() < 2 {
                        self.message = Some("Only one file, nothing to move".into());
                        return Ok(EditorRequest::None);
                    }
                    let saved_expanded: Vec<bool> =
                        self.profile.files.iter().map(|f| f.expanded).collect();
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
                    self.mode = AppMode::MovingFile;
                    self.message =
                        Some("File move: ↑↓ to position, Enter to drop, Esc to cancel".into());
                    return Ok(EditorRequest::None);
                }

                if !self.is_current_file_writable() {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }
                let targets = self.get_operation_targets();
                if !targets.is_empty() {
                    let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                    crate::tui::operations::push_undo(
                        &mut self.undo_stack,
                        &mut self.redo_stack,
                        snapshot,
                    );

                    let has_selection = !self.selection.is_empty();

                    // If from multi-selection, jump cursor to first selected row
                    if has_selection {
                        let first = self.selection.sorted_indices()[0];
                        self.cursor = first;
                        self.clamp_scroll_offset();
                    }

                    let source_items: Vec<(usize, usize)> = targets
                        .iter()
                        .filter_map(|&idx| match self.visible_items.get(idx) {
                            Some(ListItem::Entry(fi, ei)) => Some((*fi, *ei)),
                            _ => None,
                        })
                        .collect();

                    if !source_items.is_empty() {
                        self.move_state = Some(MoveState {
                            source_items,
                            insertion_cursor: self.cursor,
                            from_selection: has_selection,
                        });
                        self.mode = AppMode::Moving;
                        self.message =
                            Some("Move mode: ↑↓ to position, Enter to drop, Esc to cancel".into());
                    }
                }
            }
            Action::Paste => {
                if !self.is_current_file_writable() {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }
                if !self.clipboard.is_empty() {
                    let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                    crate::tui::operations::push_undo(
                        &mut self.undo_stack,
                        &mut self.redo_stack,
                        snapshot,
                    );
                    crate::tui::operations::paste_entries(
                        &mut self.profile,
                        &self.visible_items,
                        self.cursor,
                        &self.clipboard.entries,
                    );
                    self.rebuild_list();
                    self.message = Some(format!("Pasted {} entries", self.clipboard.entries.len()));
                } else {
                    self.message = Some("Clipboard empty".into());
                }
            }
            Action::Undo => {
                if let Some(snapshot) = self.undo_stack.pop_back() {
                    let current = crate::tui::operations::take_snapshot(&self.profile);
                    self.redo_stack.push(current);
                    crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                    self.selection.clear();
                    self.rebuild_list();
                    let remaining = self.undo_stack.len();
                    self.message = Some(format!("Undone ({remaining} left)"));
                } else {
                    self.message = Some("Nothing to undo".into());
                }
            }
            Action::Redo => {
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
            Action::Save => match crate::tui::operations::save_dirty_files(&mut self.profile) {
                Ok(saved) if !saved.is_empty() => {
                    self.message = Some(format!("Saved: {}", saved.join(", ")));
                }
                Ok(_) => {
                    self.message = Some("No unsaved changes".into());
                }
                Err(e) => {
                    self.message = Some(format!("Save error: {}", e));
                }
            },
            Action::Confirm => {
                match &self.mode {
                    AppMode::Searching => {
                        // Jump to current match and exit search
                        self.navigate_to_search_match();
                        self.search = None;
                        self.mode = AppMode::Normal;
                    }
                    AppMode::MovingFile => {
                        self.execute_file_move();
                    }
                    AppMode::Moving => {
                        self.execute_move();
                    }
                    AppMode::ConfirmDelete => {
                        let targets = self.get_operation_targets();
                        crate::tui::operations::delete_entries(
                            &mut self.profile,
                            &self.visible_items,
                            &targets,
                        );
                        self.selection.clear();
                        let return_to_search =
                            matches!(self.previous_mode, Some(AppMode::Searching));
                        self.mode = if return_to_search {
                            AppMode::Searching
                        } else {
                            AppMode::Normal
                        };
                        self.previous_mode = None;
                        self.rebuild_list();
                        if return_to_search {
                            self.update_search_and_navigate();
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

                            // Use helper to find matching pattern and all affected paths
                            let match_result = crate::tui::operations::find_matching_config_pattern(
                                &self.config,
                                &shell_key,
                                &path,
                            );

                            let (raw_pattern, affected_paths) = match match_result {
                                Some((pat, paths)) => (Some(pat), paths),
                                None => (None, vec![path.clone()]),
                            };

                            if let Some(files_config) = self.config.files.get_mut(&shell_key) {
                                // Remove the matching pattern from config
                                if let Some(ref pat) = raw_pattern {
                                    files_config.paths.retain(|p| p != pat);
                                }

                                if let Err(e) = self.config.save() {
                                    self.message = Some(format!("Config save error: {}", e));
                                } else {
                                    // Remove ALL affected files from profile
                                    let before = self.profile.files.len();
                                    self.profile
                                        .files
                                        .retain(|f| !affected_paths.contains(&f.path));
                                    let removed_count = before - self.profile.files.len();

                                    // Recalculate file_index for remaining entries
                                    for (new_fi, file) in self.profile.files.iter_mut().enumerate()
                                    {
                                        for entry in &mut file.entries {
                                            entry.file_index = new_fi;
                                        }
                                    }

                                    self.selection.clear();
                                    self.rebuild_list();
                                    if removed_count > 1 {
                                        self.message = Some(format!(
                                            "Removed {} files from config",
                                            removed_count
                                        ));
                                    } else {
                                        self.message = Some("Removed from config".into());
                                    }
                                }
                            }
                        }
                        self.mode = AppMode::Normal;
                        self.previous_mode = None;
                    }
                    AppMode::ConfirmCreateFile => {
                        if let Some((raw_path, path)) = self.pending_create_path.take() {
                            if let Some(parent) = path.parent() {
                                if let Err(e) = std::fs::create_dir_all(parent) {
                                    self.message =
                                        Some(format!("Failed to create directory: {}", e));
                                    self.mode = AppMode::Normal;
                                    return Ok(EditorRequest::None);
                                }
                            }
                            match std::fs::File::create(&path) {
                                Ok(_) => {
                                    self.add_file_to_config_and_profile(raw_path, path)?;
                                }
                                Err(e) => {
                                    self.message = Some(format!("Failed to create: {}", e));
                                }
                            }
                        }
                        self.mode = AppMode::Normal;
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

                                    let expanded = crate::config::path_resolver::expand_env_vars(
                                        &crate::config::path_resolver::expand_tilde(&raw_path),
                                    );
                                    let path = std::path::PathBuf::from(&expanded);

                                    if self.profile.files.iter().any(|f| f.path == path) {
                                        self.message = Some("Path already in config".into());
                                        self.mode = AppMode::Normal;
                                        return Ok(EditorRequest::None);
                                    }

                                    if !path.exists() {
                                        self.pending_create_path = Some((raw_path, path));
                                        self.mode = AppMode::ConfirmCreateFile;
                                        self.message =
                                            Some("File doesn't exist. Create? (y/n)".into());
                                    } else {
                                        self.add_file_to_config_and_profile(raw_path, path)?;
                                        self.mode = AppMode::Normal;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Action::Cancel => {
                match &self.mode {
                    AppMode::Searching => {
                        self.search = None;
                        self.mode = AppMode::Normal;
                        self.message = None;
                    }
                    AppMode::MovingFile => {
                        self.cancel_file_move();
                    }
                    AppMode::Moving => {
                        let from_sel = self.move_state.as_ref().is_some_and(|ms| ms.from_selection);
                        // Pop the pre-emptive undo snapshot and restore from it
                        if let Some(snapshot) = self.undo_stack.pop_back() {
                            crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                        }
                        self.move_state = None;
                        self.mode = AppMode::Normal;
                        self.rebuild_list();
                        if from_sel {
                            // First Esc: keep selection, user can Esc again to clear
                            self.message =
                                Some("Move cancelled (Esc again to clear selection)".into());
                        } else {
                            self.selection.clear();
                            self.message = Some("Move cancelled".into());
                        }
                    }
                    AppMode::ConfirmDelete => {
                        self.undo_stack.pop_back(); // Discard pre-emptive snapshot
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
                    AppMode::ConfirmRemoveFile | AppMode::ConfirmCreateFile => {
                        self.pending_remove_fi = None;
                        self.pending_create_path = None;
                        self.mode = AppMode::Normal;
                        self.message = Some("Cancelled".into());
                    }
                }
            }
            Action::Quit => {
                if self.profile.any_dirty() {
                    self.mode = AppMode::ConfirmQuit;
                    let dirty: Vec<_> = self
                        .profile
                        .dirty_files()
                        .iter()
                        .map(|f| f.display_name())
                        .collect();
                    self.message = Some(format!(
                        "Unsaved changes in: {}. Quit? (y/n)",
                        dirty.join(", ")
                    ));
                } else {
                    self.should_quit = true;
                }
            }
            Action::Search => {
                self.search = Some(SearchState::new());
                self.mode = AppMode::Searching;
                self.message = None;
            }
            Action::SearchInput(c) => {
                if let Some(ref mut search) = self.search {
                    search.input_char(c);
                }
                self.update_search_and_navigate();
            }
            Action::SearchBackspace => {
                if let Some(ref mut search) = self.search {
                    search.backspace();
                    if search.query.is_empty() {
                        self.search = None;
                        self.mode = AppMode::Normal;
                        return Ok(EditorRequest::None);
                    }
                }
                self.update_search_and_navigate();
            }
            Action::Help => {
                self.mode = AppMode::ShowingHelp;
            }
            Action::Remark => {
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
                    return Ok(EditorRequest::None);
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
                    return Ok(EditorRequest::None);
                }

                // Check all affected files are writable
                let affected_files: std::collections::HashSet<usize> =
                    target_entries.iter().map(|(fi, _)| *fi).collect();

                if affected_files
                    .iter()
                    .any(|&fi| !self.profile.files[fi].writable)
                {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }

                // Remember whether there was a pre-existing selection (Feature 3 fix)
                let had_selection = !self.selection.is_empty();

                // Determine if all targets are Comments
                let all_comment = target_entries.iter().all(|(fi, ei)| {
                    self.profile.files[*fi].entries[*ei].entry_type
                        == crate::model::EntryType::Comment
                });

                // Take undo snapshot
                let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                crate::tui::operations::push_undo(
                    &mut self.undo_stack,
                    &mut self.redo_stack,
                    snapshot,
                );

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
                // In ShowingDetail: stay in the popup (don't change mode)
            }
            Action::DetailScrollUp => {
                self.detail_scroll_offset = self.detail_scroll_offset.saturating_sub(1);
            }
            Action::DetailScrollDown => {
                self.detail_scroll_offset = self.detail_scroll_offset.saturating_add(1);
            }
            Action::DetailPageUp => {
                self.detail_scroll_offset = self
                    .detail_scroll_offset
                    .saturating_sub(self.detail_page_size);
            }
            Action::DetailPageDown => {
                self.detail_scroll_offset = self
                    .detail_scroll_offset
                    .saturating_add(self.detail_page_size);
            }
            Action::DetailHome => {
                self.detail_scroll_offset = 0;
            }
            Action::DetailEnd => {
                self.detail_scroll_offset = u16::MAX;
            }
            Action::AddFile => {
                self.text_input = Some(crate::tui::state::TextInputState {
                    prompt: "New file path: ".into(),
                    value: String::new(),
                    cursor_pos: 0,
                    purpose: crate::tui::state::InputPurpose::AddFilePath,
                });
                self.mode = AppMode::TextInput;
                self.message = None;
            }
            Action::TextInputChar(c) => {
                if let Some(ref mut input) = self.text_input {
                    input.value.insert(input.cursor_pos, c);
                    input.cursor_pos += 1;
                }
            }
            Action::TextInputBackspace => {
                if let Some(ref mut input) = self.text_input {
                    if input.cursor_pos > 0 {
                        input.cursor_pos -= 1;
                        input.value.remove(input.cursor_pos);
                    }
                }
            }
            Action::TextInputLeft => {
                if let Some(ref mut input) = self.text_input {
                    if input.cursor_pos > 0 {
                        input.cursor_pos -= 1;
                    }
                }
            }
            Action::TextInputRight => {
                if let Some(ref mut input) = self.text_input {
                    if input.cursor_pos < input.value.len() {
                        input.cursor_pos += 1;
                    }
                }
            }
            _ => {
                // Other actions not implemented yet
            }
        }
        Ok(EditorRequest::None)
    }

    fn toggle_at_cursor(&mut self) {
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
            ListItem::Entry(fi, _) => {
                // On Entry, toggle the parent file
                let fi = *fi;
                self.profile.files[fi].expanded = !self.profile.files[fi].expanded;
                self.rebuild_list();
            }
        }
    }

    pub fn rebuild_list(&mut self) {
        self.visible_items = self.profile.build_visible_list();
        if self.cursor >= self.visible_items.len() {
            self.cursor = self.visible_items.len().saturating_sub(1);
        }
        self.clamp_scroll_offset();
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

    /// Get the file index for the current cursor position
    fn current_file_index(&self) -> usize {
        match self.visible_items.get(self.cursor) {
            Some(ListItem::FileHeader(fi)) => *fi,
            Some(ListItem::Entry(fi, _)) => *fi,
            None => 0,
        }
    }

    fn add_file_to_config_and_profile(
        &mut self,
        raw_path: String,
        path: std::path::PathBuf,
    ) -> anyhow::Result<()> {
        let shell_key = self.shell_key.clone();
        let files_config = self
            .config
            .files
            .entry(shell_key)
            .or_insert_with(|| crate::model::FilesConfig { paths: vec![] });
        files_config.paths.push(raw_path);
        self.config.save()?;

        let fi = self.profile.files.len();
        let exists = path.exists();
        let content = if exists {
            std::fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };

        let parser = crate::parser::get_parser(self.profile.shell_type);
        let parsed = parser.parse(&content);
        let entries: Vec<_> = parsed
            .entries
            .into_iter()
            .map(|mut e| {
                e.file_index = fi;
                e
            })
            .collect();

        let mut file = crate::model::profile::ProfileFile::new(path.clone(), exists);
        file.entries = entries;
        file.content = content;
        file.expanded = true;
        file.writable = crate::utils::path::check_writable(&path);
        self.profile.files.push(file);

        self.rebuild_list();
        self.message = Some("File added to config".into());
        Ok(())
    }

    /// Check if the file under the cursor is writable
    fn is_current_file_writable(&self) -> bool {
        let fi = self.current_file_index();
        fi < self.profile.files.len() && self.profile.files[fi].writable
    }

    /// Navigate cursor to the currently selected search match
    fn navigate_to_search_match(&mut self) {
        if let Some(ref search) = self.search {
            if let Some((fi, ei)) = search.current_match() {
                // Ensure the file is expanded
                self.profile.files[fi].expanded = true;
                self.rebuild_list();
                // Find the entry in visible_items
                for (i, item) in self.visible_items.iter().enumerate() {
                    if matches!(item, ListItem::Entry(f, e) if *f == fi && *e == ei) {
                        self.cursor = i;
                        self.clamp_scroll_offset();
                        break;
                    }
                }
            }
        }
    }

    /// Expand files with search matches, collapse files without
    fn toggle_files_by_search(&mut self) {
        if let Some(ref search) = self.search {
            let matched_files = search.matched_file_indices();
            for (i, file) in self.profile.files.iter_mut().enumerate() {
                file.expanded = matched_files.contains(&i);
            }
            self.rebuild_list();
        }
    }

    /// Combined: update search matches, toggle files, navigate to match
    fn update_search_and_navigate(&mut self) {
        if let Some(ref mut search) = self.search {
            search.update_matches(&self.profile);
        }
        self.toggle_files_by_search();
        self.navigate_to_search_match();
    }

    /// Get visible-list indices for operation targets.
    /// Returns selected indices if any, otherwise cursor if on an entry.
    fn get_operation_targets(&self) -> Vec<usize> {
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

    fn execute_move(&mut self) {
        if let Some(ms) = self.move_state.take() {
            // Determine target file and position from insertion_cursor
            let (target_fi, target_pos) = match self.visible_items.get(ms.insertion_cursor) {
                Some(ListItem::Entry(fi, ei)) => (*fi, ei + 1), // Insert after this entry
                Some(ListItem::FileHeader(fi)) => (*fi, 0),     // Insert at start of file
                None => {
                    let fi = self.profile.files.len().saturating_sub(1);
                    (fi, self.profile.files[fi].entries.len())
                }
            };

            // Collect the entries to move (clone them before removing)
            let mut entries_to_move: Vec<crate::model::Entry> = Vec::new();
            for &(fi, ei) in &ms.source_items {
                if fi < self.profile.files.len() && ei < self.profile.files[fi].entries.len() {
                    entries_to_move.push(self.profile.files[fi].entries[ei].clone());
                }
            }

            // Remove source entries (reverse order to preserve indices)
            // Group by file, sort entry indices descending
            let mut by_file: std::collections::HashMap<usize, Vec<usize>> =
                std::collections::HashMap::new();
            for &(fi, ei) in &ms.source_items {
                by_file.entry(fi).or_default().push(ei);
            }
            let source_files: Vec<usize> = by_file.keys().cloned().collect();
            for (fi, mut indices) in by_file {
                indices.sort();
                indices.dedup();
                for &ei in indices.iter().rev() {
                    if ei < self.profile.files[fi].entries.len() {
                        self.profile.files[fi].entries.remove(ei);
                    }
                }
                self.profile.files[fi].dirty = true;
            }

            // Adjust target_pos if removals in the same file shifted indices
            // This is tricky — we need to recalculate since entries shifted.
            // Safest approach: rebuild list, then find position by count.
            // But simpler: just insert at target file, at min(target_pos, entries.len())
            let adjusted_pos = target_pos.min(self.profile.files[target_fi].entries.len());

            // Insert at target
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
            self.mode = AppMode::Normal;
            self.rebuild_list();
            self.cursor = ms
                .insertion_cursor
                .min(self.visible_items.len().saturating_sub(1));
            self.clamp_scroll_offset();
            self.message = Some("Moved".into());
        }
    }

    /// Execute file move: reorder file in config and profile
    fn execute_file_move(&mut self) {
        if let Some(fms) = self.file_move_state.take() {
            let target_fi = match self.visible_items.get(fms.insertion_cursor) {
                Some(ListItem::FileHeader(fi)) => *fi,
                _ => fms.original_fi,
            };

            if target_fi == fms.original_fi {
                // No change — just restore expanded states
                self.restore_expanded(&fms.saved_expanded);
                self.mode = AppMode::Normal;
                self.rebuild_list();
                self.message = Some("No change".into());
                return;
            }

            // Reorder profile.files
            let file = self.profile.files.remove(fms.original_fi);
            self.profile.files.insert(target_fi, file);

            // Fix file_index on all entries
            for (fi, f) in self.profile.files.iter_mut().enumerate() {
                for entry in &mut f.entries {
                    entry.file_index = fi;
                }
            }

            // Reorder config paths
            let shell_key = self.shell_key.clone();
            if let Some(files_config) = self.config.files.get_mut(&shell_key) {
                if fms.original_fi < files_config.paths.len()
                    && target_fi < files_config.paths.len()
                {
                    let path = files_config.paths.remove(fms.original_fi);
                    files_config.paths.insert(target_fi, path);
                }
            }
            let _ = self.config.save();

            // Restore expanded states mapped to new positions
            let mut new_expanded = fms.saved_expanded.clone();
            let removed = new_expanded.remove(fms.original_fi);
            new_expanded.insert(target_fi, removed);
            self.restore_expanded(&new_expanded);

            self.mode = AppMode::Normal;
            self.rebuild_list();

            // Place cursor on the moved file's new header
            let cursor_pos = self
                .visible_items
                .iter()
                .position(|item| matches!(item, ListItem::FileHeader(fi) if *fi == target_fi))
                .unwrap_or(0);
            self.cursor = cursor_pos;
            self.clamp_scroll_offset();
            self.message = Some("File moved".into());
        }
    }

    /// Cancel file move: restore expanded states
    fn cancel_file_move(&mut self) {
        if let Some(fms) = self.file_move_state.take() {
            self.restore_expanded(&fms.saved_expanded);
            self.mode = AppMode::Normal;
            self.rebuild_list();
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

    /// Restore per-file expanded states
    fn restore_expanded(&mut self, states: &[bool]) {
        for (i, file) in self.profile.files.iter_mut().enumerate() {
            if let Some(&expanded) = states.get(i) {
                file.expanded = expanded;
            }
        }
    }

    /// Check if a visible-list position belongs to a blocked file
    fn is_position_blocked(
        items: &[ListItem],
        files: &[crate::model::profile::ProfileFile],
        pos: usize,
    ) -> bool {
        let fi = match items.get(pos) {
            Some(ListItem::FileHeader(fi)) | Some(ListItem::Entry(fi, _)) => *fi,
            None => return true,
        };
        fi < files.len() && (!files[fi].exists || !files[fi].writable)
    }

    /// Move cursor up in Moving mode, skipping blocked files
    fn move_cursor_up(&mut self) {
        if let Some(ref mut ms) = self.move_state {
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
    fn move_cursor_down(&mut self) {
        if let Some(ref mut ms) = self.move_state {
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
    fn snap_move_cursor_to_non_blocked(&mut self) {
        if let Some(ref mut ms) = self.move_state {
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

    fn suspend_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn resume_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        enable_raw_mode()?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(())
    }

    /// Edit a file: suspend TUI, open in $EDITOR, re-parse, resume
    fn run_edit_file(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
    ) -> Result<()> {
        let path = self.profile.files[fi].path.clone();
        if !self.profile.files[fi].exists {
            self.message = Some(format!("File does not exist: {}", path.display()));
            return Ok(());
        }

        Self::suspend_tui(terminal)?;
        let modified = crate::tui::editor::edit_file(&path);
        Self::resume_tui(terminal)?;

        match modified {
            Ok(true) => {
                // Re-parse the file
                let content = std::fs::read_to_string(&path)?;
                let parser = crate::parser::get_parser(self.profile.shell_type);
                let result = parser.parse(&content);
                let file = &mut self.profile.files[fi];
                file.entries = result
                    .entries
                    .into_iter()
                    .map(|mut e| {
                        e.file_index = fi;
                        e
                    })
                    .collect();
                file.content = content;
                self.rebuild_list();
                self.message = Some(format!("Reloaded: {}", path.display()));
            }
            Ok(false) => {
                self.message = Some("No changes detected".into());
            }
            Err(e) => {
                self.message = Some(format!("Editor error: {}", e));
            }
        }
        Ok(())
    }

    /// Edit an entry: write value to temp file, open in $EDITOR, update entry
    fn run_edit_entry(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
        ei: usize,
    ) -> Result<()> {
        let suffix = match self.profile.shell_type {
            crate::model::ShellType::PowerShell => ".ps1",
            _ => ".sh",
        };
        let value = self.profile.files[fi].entries[ei].value.clone();

        Self::suspend_tui(terminal)?;
        let result = crate::tui::editor::edit_temp_content(&value, suffix);
        Self::resume_tui(terminal)?;

        match result {
            Ok(Some(new_content)) => {
                let new_content = new_content
                    .strip_suffix('\n')
                    .map(str::to_string)
                    .unwrap_or(new_content);
                if new_content != value {
                    let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                    crate::tui::operations::push_undo(
                        &mut self.undo_stack,
                        &mut self.redo_stack,
                        snapshot,
                    );
                    let parser = crate::parser::get_parser(self.profile.shell_type);
                    let parsed = parser.parse(&new_content);
                    let new_entries: Vec<_> = parsed
                        .entries
                        .into_iter()
                        .map(|mut e| {
                            e.file_index = fi;
                            e
                        })
                        .collect();

                    if new_entries.is_empty() {
                        // Empty edit = delete entry
                        self.profile.files[fi].entries.remove(ei);
                        self.profile.files[fi].dirty = true;
                        self.profile.files[fi].recalculate_line_numbers();
                        self.rebuild_list();
                        self.message = Some("Entry deleted (empty content)".into());
                    } else {
                        let count = crate::tui::operations::replace_entry_with_parsed(
                            &mut self.profile.files[fi],
                            ei,
                            new_entries,
                            fi,
                        );
                        self.rebuild_list();
                        self.message = Some(if count == 1 {
                            "Entry updated".into()
                        } else {
                            format!("Entry replaced with {} entries", count)
                        });
                    }
                } else {
                    self.message = Some("No changes".into());
                }
            }
            Ok(None) => {
                self.message = Some("No changes".into());
            }
            Err(e) => {
                self.message = Some(format!("Editor error: {}", e));
            }
        }
        Ok(())
    }

    /// Add a new entry: open empty temp file in $EDITOR, parse result, insert
    fn run_add_entry(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
    ) -> Result<()> {
        let suffix = match self.profile.shell_type {
            crate::model::ShellType::PowerShell => ".ps1",
            _ => ".sh",
        };

        Self::suspend_tui(terminal)?;
        let result = crate::tui::editor::edit_temp_content("", suffix);
        Self::resume_tui(terminal)?;

        match result {
            Ok(Some(content)) => {
                let content = content
                    .strip_suffix('\n')
                    .map(str::to_string)
                    .unwrap_or(content);
                if !content.is_empty() {
                    let parser = crate::parser::get_parser(self.profile.shell_type);
                    let parsed = parser.parse(&content);
                    let mut new_entries: Vec<_> = parsed
                        .entries
                        .into_iter()
                        .map(|mut e| {
                            e.file_index = fi;
                            e
                        })
                        .collect();

                    if !new_entries.is_empty() {
                        let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                        crate::tui::operations::push_undo(
                            &mut self.undo_stack,
                            &mut self.redo_stack,
                            snapshot,
                        );
                        // Insert after current entry position, or at end of file
                        let insert_pos = match self.visible_items.get(self.cursor) {
                            Some(ListItem::Entry(_, ei)) => ei + 1,
                            Some(ListItem::FileHeader(_)) => 0, // Insert at beginning
                            _ => self.profile.files[fi].entries.len(),
                        };
                        let count = new_entries.len();
                        // Insert entries
                        for (i, entry) in new_entries.drain(..).enumerate() {
                            self.profile.files[fi].entries.insert(insert_pos + i, entry);
                        }
                        self.profile.files[fi].dirty = true;
                        self.profile.files[fi].expanded = true;
                        self.profile.files[fi].recalculate_line_numbers();
                        self.rebuild_list();
                        self.message = Some(format!("Added {} entries", count));
                    }
                } else {
                    self.message = Some("Empty content, nothing added".into());
                }
            }
            Ok(None) => {
                self.message = Some("Cancelled".into());
            }
            Err(e) => {
                self.message = Some(format!("Editor error: {}", e));
            }
        }
        Ok(())
    }
}
