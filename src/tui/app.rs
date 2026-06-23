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
use crate::tui::state::{AppMode, FileMovingState, MoveState};

enum EditorRequest {
    None,
    EditFile(usize),                             // file index
    EditEntry(usize, usize),                     // file index, entry index
    AddEntry(usize),                             // target file index
    AddEntryWithTemplate(usize, Option<String>), // target file index, template content
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
    pub undo_stack: VecDeque<crate::tui::state::UndoSnapshot>,
    pub redo_stack: Vec<crate::tui::state::UndoSnapshot>,
    pub move_state: Option<MoveState>,
    pub search: Option<SearchState>,
    pub list_visible_height: usize,
    pub config: crate::model::Config,
    pub shell_key: String,
    pub pending_remove_fi: Option<usize>,
    pub pending_remove_group_pattern: Option<String>,
    pub text_input: Option<crate::tui::state::TextInputState>,
    pub pending_create_path: Option<(String, std::path::PathBuf)>,
    pub pending_delete_file_fi: Option<usize>,
    pub file_move_state: Option<FileMovingState>,
    pub expanded_snapshot: Option<crate::tui::state::ExpandedSnapshot>,
    pub detail_scroll_offset: u16,
    pub detail_page_size: u16,
    pub snippet_cursor: usize,
    pub snippet_scroll_offset: usize,
    pub snippets: Vec<crate::model::Snippet>,
    pub inline_edit: Option<crate::tui::state::InlineEditState>,
}

impl TuiApp {
    pub fn new(
        profile: ShellProfile,
        messages: &'static Messages,
        config: crate::model::Config,
        shell_key: String,
        snippets: Vec<crate::model::Snippet>,
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
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            move_state: None,
            search: None,
            list_visible_height: 20,
            snippet_cursor: 0,
            snippet_scroll_offset: 0,
            snippets,
            config,
            shell_key,
            pending_remove_fi: None,
            pending_remove_group_pattern: None,
            text_input: None,
            pending_create_path: None,
            pending_delete_file_fi: None,
            file_move_state: None,
            expanded_snapshot: None,
            detail_scroll_offset: 0,
            detail_page_size: 10,
            inline_edit: None,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Suppress resolver stderr warnings while the alternate screen is active;
        // otherwise a stray warning line (e.g. on reload) corrupts the rendered list.
        crate::config::path_resolver::set_quiet(true);
        let result = self.event_loop(&mut terminal);
        crate::config::path_resolver::set_quiet(false);

        // Always restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while !self.should_quit {
            // Keep the inline editor's horizontal viewport in sync with the cursor
            // at the current terminal width before drawing.
            if self.mode == AppMode::InlineEdit {
                let w = crate::tui::ui::inline_value_width(terminal.size()?.width, self);
                self.inline_clamp_scroll(w);
            }
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
                        if self.is_filter_mode() {
                            self.update_filter();
                        }
                    }
                    EditorRequest::EditEntry(fi, ei) => {
                        self.run_edit_entry(terminal, fi, ei)?;
                        if self.is_filter_mode() {
                            self.update_filter();
                        }
                    }
                    EditorRequest::AddEntry(fi) => {
                        self.run_add_entry(terminal, fi, None)?;
                        if self.is_filter_mode() {
                            self.update_filter();
                        }
                    }
                    EditorRequest::AddEntryWithTemplate(fi, template) => {
                        self.run_add_entry(terminal, fi, template.as_deref())?;
                        if self.is_filter_mode() {
                            self.update_filter();
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
            Action::NavigateDown => {
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
                    self.sync_move_target_cursor();
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
                    self.sync_move_target_cursor();
                } else {
                    self.selection.commit_range();
                    self.cursor = (self.cursor + half).min(max_idx);
                    self.clamp_scroll_offset();
                }
            }
            Action::Home => {
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
            Action::End => {
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
            Action::ToggleExpand => {
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
            }
            Action::EditExternal => {
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
            }
            Action::InlineInput(c) => self.inline_input_char(c),
            Action::InlineBackspace => self.inline_backspace(),
            Action::InlineDelete => self.inline_delete(),
            Action::InlineLeft => self.inline_cursor_left(),
            Action::InlineRight => self.inline_cursor_right(),
            Action::InlineHome => self.inline_cursor_home(),
            Action::InlineEnd => self.inline_cursor_end(),
            Action::Add => {
                // On a directory group header, `a` creates a new file in the group's
                // directory (prompt name → create → open $EDITOR) rather than adding
                // an entry. File headers (grouped or not) keep the add-entry flow.
                if let Some(ListItem::DirHeader(ti)) = self.visible_items.get(self.cursor) {
                    if let Some(crate::model::profile::TreeNode::Dir(g)) =
                        self.profile.tree.get(*ti)
                    {
                        let pattern = g.source_pattern.clone();
                        let dir = self.group_base_dir(g);
                        self.text_input = Some(crate::tui::state::TextInputState {
                            prompt: format!("New file name in {}", dir.display()),
                            value: String::new(),
                            cursor_pos: 0,
                            purpose: crate::tui::state::InputPurpose::NewFileInDir { dir, pattern },
                        });
                        self.mode = AppMode::TextInput;
                        self.message = None;
                    }
                    return Ok(EditorRequest::None);
                }
                if !self.is_current_file_writable() {
                    self.message = Some("File is read-only".into());
                    return Ok(EditorRequest::None);
                }
                if self.snippets.is_empty() {
                    let fi = self.current_file_index();
                    return Ok(EditorRequest::AddEntry(fi));
                }
                self.snippet_cursor = 0;
                self.snippet_scroll_offset = 0;
                self.previous_mode = Some(self.mode.clone());
                self.mode = AppMode::SelectingSnippet;
                return Ok(EditorRequest::None);
            }
            Action::SnippetUp => {
                if self.snippet_cursor > 0 {
                    self.snippet_cursor -= 1;
                    if self.snippet_cursor < self.snippet_scroll_offset {
                        self.snippet_scroll_offset = self.snippet_cursor;
                    }
                }
                return Ok(EditorRequest::None);
            }
            Action::SnippetDown => {
                if self.snippet_cursor < self.snippets.len().saturating_sub(1) {
                    self.snippet_cursor += 1;
                }
                return Ok(EditorRequest::None);
            }
            Action::SnippetSelect => {
                let fi = self.current_file_index();
                let template = self
                    .snippets
                    .get(self.snippet_cursor)
                    .and_then(|s| s.template.clone());
                let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                self.mode = if matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive) {
                    AppMode::FilterActive
                } else {
                    AppMode::Normal
                };
                return Ok(EditorRequest::AddEntryWithTemplate(fi, template));
            }
            Action::SnippetCancel => {
                let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
                self.mode = if matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive) {
                    AppMode::FilterActive
                } else {
                    AppMode::Normal
                };
                return Ok(EditorRequest::None);
            }
            Action::Delete => {
                if let Some(ListItem::DirHeader(ti)) = self.visible_items.get(self.cursor) {
                    let ti = *ti;
                    if let Some(crate::model::profile::TreeNode::Dir(g)) = self.profile.tree.get(ti)
                    {
                        let pattern = g.source_pattern.clone();
                        let file_count = g.file_indices.len();
                        self.pending_remove_group_pattern = Some(pattern.clone());
                        self.previous_mode = Some(self.mode.clone());
                        self.mode = AppMode::ConfirmRemoveGroup;
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
                        self.mode = AppMode::ConfirmDeleteFile;
                        self.message = Some(format!("Move file '{}' to trash? (y/n)", name));
                        return Ok(EditorRequest::None);
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
                if self.mode == AppMode::Moving {
                    // Already placing: switch to cut (requires writable sources).
                    self.set_placement_cut(true);
                } else {
                    // Cut = placement with source removal on drop. Sources stay
                    // visible (blue) until the drop; nothing is removed yet.
                    self.begin_placement(true);
                }
            }
            Action::Copy => {
                if self.mode == AppMode::Moving {
                    // Already placing: switch to copy.
                    self.set_placement_cut(false);
                } else {
                    // Copy = placement that keeps sources on drop.
                    self.begin_placement(false);
                }
            }
            Action::StartMove => {
                // `m` now reorders files only; entries use c/x placement.
                // DirHeader: move not supported on groups
                if matches!(
                    self.visible_items.get(self.cursor),
                    Some(ListItem::DirHeader(_))
                ) {
                    self.message = Some("Move is not supported on groups".into());
                    return Ok(EditorRequest::None);
                }
                // Check if cursor is on a FileHeader → enter file move mode
                if let Some(ListItem::FileHeader(fi)) = self.visible_items.get(self.cursor) {
                    let fi = *fi;
                    let is_inside_group = self.profile.tree.iter().any(|n|
                        matches!(n, crate::model::profile::TreeNode::Dir(g) if g.file_indices.contains(&fi))
                    );
                    if is_inside_group {
                        self.message = Some(
                            "Files inside a group are sorted alphabetically; move is not supported"
                                .into(),
                        );
                        return Ok(EditorRequest::None);
                    }
                    if self.profile.files.len() < 2 {
                        self.message = Some("Only one file, nothing to move".into());
                        return Ok(EditorRequest::None);
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
                    self.message =
                        Some("File move: ↑↓ to position, Enter to drop, Esc to cancel".into());
                    return Ok(EditorRequest::None);
                }

                // On an entry: `m` no longer moves entries — point at c/x.
                self.message = Some("Use c to copy or x to move entries".into());
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
                                    let removed_count =
                                        before.saturating_sub(self.profile.files.len());

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
                                self.message =
                                    Some(format!("Group '{}' removed from config", pattern));
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
                                    self.message =
                                        Some(format!("Moved to trash: {}", path.display()));
                                }
                                Err(e) => {
                                    self.message = Some(format!("Failed to move to trash: {}", e));
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
                                        self.message =
                                            Some("File doesn't exist. Create? (y/n)".into());
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
                                        self.message =
                                            Some(format!("Failed to create file: {}", e));
                                        return Ok(EditorRequest::None);
                                    }
                                    // Warn if the new name won't match the group's glob
                                    // pattern (it will be created but won't rejoin the
                                    // group on the next reload).
                                    let warn_unmatched = {
                                        let expanded =
                                            crate::config::path_resolver::expand_env_vars(
                                                &crate::config::path_resolver::expand_tilde(
                                                    &pattern,
                                                ),
                                            );
                                        (expanded.contains('*') || expanded.contains('?'))
                                            && glob::Pattern::new(&expanded)
                                                .map(|p| !p.matches_path(&path))
                                                .unwrap_or(false)
                                    };
                                    self.selection.clear();
                                    self.reload_profile()?;
                                    let new_fi =
                                        self.profile.files.iter().position(|f| f.path == path);
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
            }
            Action::Cancel => {
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
                        let in_filter =
                            matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive);
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
                        self.mode = if matches!(
                            return_mode,
                            AppMode::FilterInput | AppMode::FilterActive
                        ) {
                            AppMode::FilterActive
                        } else {
                            AppMode::Normal
                        };
                    }
                    AppMode::InlineEdit => self.inline_cancel(),
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
                if self.mode == AppMode::FilterActive {
                    // Re-open input to edit the existing filter query
                    self.mode = AppMode::FilterInput;
                } else {
                    // Capture expanded state before filter
                    let files: Vec<bool> = self.profile.files.iter().map(|f| f.expanded).collect();
                    let dirs: Vec<bool> = self
                        .profile
                        .tree
                        .iter()
                        .map(|n| match n {
                            crate::model::profile::TreeNode::Dir(g) => g.expanded,
                            _ => false,
                        })
                        .collect();
                    self.expanded_snapshot =
                        Some(crate::tui::state::ExpandedSnapshot { files, dirs });
                    self.search = Some(SearchState::new());
                    self.mode = AppMode::FilterInput;
                    self.message = None;
                }
            }
            Action::SearchInput(c) => {
                if let Some(ref mut search) = self.search {
                    search.input_char(c);
                }
                self.update_filter();
            }
            Action::SearchBackspace => {
                if let Some(ref mut search) = self.search {
                    search.backspace();
                }
                self.update_filter();
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
                    prompt: "Add file, group, glob, or $VAR to config".into(),
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

    pub fn rebuild_list(&mut self) {
        if let Some(ref search) = self.search {
            if !search.query.is_empty() {
                let matched_files = search.matched_file_indices();
                let matched_entries = search.matched_entry_indices();
                // Expand files that have matches, collapse others
                for (i, file) in self.profile.files.iter_mut().enumerate() {
                    file.expanded = matched_files.contains(&i);
                }
                // Expand DirGroups that contain any matched file
                for n in &mut self.profile.tree {
                    if let crate::model::profile::TreeNode::Dir(g) = n {
                        g.expanded = g.file_indices.iter().any(|fi| matched_files.contains(fi));
                    }
                }
                self.visible_items = self
                    .profile
                    .build_visible_list_filtered(&matched_files, &matched_entries);
                return;
            }
        }

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
    /// Returns the tree index of the `Dir` group containing file `fi`, if any.
    fn group_index_of_file(&self, fi: usize) -> Option<usize> {
        self.profile.tree.iter().position(|n| {
            matches!(n, crate::model::profile::TreeNode::Dir(g) if g.file_indices.contains(&fi))
        })
    }

    /// Base directory a new file should be created in for a directory group.
    /// Prefers an existing member's parent; falls back to resolving the group's
    /// source pattern (directory as-is, or the parent of a glob pattern).
    fn group_base_dir(&self, g: &crate::model::profile::DirGroup) -> std::path::PathBuf {
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

    fn current_file_index(&self) -> usize {
        match self.visible_items.get(self.cursor) {
            Some(ListItem::FileHeader(fi)) => *fi,
            Some(ListItem::Entry(fi, _)) => *fi,
            Some(ListItem::DirHeader(_)) => 0, // DirHeader doesn't map to a single file; default to 0
            None => 0,
        }
    }

    /// Append a config pattern (plain file, glob, directory, or var-bearing
    /// form) and load the file(s) it resolves to. Mirrors `load_shell_profile`
    /// so the `a` key supports every format the config file accepts.
    fn add_pattern_to_config_and_profile(&mut self, raw_path: String) -> anyhow::Result<()> {
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

    /// Check if the file under the cursor is writable
    fn is_current_file_writable(&self) -> bool {
        let fi = self.current_file_index();
        fi < self.profile.files.len() && self.profile.files[fi].writable
    }

    /// Returns true when either filter mode is active.
    pub fn is_filter_mode(&self) -> bool {
        matches!(self.mode, AppMode::FilterInput | AppMode::FilterActive)
    }

    /// Update fuzzy-match results and rebuild the filtered list.
    fn update_filter(&mut self) {
        if let Some(ref mut search) = self.search {
            search.update_matches(&self.profile);
        }
        self.rebuild_list();
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

    /// Drop the placement (c/x): clone sources and insert at the target. For a cut
    /// (`ms.cut`), also remove the sources, so the net effect is a move; for a copy,
    /// the sources are left in place. Single drop → return to the resting mode.
    fn execute_drop(&mut self) {
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
            self.cursor = ms
                .insertion_cursor
                .min(self.visible_items.len().saturating_sub(1));
            self.clamp_scroll_offset();
            self.message = Some(if ms.cut { "Moved" } else { "Copied" }.into());
        }
    }

    /// Begin a copy/cut placement: record the selected/cursor entries as sources
    /// (rendered blue), snap the target to a writable position, and enter Moving
    /// mode. Nothing is mutated until the drop (`execute_drop`).
    fn begin_placement(&mut self, cut: bool) {
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
    fn set_placement_cut(&mut self, cut: bool) {
        let blocked = cut
            && self.move_state.as_ref().is_some_and(|ms| {
                ms.source_items
                    .iter()
                    .any(|&(fi, _)| !self.profile.files[fi].writable)
            });
        if blocked {
            return; // keep copy; the unchanged "Copy:" hint signals it can't cut
        }
        if let Some(ms) = self.move_state.as_mut() {
            ms.cut = cut;
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
            let mut new_expanded = crate::tui::state::ExpandedSnapshot {
                files: fms.saved_expanded.files.clone(),
                dirs: fms.saved_expanded.dirs.clone(),
            };
            let removed = new_expanded.files.remove(fms.original_fi);
            new_expanded.files.insert(target_fi, removed);
            self.restore_expanded(&new_expanded);

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
    fn restore_expanded(&mut self, snap: &crate::tui::state::ExpandedSnapshot) {
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

    fn reload_profile(&mut self) -> anyhow::Result<()> {
        use crate::model::profile::{ProfileFile, TreeNode};
        use std::collections::{HashMap, HashSet};

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
            self.message = Some("Undo cleared (file set changed)".into());
        }

        self.profile = new_profile;
        self.visible_items = self.profile.build_visible_list();
        if self.cursor >= self.visible_items.len() {
            self.cursor = self.visible_items.len().saturating_sub(1);
        }
        Ok(())
    }

    /// Check if a visible-list position belongs to a blocked file
    fn is_position_blocked(
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
    fn sync_move_target_cursor(&mut self) {
        if let Some(ic) = self.move_state.as_ref().map(|ms| ms.insertion_cursor) {
            self.cursor = ic;
            self.clamp_scroll_offset();
        }
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
                self.apply_edited_value(fi, ei, &value, &new_content);
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

    /// Apply an edited entry value (from the external editor or the inline editor):
    /// push an undo snapshot, re-parse, and replace the entry. Empty content deletes
    /// the entry. No-op (with a "No changes" message) when `new_content == old_value`.
    fn apply_edited_value(&mut self, fi: usize, ei: usize, old_value: &str, new_content: &str) {
        if new_content == old_value {
            self.message = Some("No changes".into());
            return;
        }
        let snapshot = crate::tui::operations::take_snapshot(&self.profile);
        crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
        let parser = crate::parser::get_parser(self.profile.shell_type);
        let parsed = parser.parse(new_content);
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
    }

    // --- Inline editor (single-line entries) -------------------------------

    /// Enter inline-edit mode for a single-line entry, seeding the buffer with its
    /// raw value and placing the cursor at the end.
    fn begin_inline_edit(&mut self, fi: usize, ei: usize) {
        let buffer = self.profile.files[fi].entries[ei].value.clone();
        let cursor = buffer.chars().count();
        self.inline_edit = Some(crate::tui::state::InlineEditState {
            fi,
            ei,
            buffer,
            cursor,
            scroll: 0,
        });
        self.mode = AppMode::InlineEdit;
        self.message = None;
    }

    /// Keep the inline editor's horizontal viewport in sync with the cursor for a
    /// `width`-wide VALUE column. Called from the event loop before each draw.
    pub fn inline_clamp_scroll(&mut self, width: usize) {
        if let Some(ref mut e) = self.inline_edit {
            let len = e.buffer.chars().count();
            e.scroll = clamp_inline_scroll(e.scroll, e.cursor.min(len), len, width);
        }
    }

    fn inline_input_char(&mut self, c: char) {
        if let Some(ref mut e) = self.inline_edit {
            let byte = e
                .buffer
                .char_indices()
                .nth(e.cursor)
                .map(|(b, _)| b)
                .unwrap_or(e.buffer.len());
            e.buffer.insert(byte, c);
            e.cursor += 1;
            self.message = None;
        }
    }

    fn inline_backspace(&mut self) {
        if let Some(ref mut e) = self.inline_edit {
            if e.cursor > 0 {
                let prev = e
                    .buffer
                    .char_indices()
                    .nth(e.cursor - 1)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                e.buffer.remove(prev);
                e.cursor -= 1;
                self.message = None;
            }
        }
    }

    fn inline_delete(&mut self) {
        if let Some(ref mut e) = self.inline_edit {
            let len = e.buffer.chars().count();
            if e.cursor < len {
                let at = e
                    .buffer
                    .char_indices()
                    .nth(e.cursor)
                    .map(|(b, _)| b)
                    .unwrap_or(e.buffer.len());
                e.buffer.remove(at);
                self.message = None;
            }
        }
    }

    fn inline_cursor_left(&mut self) {
        if let Some(ref mut e) = self.inline_edit {
            e.cursor = e.cursor.saturating_sub(1);
        }
    }

    fn inline_cursor_right(&mut self) {
        if let Some(ref mut e) = self.inline_edit {
            let len = e.buffer.chars().count();
            if e.cursor < len {
                e.cursor += 1;
            }
        }
    }

    fn inline_cursor_home(&mut self) {
        if let Some(ref mut e) = self.inline_edit {
            e.cursor = 0;
        }
    }

    fn inline_cursor_end(&mut self) {
        if let Some(ref mut e) = self.inline_edit {
            e.cursor = e.buffer.chars().count();
        }
    }

    fn inline_cancel(&mut self) {
        self.inline_edit = None;
        self.mode = if self.is_filtering() {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
        self.message = None;
    }

    /// Commit the inline edit: re-parse and replace via the shared apply path, then
    /// return to the resting mode. Refreshes the filter when one is active.
    fn inline_commit(&mut self) {
        let Some(e) = self.inline_edit.take() else {
            return;
        };
        let old_value = self.profile.files[e.fi].entries[e.ei].value.clone();
        self.apply_edited_value(e.fi, e.ei, &old_value, &e.buffer);
        self.mode = if self.is_filtering() {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
        if self.is_filtering() {
            self.update_filter();
        }
    }

    /// True when a filter query is active.
    fn is_filtering(&self) -> bool {
        self.search.as_ref().is_some_and(|s| !s.query.is_empty())
    }

    /// Add a new entry: open empty (or template) temp file in $EDITOR, parse result, insert
    fn run_add_entry(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
        template: Option<&str>,
    ) -> Result<()> {
        let suffix = match self.profile.shell_type {
            crate::model::ShellType::PowerShell => ".ps1",
            _ => ".sh",
        };

        Self::suspend_tui(terminal)?;
        let initial_content = template.unwrap_or("");
        let result = crate::tui::editor::edit_temp_content(initial_content, suffix);
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

/// Slide a `width`-wide horizontal viewport so the cursor stays visible, scrolling
/// by the minimum needed. The virtual length includes the trailing cursor slot, so
/// no blank gap is left past the end after the buffer shrinks. (Ported from confy.)
fn clamp_inline_scroll(scroll: usize, cursor: usize, len: usize, width: usize) -> usize {
    let w = width.max(1);
    let cur = cursor.min(len);
    let mut s = scroll;
    if cur < s {
        s = cur;
    } else if cur >= s + w {
        s = cur + 1 - w;
    }
    s.min((len + 1).saturating_sub(w))
}

#[cfg(test)]
mod add_pattern_tests {
    use super::*;
    use crate::model::profile::{load_shell_profile, TreeNode};
    use crate::model::{Config, FilesConfig, ShellType};

    fn app_for(td: &tempfile::TempDir, paths: Vec<String>) -> TuiApp {
        let mut config = Config {
            source_path: td.path().join("config.toml"),
            ..Config::default()
        };
        config.files.insert("zsh".into(), FilesConfig { paths });
        let profile = load_shell_profile(&config, ShellType::Zsh).unwrap();
        TuiApp::new(
            profile,
            crate::i18n::messages(),
            config,
            "zsh".into(),
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn add_glob_pattern_creates_dir_group() {
        let td = tempfile::tempdir().unwrap();
        let sub = td.path().join("zshrc.d");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.sh"), "alias a=1\n").unwrap();
        std::fs::write(sub.join("b.sh"), "alias b=2\n").unwrap();

        let mut app = app_for(&td, vec![]);
        app.add_pattern_to_config_and_profile(format!("{}/*.sh", sub.display()))
            .unwrap();

        assert_eq!(app.profile.files.len(), 2);
        match app.profile.tree.last().unwrap() {
            TreeNode::Dir(g) => {
                assert_eq!(g.file_indices.len(), 2);
                assert!(g.expanded, "new group should be expanded");
            }
            TreeNode::File(_) => panic!("expected Dir group, got File node"),
        }
        // Pattern persisted to config on disk.
        let saved = std::fs::read_to_string(td.path().join("config.toml")).unwrap();
        assert!(saved.contains("*.sh"));
    }

    #[test]
    fn add_directory_path_expands_to_files() {
        let td = tempfile::tempdir().unwrap();
        let sub = td.path().join("conf.d");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("x.sh"), "alias x=1\n").unwrap();

        let mut app = app_for(&td, vec![]);
        app.add_pattern_to_config_and_profile(sub.to_string_lossy().to_string())
            .unwrap();

        assert_eq!(app.profile.files.len(), 1);
        assert!(matches!(app.profile.tree.last().unwrap(), TreeNode::Dir(_)));
    }

    #[test]
    fn add_plain_file_creates_file_node() {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("solo.sh");
        std::fs::write(&f, "alias s=1\n").unwrap();

        let mut app = app_for(&td, vec![]);
        app.add_pattern_to_config_and_profile(f.to_string_lossy().to_string())
            .unwrap();

        assert_eq!(app.profile.files.len(), 1);
        assert!(matches!(
            app.profile.tree.last().unwrap(),
            TreeNode::File(0)
        ));
        assert!(app.profile.files[0].expanded);
    }

    #[test]
    fn add_env_var_pattern_resolves() {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("v.sh");
        std::fs::write(&f, "alias v=1\n").unwrap();
        // SAFETY: single-threaded test; unique var name avoids cross-test races.
        unsafe { std::env::set_var("WENV_TEST_DIR", td.path()) };

        let mut app = app_for(&td, vec![]);
        app.add_pattern_to_config_and_profile("$WENV_TEST_DIR/v.sh".into())
            .unwrap();

        assert_eq!(app.profile.files.len(), 1);
        assert_eq!(app.profile.files[0].path, f);
        unsafe { std::env::remove_var("WENV_TEST_DIR") };
    }
}

#[cfg(test)]
mod inline_edit_tests {
    use super::*;
    use crate::model::profile::load_shell_profile;
    use crate::model::{Config, FilesConfig, ShellType};
    use crate::tui::keys::Action;
    use crate::tui::state::AppMode;

    /// Build an app over one zsh file with `content`, file expanded, cursor on the
    /// first entry.
    fn app_with(content: &str) -> (tempfile::TempDir, TuiApp) {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("a.zsh");
        std::fs::write(&f, content).unwrap();
        let mut config = Config {
            source_path: td.path().join("config.toml"),
            ..Config::default()
        };
        config.files.insert(
            "zsh".into(),
            FilesConfig {
                paths: vec![f.to_string_lossy().to_string()],
            },
        );
        let profile = load_shell_profile(&config, ShellType::Zsh).unwrap();
        let mut app = TuiApp::new(
            profile,
            crate::i18n::messages(),
            config,
            "zsh".into(),
            vec![],
        )
        .unwrap();
        app.profile.files[0].expanded = true;
        app.rebuild_list();
        // cursor 0 is the FileHeader; move to the first entry.
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::Entry(_, _)))
            .unwrap();
        (td, app)
    }

    #[test]
    fn edit_single_line_enters_inline_mode() {
        let (_td, mut app) = app_with("alias ll='ls -la'\n");
        let req = app.handle_action(Action::Edit).unwrap();
        assert!(matches!(req, EditorRequest::None));
        assert_eq!(app.mode, AppMode::InlineEdit);
        let e = app.inline_edit.as_ref().unwrap();
        assert_eq!(e.buffer, "alias ll='ls -la'");
        assert_eq!(e.cursor, e.buffer.chars().count(), "cursor seeded at end");
    }

    #[test]
    fn edit_multiline_entry_routes_external() {
        // A leading comment merges into the alias entry, making value multi-line.
        let (_td, mut app) = app_with("# note\nalias ll='ls -la'\n");
        let req = app.handle_action(Action::Edit).unwrap();
        assert!(matches!(req, EditorRequest::EditEntry(0, 0)));
        assert_ne!(app.mode, AppMode::InlineEdit);
    }

    #[test]
    fn edit_external_forces_external_for_single_line() {
        let (_td, mut app) = app_with("alias ll='ls -la'\n");
        let req = app.handle_action(Action::EditExternal).unwrap();
        assert!(matches!(req, EditorRequest::EditEntry(0, 0)));
        assert_ne!(app.mode, AppMode::InlineEdit);
    }

    #[test]
    fn buffer_edit_ops_are_utf8_safe() {
        let (_td, mut app) = app_with("alias café='x'\n");
        app.handle_action(Action::Edit).unwrap();
        // Cursor at end; move home, then right past the multibyte 'é' region.
        app.inline_cursor_home();
        assert_eq!(app.inline_edit.as_ref().unwrap().cursor, 0);
        for _ in 0..9 {
            app.inline_cursor_right();
        } // after "alias caf"
        app.inline_input_char('X');
        app.inline_cursor_end();
        app.inline_backspace(); // remove trailing '
        let buf = &app.inline_edit.as_ref().unwrap().buffer;
        assert!(buf.starts_with("alias cafX"), "got {buf}");
    }

    #[test]
    fn commit_reparses_and_updates_entry() {
        let (_td, mut app) = app_with("alias ll='ls -la'\n");
        app.handle_action(Action::Edit).unwrap();
        // Replace the whole buffer with a new value.
        app.inline_edit.as_mut().unwrap().buffer = "alias ll='ls -lah'".into();
        app.inline_commit();
        assert_ne!(app.mode, AppMode::InlineEdit);
        assert_eq!(app.profile.files[0].entries[0].value, "alias ll='ls -lah'");
        assert!(app.profile.files[0].dirty);
    }

    #[test]
    fn empty_commit_clears_value_like_external_editor() {
        // Parity with the external editor: re-parsing an empty buffer yields a
        // single empty Code entry (the parser never returns zero entries), so the
        // alias is replaced, not the row kept. File is marked dirty.
        let (_td, mut app) = app_with("alias ll='ls -la'\n");
        app.handle_action(Action::Edit).unwrap();
        app.inline_edit.as_mut().unwrap().buffer = String::new();
        app.inline_commit();
        assert_eq!(app.profile.files[0].entries.len(), 1);
        assert_eq!(app.profile.files[0].entries[0].value, "");
        assert!(app.profile.files[0].dirty);
    }

    #[test]
    fn cancel_discards_changes() {
        let (_td, mut app) = app_with("alias ll='ls -la'\n");
        app.handle_action(Action::Edit).unwrap();
        app.inline_edit.as_mut().unwrap().buffer = "garbage".into();
        app.inline_cancel();
        assert_ne!(app.mode, AppMode::InlineEdit);
        assert!(app.inline_edit.is_none());
        assert_eq!(app.profile.files[0].entries[0].value, "alias ll='ls -la'");
        assert!(
            !app.profile.files[0].dirty,
            "cancel must not dirty the file"
        );
    }

    #[test]
    fn clamp_scroll_keeps_cursor_in_viewport() {
        // width 5: cursor at 0 -> scroll 0; cursor at 9 in len 10 -> scroll 5.
        assert_eq!(clamp_inline_scroll(0, 0, 10, 5), 0);
        assert_eq!(clamp_inline_scroll(0, 9, 10, 5), 5);
        // Cursor scrolled left below the window pulls the viewport back.
        assert_eq!(clamp_inline_scroll(5, 2, 10, 5), 2);
        // No blank gap past the end after the buffer shrinks.
        assert_eq!(clamp_inline_scroll(8, 3, 3, 5), 0);
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    use crate::model::profile::load_shell_profile;
    use crate::model::{Config, FilesConfig, ShellType};
    use crate::tui::keys::{map_key, Action};
    use crate::tui::state::AppMode;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// App over one zsh file with three entries, expanded, cursor on the first entry.
    fn app3() -> (tempfile::TempDir, TuiApp) {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("a.zsh");
        std::fs::write(&f, "alias a='1'\nalias b='2'\nexport C=3\n").unwrap();
        let mut config = Config {
            source_path: td.path().join("config.toml"),
            ..Config::default()
        };
        config.files.insert(
            "zsh".into(),
            FilesConfig {
                paths: vec![f.to_string_lossy().to_string()],
            },
        );
        let profile = load_shell_profile(&config, ShellType::Zsh).unwrap();
        let mut app = TuiApp::new(
            profile,
            crate::i18n::messages(),
            config,
            "zsh".into(),
            vec![],
        )
        .unwrap();
        app.profile.files[0].expanded = true;
        app.rebuild_list();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::Entry(_, _)))
            .unwrap();
        (td, app)
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn copy_enters_placement_then_drops_clone_keeping_source() {
        let (_td, mut app) = app3();
        // cursor on entry 0 (alias a)
        app.handle_action(Action::Copy).unwrap();
        assert_eq!(app.mode, AppMode::Moving);
        let ms = app.move_state.as_ref().unwrap();
        assert!(!ms.cut);
        assert_eq!(ms.source_items, vec![(0, 0)]);
        // Drop after the last entry (visible index 3 = export C).
        app.move_state.as_mut().unwrap().insertion_cursor = 3;
        app.handle_action(Action::Confirm).unwrap();
        assert_ne!(app.mode, AppMode::Moving);
        // Source kept + clone appended → 4 entries; clone equals source value.
        assert_eq!(app.profile.files[0].entries.len(), 4);
        assert_eq!(app.profile.files[0].entries[0].value, "alias a='1'");
        assert_eq!(app.profile.files[0].entries[3].value, "alias a='1'");
        assert!(app.profile.files[0].dirty);
    }

    #[test]
    fn cut_drops_and_removes_source() {
        let (_td, mut app) = app3();
        app.handle_action(Action::Cut).unwrap();
        assert_eq!(app.mode, AppMode::Moving);
        assert!(app.move_state.as_ref().unwrap().cut);
        // Source still present during placement (not removed yet).
        assert_eq!(app.profile.files[0].entries.len(), 3);
        // Drop after the last entry.
        app.move_state.as_mut().unwrap().insertion_cursor = 3;
        app.handle_action(Action::Confirm).unwrap();
        // Net move: a removed from front, reinserted at end → [b, C, a].
        let vals: Vec<_> = app.profile.files[0]
            .entries
            .iter()
            .map(|e| e.value.as_str())
            .collect();
        assert_eq!(vals, vec!["alias b='2'", "export C=3", "alias a='1'"]);
    }

    #[test]
    fn cancel_placement_makes_no_change() {
        let (_td, mut app) = app3();
        app.handle_action(Action::Copy).unwrap();
        app.handle_action(Action::Cancel).unwrap();
        assert_ne!(app.mode, AppMode::Moving);
        assert!(app.move_state.is_none());
        assert_eq!(app.profile.files[0].entries.len(), 3);
        assert!(!app.profile.files[0].dirty);
    }

    #[test]
    fn c_x_toggle_placement_mode_while_placing() {
        let (_td, mut app) = app3();
        app.handle_action(Action::Copy).unwrap();
        assert!(!app.move_state.as_ref().unwrap().cut, "starts as copy");
        // x switches to cut (move) without leaving placement.
        app.handle_action(Action::Cut).unwrap();
        assert_eq!(app.mode, AppMode::Moving);
        assert!(app.move_state.as_ref().unwrap().cut);
        // c switches back to copy.
        app.handle_action(Action::Copy).unwrap();
        assert_eq!(app.mode, AppMode::Moving);
        assert!(!app.move_state.as_ref().unwrap().cut);
    }

    #[test]
    fn toggle_to_cut_blocked_when_source_read_only() {
        let (_td, mut app) = app3();
        app.handle_action(Action::Copy).unwrap(); // copy allows read-only sources
        app.profile.files[0].writable = false;
        app.handle_action(Action::Cut).unwrap(); // attempt switch to cut
        assert!(
            !app.move_state.as_ref().unwrap().cut,
            "cut blocked: source is read-only, stays copy"
        );
    }

    #[test]
    fn placing_keymap_maps_c_and_x() {
        use crate::tui::keys::map_key;
        assert!(matches!(map_key(&AppMode::Moving, key('c')), Action::Copy));
        assert!(matches!(map_key(&AppMode::Moving, key('x')), Action::Cut));
    }

    #[test]
    fn m_on_entry_does_not_enter_moving() {
        let (_td, mut app) = app3();
        app.handle_action(Action::StartMove).unwrap();
        assert_ne!(app.mode, AppMode::Moving);
        assert!(app.move_state.is_none());
    }

    #[test]
    fn keymap_swaps_and_placement_keys() {
        // a = insert entry (Add), n = new file path (AddFile)
        assert!(matches!(map_key(&AppMode::Normal, key('a')), Action::Add));
        assert!(matches!(
            map_key(&AppMode::Normal, key('n')),
            Action::AddFile
        ));
        // c/x placement; v is no longer paste in normal mode.
        assert!(matches!(map_key(&AppMode::Normal, key('c')), Action::Copy));
        assert!(matches!(map_key(&AppMode::Normal, key('x')), Action::Cut));
        assert!(matches!(map_key(&AppMode::Normal, key('v')), Action::Noop));
        // In Moving mode, v and Enter both confirm the drop.
        assert!(matches!(
            map_key(&AppMode::Moving, key('v')),
            Action::Confirm
        ));
        assert!(matches!(
            map_key(
                &AppMode::Moving,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            Action::Confirm
        ));
        // Delete key is an alias for `d` in normal mode.
        assert!(matches!(
            map_key(
                &AppMode::Normal,
                KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)
            ),
            Action::Delete
        ));
        assert!(matches!(
            map_key(&AppMode::Normal, key('d')),
            Action::Delete
        ));
    }

    /// App over a directory group (`<dir>/*.sh`) with two bash files, group and
    /// files expanded.
    fn app_group() -> (tempfile::TempDir, TuiApp) {
        let td = tempfile::tempdir().unwrap();
        let dir = td.path().join("grp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("one.sh"), "alias a='1'\n").unwrap();
        std::fs::write(dir.join("two.sh"), "alias b='2'\n").unwrap();
        let pattern = format!("{}/*.sh", dir.display());
        let mut config = Config {
            source_path: td.path().join("config.toml"),
            ..Config::default()
        };
        config.files.insert(
            "bash".into(),
            FilesConfig {
                paths: vec![pattern],
            },
        );
        let profile = load_shell_profile(&config, ShellType::Bash).unwrap();
        let mut app = TuiApp::new(
            profile,
            crate::i18n::messages(),
            config,
            "bash".into(),
            vec![],
        )
        .unwrap();
        for n in &mut app.profile.tree {
            if let crate::model::profile::TreeNode::Dir(g) = n {
                g.expanded = true;
            }
        }
        for f in &mut app.profile.files {
            f.expanded = true;
        }
        app.rebuild_list();
        (td, app)
    }

    #[test]
    fn delete_on_grouped_file_enters_trash_confirm() {
        let (_td, mut app) = app_group();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::FileHeader(_)))
            .unwrap();
        let fi = match app.visible_items[app.cursor] {
            ListItem::FileHeader(fi) => fi,
            _ => unreachable!(),
        };
        app.handle_action(Action::Delete).unwrap();
        assert_eq!(app.mode, AppMode::ConfirmDeleteFile);
        assert_eq!(app.pending_delete_file_fi, Some(fi));
    }

    #[test]
    fn delete_on_standalone_file_uses_config_removal() {
        // app3 is a single plain-file config (not a group): `d` removes from config.
        let (_td, mut app) = app3();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::FileHeader(_)))
            .unwrap();
        app.handle_action(Action::Delete).unwrap();
        assert_eq!(app.mode, AppMode::ConfirmRemoveFile);
        assert!(app.pending_delete_file_fi.is_none());
    }

    #[test]
    fn add_on_dir_header_opens_new_file_prompt() {
        let (_td, mut app) = app_group();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::DirHeader(_)))
            .unwrap();
        app.handle_action(Action::Add).unwrap();
        assert_eq!(app.mode, AppMode::TextInput);
        match &app.text_input.as_ref().unwrap().purpose {
            crate::tui::state::InputPurpose::NewFileInDir { dir, .. } => {
                assert!(dir.ends_with("grp"));
            }
            _ => panic!("expected NewFileInDir purpose"),
        }
    }

    #[test]
    fn new_file_in_dir_creates_file_and_requests_edit() {
        let (_td, mut app) = app_group();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::DirHeader(_)))
            .unwrap();
        app.handle_action(Action::Add).unwrap();
        for c in "three.sh".chars() {
            app.handle_action(Action::TextInputChar(c)).unwrap();
        }
        let req = app.handle_action(Action::Confirm).unwrap();
        let path = match req {
            EditorRequest::EditFile(fi) => app.profile.files[fi].path.clone(),
            _ => panic!("expected EditFile request"),
        };
        assert!(path.exists(), "new file should be created on disk");
        assert!(path.ends_with("three.sh"));
        // It matches the group's glob, so it rejoined the group on reload.
        assert!(app.profile.files.iter().any(|f| f.path == path));
    }

    #[test]
    fn new_file_in_dir_rejects_existing_name() {
        let (_td, mut app) = app_group();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::DirHeader(_)))
            .unwrap();
        app.handle_action(Action::Add).unwrap();
        for c in "one.sh".chars() {
            app.handle_action(Action::TextInputChar(c)).unwrap();
        }
        let before = app.profile.files.len();
        let req = app.handle_action(Action::Confirm).unwrap();
        assert!(matches!(req, EditorRequest::None));
        assert_eq!(app.profile.files.len(), before);
        assert_eq!(app.message.as_deref(), Some("File already exists"));
    }
}
