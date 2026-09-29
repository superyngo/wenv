//! TUI application core

mod confirm;
mod external;
mod navigation;
mod placement;
mod profile_io;
mod text_input;

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::{backend::CrosstermBackend, Terminal};

use std::collections::VecDeque;
use std::io;

use crate::i18n::Messages;
use crate::model::profile::{ListItem, ShellProfile};
use crate::tui::keys::{self, Action};
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
    /// Snapshot taken when delete is requested; pushed to `undo_stack` only on confirm.
    /// Text last removed by a kill shortcut (`Ctrl+U/K/W`), for `Ctrl+Y`.
    pub kill_ring: String,
    pub pending_undo: Option<crate::tui::state::UndoSnapshot>,
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
            kill_ring: String::new(),
            pending_undo: None,
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
        // Restore the terminal before the panic message prints, so a panic never leaves
        // the shell in raw mode / alternate screen.
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = crossterm::terminal::disable_raw_mode();
            let _ = crossterm::execute!(
                io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show
            );
            previous_hook(info);
        }));
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Suppress resolver stderr warnings while the alternate screen is active;
        // otherwise a stray warning line (e.g. on reload) corrupts the rendered list.
        crate::config::path_resolver::set_quiet(true);
        let result = self.event_loop(&mut terminal);
        crate::config::path_resolver::set_quiet(false);

        // Always restore terminal
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        )?;
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
                let total_height = f.area().height as usize;
                // title(1) + status(1) + search?(1) + column header(1) + separator(1).
                // Must match the real rendered rows exactly: if this undercounts,
                // ratatui silently shifts the list offset to keep the selection
                // visible and it desyncs from `self.scroll_offset` (cursor sticks
                // at the bottom row while the viewport scrolls on the first ↑).
                let chrome = if self.search.is_some() { 5 } else { 4 };
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
            Action::NavigateUp => self.action_navigate_up(),
            Action::NavigateDown => self.action_navigate_down(),
            Action::PageUp => self.action_page_up(),
            Action::PageDown => self.action_page_down(),
            Action::Home => self.action_home(),
            Action::End => self.action_end(),
            Action::ToggleExpand => self.action_toggle_expand(),
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
            Action::RangeSelectUp => self.action_range_select_up(),
            Action::RangeSelectDown => self.action_range_select_down(),
            Action::Edit => return self.action_edit(),
            Action::EditExternal => return self.action_edit_external(),
            Action::InlineEdit(op) => {
                if let Some(e) = &mut self.inline_edit {
                    crate::tui::line_edit::apply(
                        op,
                        &mut e.buffer,
                        &mut e.cursor,
                        &mut self.kill_ring,
                    );
                }
            }
            Action::TextInputEdit(op) => {
                if let Some(t) = &mut self.text_input {
                    crate::tui::line_edit::apply(
                        op,
                        &mut t.value,
                        &mut t.cursor_pos,
                        &mut self.kill_ring,
                    );
                }
            }
            Action::SearchEdit(op) => {
                if let Some(search) = &mut self.search {
                    crate::tui::line_edit::apply(
                        op,
                        &mut search.query,
                        &mut search.cursor,
                        &mut self.kill_ring,
                    );
                }
                self.update_filter();
            }
            Action::InlineInput(c) => self.inline_input_char(c),
            Action::InlineBackspace => self.inline_backspace(),
            Action::InlineDelete => self.inline_delete(),
            Action::InlineLeft => self.inline_cursor_left(),
            Action::InlineRight => self.inline_cursor_right(),
            Action::InlineHome => self.inline_cursor_home(),
            Action::InlineEnd => self.inline_cursor_end(),
            Action::Add => return self.action_add(),
            Action::SnippetUp => self.action_snippet_up(),
            Action::SnippetDown => self.action_snippet_down(),
            Action::SnippetSelect => return Ok(self.action_snippet_select()),
            Action::SnippetCancel => self.action_snippet_cancel(),
            Action::Delete => self.action_delete()?,
            Action::Cut => {
                if self.mode == AppMode::Moving {
                    self.set_placement_cut(true);
                } else {
                    self.begin_placement(true);
                }
            }
            Action::Copy => {
                if self.mode == AppMode::Moving {
                    self.set_placement_cut(false);
                } else {
                    self.begin_placement(false);
                }
            }
            Action::StartMove => self.action_start_move(),
            Action::Undo => self.action_undo(),
            Action::Redo => self.action_redo(),
            Action::Save => self.action_save(),
            Action::Confirm => return self.action_confirm(),
            Action::Cancel => self.action_cancel(),
            Action::Quit => {
                if self.profile.any_dirty() {
                    self.mode = AppMode::ConfirmQuit;
                    let dirty: Vec<_> = self
                        .profile
                        .dirty_files()
                        .iter()
                        .map(|f| f.display_name())
                        .collect();
                    self.message = Some(
                        self.messages
                            .fmt("msg_unsaved_changes_quit", &[&dirty.join(", ")]),
                    );
                } else {
                    self.should_quit = true;
                }
            }
            Action::Search => self.action_search(),
            Action::SearchInput(c) => {
                if let Some(search) = &mut self.search {
                    search.input_char(c);
                }
                self.update_filter();
            }
            Action::SearchBackspace => {
                if let Some(search) = &mut self.search {
                    search.backspace();
                }
                self.update_filter();
            }
            Action::SearchDelete => {
                if let Some(search) = &mut self.search {
                    search.delete();
                }
                self.update_filter();
            }
            Action::SearchLeft => {
                if let Some(search) = &mut self.search {
                    search.cursor_left();
                }
            }
            Action::SearchRight => {
                if let Some(search) = &mut self.search {
                    search.cursor_right();
                }
            }
            Action::SearchHome => {
                if let Some(search) = &mut self.search {
                    search.cursor_home();
                }
            }
            Action::SearchEnd => {
                if let Some(search) = &mut self.search {
                    search.cursor_end();
                }
            }
            Action::Help => {
                self.mode = AppMode::ShowingHelp;
                self.detail_scroll_offset = 0;
            }
            Action::Remark => self.action_remark()?,
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
            Action::AddFile => self.action_add_file(),
            Action::TextInputChar(c) => self.action_text_input_char(c),
            Action::TextInputBackspace => self.action_text_input_backspace(),
            Action::TextInputDelete => self.action_text_input_delete(),
            Action::TextInputLeft => self.action_text_input_left(),
            Action::TextInputRight => self.action_text_input_right(),
            Action::TextInputHome => self.action_text_input_home(),
            Action::TextInputEnd => self.action_text_input_end(),
            _ => {}
        }
        Ok(EditorRequest::None)
    }

    pub fn rebuild_list(&mut self) {
        if let Some(search) = &self.search {
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
    fn cut_drop_downward_lands_cursor_on_moved_entry() {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("x.zsh");
        std::fs::write(&f, "alias a=1\nalias b=2\nalias c=3\n").unwrap();
        let mut app = app_for(&td, vec![f.display().to_string()]);
        app.profile.toggle_all(true);
        app.rebuild_list();
        let first = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::Entry(_, 0)))
            .unwrap();
        app.cursor = first;
        app.handle_action(Action::Cut).unwrap();
        // Place after the last entry
        let last = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::Entry(_, 2)))
            .unwrap();
        app.move_state.as_mut().unwrap().insertion_cursor = last;
        app.handle_action(Action::Confirm).unwrap();

        let names: Vec<_> = app.profile.files[0]
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect();
        assert_eq!(names, vec!["b", "c", "a"]);
        assert!(matches!(
            app.visible_items[app.cursor],
            ListItem::Entry(0, 2)
        ));
    }

    #[test]
    fn delete_cancel_keeps_redo_and_confirm_pushes_undo() {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("x.zsh");
        std::fs::write(&f, "alias x=1\n").unwrap();
        let mut app = app_for(&td, vec![f.display().to_string()]);
        app.profile.toggle_all(true);
        app.rebuild_list();
        app.cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::Entry(..)))
            .unwrap();
        app.redo_stack
            .push(crate::tui::operations::take_snapshot(&app.profile));

        app.handle_action(Action::Delete).unwrap();
        assert_eq!(app.mode, AppMode::ConfirmDelete);
        app.handle_action(Action::Cancel).unwrap();
        assert_eq!(app.redo_stack.len(), 1, "cancel must not clear redo");
        assert!(app.undo_stack.is_empty());

        app.handle_action(Action::Delete).unwrap();
        app.handle_action(Action::Confirm).unwrap();
        assert_eq!(app.undo_stack.len(), 1);
        assert!(app.redo_stack.is_empty());
    }

    #[test]
    fn edit_file_refuses_when_dirty() {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("x.zsh");
        std::fs::write(&f, "alias x=1\n").unwrap();
        let mut app = app_for(&td, vec![f.display().to_string()]);
        app.profile.files[0].dirty = true;
        let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
        let mut term = Terminal::new(backend).unwrap();
        app.run_edit_file(&mut term, 0).unwrap();
        assert!(app.message.unwrap().contains("Unsaved"));
    }

    #[test]
    fn file_move_reorders_config_patterns_not_resolved_indices() {
        let td = tempfile::tempdir().unwrap();
        let sub = td.path().join("zshrc.d");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.sh"), "alias a=1\n").unwrap();
        std::fs::write(sub.join("b.sh"), "alias b=2\n").unwrap();
        let x = td.path().join("x.zsh");
        let y = td.path().join("y.zsh");
        std::fs::write(&x, "alias x=1\n").unwrap();
        std::fs::write(&y, "alias y=1\n").unwrap();
        let glob = format!("{}/*.sh", sub.display());
        let xs = x.display().to_string();
        let ys = y.display().to_string();

        let mut app = app_for(&td, vec![glob.clone(), xs.clone(), ys.clone()]);
        // files: a.sh(0) b.sh(1) x(2) y(3); move y above the glob group (onto a.sh)
        let fi = app.profile.files.iter().position(|f| f.path == y).unwrap();
        let target = app
            .profile
            .files
            .iter()
            .position(|f| f.path.ends_with("a.sh"))
            .unwrap();
        app.profile.toggle_all(false);
        app.rebuild_list();
        for n in &mut app.profile.tree {
            if let TreeNode::Dir(g) = n {
                g.expanded = true;
            }
        }
        app.rebuild_list();
        let cursor = app
            .visible_items
            .iter()
            .position(|it| matches!(it, ListItem::FileHeader(i) if *i == target))
            .unwrap();
        app.file_move_state = Some(FileMovingState {
            original_fi: fi,
            insertion_cursor: cursor,
            saved_expanded: crate::tui::state::ExpandedSnapshot {
                files: vec![false; app.profile.files.len()],
                dirs: vec![true],
            },
        });
        app.previous_mode = Some(AppMode::Normal);
        app.mode = AppMode::MovingFile;
        app.execute_file_move();

        let paths = &app.config.files["zsh"].paths;
        assert_eq!(paths, &vec![ys, glob, xs]);
        // Tree and files agree after the reload
        assert_eq!(app.profile.files[0].path, y);
        assert_eq!(app.profile.files.len(), 4);
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
        assert_eq!(text_input::clamp_inline_scroll(0, 0, 10, 5), 0);
        assert_eq!(text_input::clamp_inline_scroll(0, 9, 10, 5), 5);
        // Cursor scrolled left below the window pulls the viewport back.
        assert_eq!(text_input::clamp_inline_scroll(5, 2, 10, 5), 2);
        // No blank gap past the end after the buffer shrinks.
        assert_eq!(text_input::clamp_inline_scroll(8, 3, 3, 5), 0);
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

    /// Regression: the chrome row count in the event loop must match the rows
    /// the renderer actually occupies. If it undercounts, ratatui silently
    /// bumps the list offset to keep the selection visible and desyncs from
    /// `scroll_offset` (symptom: at the bottom, the first ↑ scrolls the
    /// viewport while the cursor stays put).
    #[test]
    fn viewport_height_matches_rendered_chrome() {
        use ratatui::backend::TestBackend;
        let (_td, mut app) = app_group();
        // Visible list: DirHeader, one.sh header, entry a, two.sh header, entry b.
        assert_eq!(app.visible_items.len(), 5);
        let mut terminal = ratatui::Terminal::new(TestBackend::new(80, 7)).unwrap();
        app.cursor = 4; // last item (entry b)
        terminal
            .draw(|f| {
                // Mirror the event_loop sizing exactly.
                let total_height = f.area().height as usize;
                let chrome = if app.search.is_some() { 5 } else { 4 };
                app.list_visible_height = total_height.saturating_sub(chrome);
                app.clamp_scroll_offset();
                crate::tui::ui::draw(f, &mut app);
            })
            .unwrap();
        // 7 rows - 4 chrome = 3 visible rows; cursor 4 → scroll 2.
        assert_eq!(app.list_visible_height, 3);
        assert_eq!(app.scroll_offset, 2);
        let buf = terminal.backend().buffer();
        let row = |y: u16| -> String { (0..80).map(|x| buf[(x, y)].symbol()).collect() };
        // First list row (below title + column header + separator) must show the
        // item at scroll_offset — entry a. If ratatui had shifted the offset,
        // this row would show the next item instead.
        assert!(row(3).contains("alias a='1'"), "row 3 was: {}", row(3));
        // Last list row (just above the status bar) shows the cursor item.
        assert!(row(5).contains("alias b='2'"), "row 5 was: {}", row(5));
    }

    /// Regression: popups must not panic on tiny terminals (`clamp(min, max)` with
    /// `min > max` aborted the whole TUI on 1-row terminals).
    #[test]
    fn popups_survive_tiny_terminals() {
        use ratatui::backend::TestBackend;
        for (w, h) in [(0u16, 0u16), (1, 1), (3, 2), (10, 3), (20, 5)] {
            for mode in [
                AppMode::ShowingHelp,
                AppMode::ShowingDetail,
                AppMode::ConfirmQuit,
            ] {
                let (_td, mut app) = app_group();
                app.mode = mode;
                let mut terminal = ratatui::Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal
                    .draw(|f| crate::tui::ui::draw(f, &mut app))
                    .unwrap();
            }
        }
    }
}
