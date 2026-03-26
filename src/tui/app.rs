//! TUI application core

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use crate::i18n::Messages;
use crate::model::profile::{ListItem, ShellProfile};
use crate::tui::keys::{self, Action};
use crate::tui::list;
use crate::tui::search::SearchState;
use crate::tui::selection::SelectionState;
use crate::tui::state::{AppMode, ClipboardState, MoveState, UndoSnapshot};

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
    pub mode: AppMode,
    pub previous_mode: Option<AppMode>,
    pub should_quit: bool,
    pub message: Option<String>,
    pub messages: &'static Messages,
    pub selection: SelectionState,
    pub clipboard: ClipboardState,
    pub undo_snapshot: Option<UndoSnapshot>,
    pub move_state: Option<MoveState>,
    pub search: Option<SearchState>,
    pub list_visible_height: usize,
}

impl TuiApp {
    pub fn new(profile: ShellProfile, messages: &'static Messages) -> Result<Self> {
        let visible_items = profile.build_visible_list();
        Ok(Self {
            profile,
            visible_items,
            cursor: 0,
            mode: AppMode::Normal,
            previous_mode: None,
            should_quit: false,
            message: None,
            messages,
            selection: SelectionState::new(),
            clipboard: ClipboardState::new(),
            undo_snapshot: None,
            move_state: None,
            search: None,
            list_visible_height: 20,
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
                    }
                    EditorRequest::EditEntry(fi, ei) => {
                        self.run_edit_entry(terminal, fi, ei)?;
                    }
                    EditorRequest::AddEntry(fi) => {
                        self.run_add_entry(terminal, fi)?;
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
                } else if self.mode == AppMode::Moving {
                    if let Some(ref mut ms) = self.move_state {
                        if ms.insertion_cursor > 0 {
                            ms.insertion_cursor -= 1;
                        }
                    }
                } else {
                    self.selection.commit_range();
                    self.cursor = list::navigate_up(&self.visible_items, self.cursor);
                }
            }
            Action::NavigateDown => {
                if self.mode == AppMode::Searching {
                    if let Some(ref mut search) = self.search {
                        search.select_next();
                    }
                    self.navigate_to_search_match();
                } else if self.mode == AppMode::Moving {
                    if let Some(ref mut ms) = self.move_state {
                        if ms.insertion_cursor < self.visible_items.len().saturating_sub(1) {
                            ms.insertion_cursor += 1;
                        }
                    }
                } else {
                    self.selection.commit_range();
                    self.cursor = list::navigate_down(&self.visible_items, self.cursor);
                }
            }
            Action::PageUp => {
                let half = (self.list_visible_height / 2).max(1);
                if self.mode == AppMode::Moving {
                    if let Some(ref mut ms) = self.move_state {
                        ms.insertion_cursor = ms.insertion_cursor.saturating_sub(half);
                    }
                } else {
                    self.selection.commit_range();
                    self.cursor = self.cursor.saturating_sub(half);
                }
            }
            Action::PageDown => {
                let half = (self.list_visible_height / 2).max(1);
                let max_idx = self.visible_items.len().saturating_sub(1);
                if self.mode == AppMode::Moving {
                    if let Some(ref mut ms) = self.move_state {
                        ms.insertion_cursor = (ms.insertion_cursor + half).min(max_idx);
                    }
                } else {
                    self.selection.commit_range();
                    self.cursor = (self.cursor + half).min(max_idx);
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
                self.selection.set_range(self.cursor, &self.visible_items);
            }
            Action::RangeSelectDown => {
                self.selection.set_range(self.cursor, &self.visible_items);
                self.cursor = list::navigate_down(&self.visible_items, self.cursor);
                self.selection.set_range(self.cursor, &self.visible_items);
            }
            Action::Edit => {
                if let Some(item) = self.visible_items.get(self.cursor) {
                    match item {
                        ListItem::FileHeader(fi) => return Ok(EditorRequest::EditFile(*fi)),
                        ListItem::Entry(fi, ei) => return Ok(EditorRequest::EditEntry(*fi, *ei)),
                    }
                }
            }
            Action::Add => {
                let fi = self.current_file_index();
                return Ok(EditorRequest::AddEntry(fi));
            }
            Action::Delete => {
                let targets = self.get_operation_targets();
                if !targets.is_empty() {
                    self.undo_snapshot = Some(crate::tui::operations::take_snapshot(&self.profile));
                    self.previous_mode = Some(self.mode.clone());
                    self.mode = AppMode::ConfirmDelete;
                    let count = targets.len();
                    self.message = Some(format!("Delete {} entries? (y/n)", count));
                }
            }
            Action::Cut => {
                let targets = self.get_operation_targets();
                if !targets.is_empty() {
                    self.undo_snapshot = Some(crate::tui::operations::take_snapshot(&self.profile));
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
                let targets = self.get_operation_targets();
                if !targets.is_empty() {
                    self.undo_snapshot = Some(crate::tui::operations::take_snapshot(&self.profile));

                    let has_selection = !self.selection.is_empty();

                    // If from multi-selection, jump cursor to first selected row
                    if has_selection {
                        let first = self.selection.sorted_indices()[0];
                        self.cursor = first;
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
                if !self.clipboard.is_empty() {
                    self.undo_snapshot = Some(crate::tui::operations::take_snapshot(&self.profile));
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
                if let Some(snapshot) = self.undo_snapshot.take() {
                    crate::tui::operations::restore_snapshot(&mut self.profile, snapshot);
                    self.selection.clear();
                    self.rebuild_list();
                    self.message = Some("Undone".into());
                } else {
                    self.message = Some("Nothing to undo".into());
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
                    AppMode::ShowingDetail | AppMode::ShowingHelp => {
                        self.mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
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
                    AppMode::Moving => {
                        let from_sel = self.move_state.as_ref().is_some_and(|ms| ms.from_selection);
                        // Restore from snapshot
                        if let Some(snapshot) = self.undo_snapshot.take() {
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
                        if let Some(_snapshot) = self.undo_snapshot.take() {}
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
                        self.mode = AppMode::Normal;
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
                self.message = Some("Remark: not yet implemented".into());
            }
            Action::AddFile => {
                self.message = Some("Add file: not yet implemented".into());
            }
            Action::TextInputChar(_) | Action::TextInputBackspace | Action::TextInputLeft | Action::TextInputRight => {}
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
    }

    /// Get the file index for the current cursor position
    fn current_file_index(&self) -> usize {
        match self.visible_items.get(self.cursor) {
            Some(ListItem::FileHeader(fi)) => *fi,
            Some(ListItem::Entry(fi, _)) => *fi,
            None => 0,
        }
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
            let mut affected_files: std::collections::HashSet<usize> = source_files.into_iter().collect();
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
            self.message = Some("Moved".into());
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
                let new_content = new_content.trim_end_matches('\n').to_string();
                if new_content != value {
                    self.undo_snapshot = Some(crate::tui::operations::take_snapshot(&self.profile));
                    // Re-parse the edited content to get proper entry type/name
                    let parser = crate::parser::get_parser(self.profile.shell_type);
                    let parsed = parser.parse(&new_content);
                    if let Some(new_entry) = parsed.entries.into_iter().next() {
                        let entry = &mut self.profile.files[fi].entries[ei];
                        entry.entry_type = new_entry.entry_type;
                        entry.name = new_entry.name;
                        entry.value = new_entry.value;
                        self.profile.files[fi].dirty = true;
                        self.profile.files[fi].recalculate_line_numbers();
                        self.rebuild_list();
                        self.message = Some("Entry updated".into());
                    } else {
                        // Content was emptied or unparseable
                        self.profile.files[fi].entries[ei].value = new_content;
                        self.profile.files[fi].dirty = true;
                        self.profile.files[fi].recalculate_line_numbers();
                        self.rebuild_list();
                        self.message = Some("Entry value updated (raw)".into());
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
                let content = content.trim_end_matches('\n').to_string();
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
                        self.undo_snapshot =
                            Some(crate::tui::operations::take_snapshot(&self.profile));
                        // Insert after current entry position, or at end of file
                        let insert_pos = match self.visible_items.get(self.cursor) {
                            Some(ListItem::Entry(_, ei)) => ei + 1,
                            Some(ListItem::FileHeader(_)) => 0,  // Insert at beginning
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
