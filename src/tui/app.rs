//! TUI application state and logic

use std::io;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::i18n::Messages;
use crate::model::{Entry, EntryType, ShellType};

/// Application mode
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    ConfirmQuit,   // Confirm quit with unsaved changes
    SelectingType, // For [a]Add - selecting entry type
    Editing,       // Unified editing mode
    Moving,        // Moving entry up/down
}

/// Edit field focus
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditField {
    Name,
    Value,
    Submit,
}

impl EditField {
    /// Move to next field (Tab)
    pub fn next(self) -> Self {
        match self {
            EditField::Name => EditField::Value,
            EditField::Value => EditField::Submit,
            EditField::Submit => EditField::Name,
        }
    }

    /// Move to previous field (Shift+Tab)
    pub fn prev(self) -> Self {
        match self {
            EditField::Name => EditField::Submit,
            EditField::Value => EditField::Name,
            EditField::Submit => EditField::Value,
        }
    }

    /// Move to next field for types that skip Name (Source/Code/Comment)
    pub fn next_skip_name(self) -> Self {
        match self {
            EditField::Name => EditField::Value,
            EditField::Value => EditField::Submit,
            EditField::Submit => EditField::Value,
        }
    }

    /// Move to previous field for types that skip Name (Source/Code/Comment)
    pub fn prev_skip_name(self) -> Self {
        match self {
            EditField::Name => EditField::Value,
            EditField::Value => EditField::Submit,
            EditField::Submit => EditField::Value,
        }
    }
}

/// Edit state for unified editing
#[derive(Debug, Clone)]
pub struct EditState {
    pub field: EditField,
    pub name_buffer: String,
    pub value_buffer: String,
    pub entry_type: EntryType,
    pub is_new: bool,           // true for Add, false for Edit
    pub cursor_position: usize, // Cursor position in current field (byte offset)
    pub cursor_row: usize,      // Current row in value field (for multi-line)
    pub cursor_col: usize,      // Current column in value field (for multi-line)
    pub scroll_offset: usize,   // Scroll offset for value display
}

/// TUI Application state
pub struct TuiApp {
    // State
    pub entries: Vec<Entry>,
    pub selected_index: usize,
    pub list_scroll_offset: usize,
    pub list_visible_height: usize, // Updated during draw

    // File
    pub file_path: PathBuf,
    pub file_content: String,
    pub shell_type: ShellType,

    // UI state
    pub mode: AppMode,
    pub message: Option<String>,
    pub should_quit: bool,

    // Edit state (unified)
    pub edit_state: Option<EditState>,

    // Type selection for Add
    pub type_selection_index: usize,
    pub type_list_scroll_offset: usize,

    // Move mode original position
    pub move_original_index: Option<usize>,

    // Detail popup scroll offset
    pub detail_scroll: usize,

    // i18n
    pub messages: &'static Messages,

    // Dirty flag and temp file for unsaved changes
    pub dirty: bool,
    pub temp_file_path: PathBuf,

    // Multi-selection state
    pub selection_anchor: Option<usize>,
    pub selected_range: Option<(usize, usize)>, // (min, max) inclusive
}

impl TuiApp {
    /// Create a new TUI app
    pub fn new(
        file_path: PathBuf,
        shell_type: ShellType,
        messages: &'static Messages,
    ) -> Result<Self> {
        let file_content = crate::utils::path::read_file(&file_path)?;
        let parser = crate::parser::get_parser(shell_type);
        let parse_result = parser.parse(&file_content);
        let entries = parse_result.entries;

        // Create temp file path in same directory
        let temp_file_path = {
            let filename = file_path.file_name().unwrap_or_default().to_string_lossy();
            file_path.with_file_name(format!("{}.wenv.tmp", filename))
        };

        Ok(Self {
            entries,
            selected_index: 0,
            list_scroll_offset: 0,
            list_visible_height: 20,
            file_path,
            file_content,
            shell_type,
            mode: AppMode::Normal,
            message: None,
            should_quit: false,
            edit_state: None,
            type_selection_index: 0,
            type_list_scroll_offset: 0,
            move_original_index: None,
            detail_scroll: 0,
            messages,
            dirty: false,
            temp_file_path,
            selection_anchor: None,
            selected_range: None,
        })
    }

    /// Run the TUI application
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the main loop
        let result = self.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop
    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|f| crate::tui::ui::draw(f, self))?;

            match event::read()? {
                Event::Key(key) => {
                    // Only handle key press events, ignore release
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key)?;
                    }
                }
                Event::Mouse(mouse) => {
                    self.handle_mouse(mouse.kind)?;
                }
                _ => {}
            }

            if self.should_quit {
                // Clean up temp file on quit
                self.cleanup_temp_file();
                break;
            }
        }

        Ok(())
    }

    /// Handle mouse input
    fn handle_mouse(&mut self, kind: MouseEventKind) -> Result<()> {
        match kind {
            MouseEventKind::ScrollUp => {
                match self.mode {
                    AppMode::Normal => {
                        self.move_up();
                    }
                    AppMode::Moving => {
                        self.move_entry_up();
                    }
                    AppMode::ShowingDetail => {
                        self.detail_scroll = self.detail_scroll.saturating_sub(3);
                    }
                    AppMode::Editing => {
                        if let Some(ref state) = self.edit_state {
                            if state.field == EditField::Value {
                                // Move cursor up in value field
                                self.move_cursor_up_in_value();
                            }
                        }
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollDown => {
                match self.mode {
                    AppMode::Normal => {
                        self.move_down();
                    }
                    AppMode::Moving => {
                        self.move_entry_down();
                    }
                    AppMode::ShowingDetail => {
                        self.detail_scroll = self.detail_scroll.saturating_add(3);
                    }
                    AppMode::Editing => {
                        if let Some(ref state) = self.edit_state {
                            if state.field == EditField::Value {
                                // Move cursor down in value field
                                self.move_cursor_down_in_value();
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keyboard input
    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key)?,
            AppMode::ShowingDetail => self.handle_detail_mode(key.code)?,
            AppMode::ShowingHelp => self.handle_help_mode(key.code)?,
            AppMode::ConfirmDelete => self.handle_confirm_delete_mode(key.code)?,
            AppMode::ConfirmQuit => self.handle_confirm_quit_mode(key.code)?,
            AppMode::SelectingType => self.handle_selecting_type_mode(key.code)?,
            AppMode::Editing => self.handle_editing_mode(key.code)?,
            AppMode::Moving => self.handle_moving_mode(key.code)?,
        }

        Ok(())
    }

    /// Handle keys in normal mode
    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            // Ctrl+S: Save to original file
            KeyCode::Char('s') if has_ctrl => {
                self.save_to_original_file()?;
            }
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.dirty {
                    self.mode = AppMode::ConfirmQuit;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::ShowingHelp;
            }
            // Navigation with Shift for multi-select
            KeyCode::Up | KeyCode::Char('k') => {
                if has_shift {
                    self.extend_selection_up();
                } else {
                    self.clear_selection();
                    self.move_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if has_shift {
                    self.extend_selection_down();
                } else {
                    self.clear_selection();
                    self.move_down();
                }
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                self.clear_selection();
                self.mode = AppMode::ShowingDetail;
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                self.mode = AppMode::ConfirmDelete;
            }
            KeyCode::Home => {
                self.clear_selection();
                self.jump_to_first();
            }
            KeyCode::End => {
                self.clear_selection();
                self.jump_to_last();
            }
            KeyCode::PageUp => {
                self.clear_selection();
                self.page_up();
            }
            KeyCode::PageDown => {
                self.clear_selection();
                self.page_down();
            }
            KeyCode::Char('f') => {
                self.format_file()?;
            }
            KeyCode::Char('c') => {
                self.check_entry()?;
            }
            KeyCode::Char('a') => {
                self.clear_selection();
                self.start_adding_entry();
            }
            KeyCode::Char('e') => {
                self.clear_selection();
                self.start_editing();
            }
            KeyCode::Char('m') => {
                self.clear_selection();
                self.start_moving();
            }
            KeyCode::Char('t') => {
                self.toggle_comment()?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keys in detail mode
    fn handle_detail_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') | KeyCode::Enter => {
                self.mode = AppMode::Normal;
                self.detail_scroll = 0;
            }
            KeyCode::Char('e') => {
                // Enter edit mode from detail view
                self.detail_scroll = 0;
                self.start_editing();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                // Scroll up in detail view
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // Scroll down in detail view
                self.detail_scroll = self.detail_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(10);
            }
            KeyCode::Home => {
                self.detail_scroll = 0;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keys in help mode
    fn handle_help_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keys in confirm delete mode
    fn handle_confirm_delete_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.delete_selected_entries()?;
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keys in confirm quit mode
    fn handle_confirm_quit_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Save and quit
                self.save_to_original_file()?;
                self.should_quit = true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Discard changes and quit
                self.should_quit = true;
            }
            KeyCode::Esc => {
                // Cancel, go back to Normal
                self.mode = AppMode::Normal;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle type selection mode for Add
    fn handle_selecting_type_mode(&mut self, key: KeyCode) -> Result<()> {
        const NUM_TYPES: usize = 5; // Alias, Function, EnvVar, Source, Code/Comment

        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.type_selection_index > 0 {
                    self.type_selection_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.type_selection_index < NUM_TYPES - 1 {
                    self.type_selection_index += 1;
                }
            }
            KeyCode::Enter => {
                self.confirm_type_selection();
            }
            KeyCode::Char('1') => {
                self.type_selection_index = 0;
                self.confirm_type_selection();
            }
            KeyCode::Char('2') => {
                self.type_selection_index = 1;
                self.confirm_type_selection();
            }
            KeyCode::Char('3') => {
                self.type_selection_index = 2;
                self.confirm_type_selection();
            }
            KeyCode::Char('4') => {
                self.type_selection_index = 3;
                self.confirm_type_selection();
            }
            KeyCode::Char('5') => {
                self.type_selection_index = 4;
                self.confirm_type_selection();
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.mode = AppMode::Normal;
                self.type_selection_index = 0;
                self.type_list_scroll_offset = 0;
                self.message = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Confirm type selection and open edit window
    fn confirm_type_selection(&mut self) {
        // Code/Comment merged: index 4 is now Code/Comment, parser decides final type
        let entry_type = match self.type_selection_index {
            0 => EntryType::Alias,
            1 => EntryType::Function,
            2 => EntryType::EnvVar,
            3 => EntryType::Source,
            4 => EntryType::Code, // Code/Comment - parser will decide based on content
            _ => EntryType::Alias,
        };

        // For Source/Code, start at Value field (skip Name)
        let start_field = if matches!(entry_type, EntryType::Source | EntryType::Code) {
            EditField::Value
        } else {
            EditField::Name
        };

        // Create edit state for new entry
        self.edit_state = Some(EditState {
            field: start_field,
            name_buffer: String::new(),
            value_buffer: String::new(),
            entry_type,
            is_new: true,
            cursor_position: 0,
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
        });
        self.mode = AppMode::Editing;
        self.type_selection_index = 0;
        self.type_list_scroll_offset = 0;
        self.message = None;
    }

    /// Handle unified editing mode
    fn handle_editing_mode(&mut self, key: KeyCode) -> Result<()> {
        let Some(ref mut state) = self.edit_state else {
            self.mode = AppMode::Normal;
            return Ok(());
        };

        match key {
            KeyCode::Tab => {
                // For Source/Code/Comment, skip the Name field
                let skip_name = matches!(
                    state.entry_type,
                    EntryType::Source | EntryType::Code | EntryType::Comment
                );
                state.field = if skip_name {
                    state.field.next_skip_name()
                } else {
                    state.field.next()
                };
            }
            KeyCode::BackTab => {
                let skip_name = matches!(
                    state.entry_type,
                    EntryType::Source | EntryType::Code | EntryType::Comment
                );
                state.field = if skip_name {
                    state.field.prev_skip_name()
                } else {
                    state.field.prev()
                };
            }
            KeyCode::Esc => {
                // Cancel editing from any field (including Submit)
                self.cancel_editing();
            }
            KeyCode::Enter => {
                if state.field == EditField::Submit {
                    // Submit the edit
                    self.submit_editing()?;
                } else if state.field == EditField::Value {
                    // For Source type, Enter submits directly (single-line only)
                    if state.entry_type == EntryType::Source {
                        self.submit_editing()?;
                    } else {
                        // Insert newline in value field for other types
                        state.value_buffer.insert(state.cursor_position, '\n');
                        state.cursor_position += 1;
                        state.cursor_row += 1;
                        state.cursor_col = 0;
                    }
                } else {
                    // Move to next field
                    state.field = state.field.next();
                }
            }
            KeyCode::Up => {
                if state.field == EditField::Value {
                    // Move cursor up in multi-line value
                    self.move_cursor_up_in_value();
                }
            }
            KeyCode::Down => {
                if state.field == EditField::Value {
                    // Move cursor down in multi-line value
                    self.move_cursor_down_in_value();
                }
            }
            KeyCode::PageUp => {
                if state.field == EditField::Value {
                    // Move cursor up 10 lines
                    for _ in 0..10 {
                        self.move_cursor_up_in_value();
                    }
                }
            }
            KeyCode::PageDown => {
                if state.field == EditField::Value {
                    // Move cursor down 10 lines
                    for _ in 0..10 {
                        self.move_cursor_down_in_value();
                    }
                }
            }
            KeyCode::Char(c) => {
                match state.field {
                    EditField::Name => {
                        state.name_buffer.insert(state.cursor_position, c);
                        state.cursor_position += c.len_utf8();
                    }
                    EditField::Value => {
                        state.value_buffer.insert(state.cursor_position, c);
                        state.cursor_position += c.len_utf8();
                        state.cursor_col += 1;
                    }
                    EditField::Submit => {
                        // 'q' on Submit should quit
                        if c == 'q' {
                            self.cancel_editing();
                        }
                    }
                }
            }
            KeyCode::Backspace => match state.field {
                EditField::Name => {
                    if state.cursor_position > 0 {
                        // Find the previous character boundary
                        let new_pos = prev_char_boundary(&state.name_buffer, state.cursor_position);
                        state.name_buffer.drain(new_pos..state.cursor_position);
                        state.cursor_position = new_pos;
                    }
                }
                EditField::Value => {
                    if state.cursor_position > 0 {
                        // Find the previous character boundary
                        let new_pos =
                            prev_char_boundary(&state.value_buffer, state.cursor_position);
                        let removed: String = state
                            .value_buffer
                            .drain(new_pos..state.cursor_position)
                            .collect();
                        state.cursor_position = new_pos;
                        if removed.contains('\n') {
                            // Recalculate row and col
                            self.recalculate_cursor_row_col();
                        } else {
                            state.cursor_col = state.cursor_col.saturating_sub(1);
                        }
                    }
                }
                EditField::Submit => {}
            },
            KeyCode::Delete => match state.field {
                EditField::Name => {
                    if state.cursor_position < state.name_buffer.len() {
                        let next_pos =
                            next_char_boundary(&state.name_buffer, state.cursor_position);
                        state.name_buffer.drain(state.cursor_position..next_pos);
                    }
                }
                EditField::Value => {
                    if state.cursor_position < state.value_buffer.len() {
                        let next_pos =
                            next_char_boundary(&state.value_buffer, state.cursor_position);
                        state.value_buffer.drain(state.cursor_position..next_pos);
                    }
                }
                EditField::Submit => {}
            },
            KeyCode::Left => {
                if state.cursor_position > 0 {
                    let buffer = match state.field {
                        EditField::Name => &state.name_buffer,
                        EditField::Value => &state.value_buffer,
                        EditField::Submit => return Ok(()),
                    };
                    state.cursor_position = prev_char_boundary(buffer, state.cursor_position);
                    if state.field == EditField::Value {
                        self.recalculate_cursor_row_col();
                    }
                }
            }
            KeyCode::Right => {
                let max_pos = match state.field {
                    EditField::Name => state.name_buffer.len(),
                    EditField::Value => state.value_buffer.len(),
                    EditField::Submit => 0,
                };
                if state.cursor_position < max_pos {
                    let buffer = match state.field {
                        EditField::Name => &state.name_buffer,
                        EditField::Value => &state.value_buffer,
                        EditField::Submit => return Ok(()),
                    };
                    state.cursor_position = next_char_boundary(buffer, state.cursor_position);
                    if state.field == EditField::Value {
                        self.recalculate_cursor_row_col();
                    }
                }
            }
            KeyCode::Home => {
                if state.field == EditField::Value {
                    // Go to start of current line
                    let safe_pos = find_char_boundary(&state.value_buffer, state.cursor_position);
                    let before = &state.value_buffer[..safe_pos];
                    if let Some(newline_pos) = before.rfind('\n') {
                        state.cursor_position = newline_pos + 1;
                    } else {
                        state.cursor_position = 0;
                    }
                    state.cursor_col = 0;
                } else {
                    state.cursor_position = 0;
                }
            }
            KeyCode::End => {
                if state.field == EditField::Value {
                    // Go to end of current line
                    let safe_pos = find_char_boundary(&state.value_buffer, state.cursor_position);
                    let remaining = &state.value_buffer[safe_pos..];
                    let line_end = remaining.find('\n').unwrap_or(remaining.len());
                    state.cursor_position = safe_pos + line_end;
                    self.recalculate_cursor_row_col();
                } else {
                    state.cursor_position = match state.field {
                        EditField::Name => state.name_buffer.len(),
                        EditField::Value => state.value_buffer.len(),
                        EditField::Submit => 0,
                    };
                }
            }
            _ => {}
        }

        // Update cursor position when switching fields
        if matches!(key, KeyCode::Tab | KeyCode::BackTab) {
            if let Some(ref mut state) = self.edit_state {
                state.cursor_position = match state.field {
                    EditField::Name => state.name_buffer.len(),
                    EditField::Value => state.value_buffer.len(),
                    EditField::Submit => 0,
                };
                state.cursor_row = 0;
                state.cursor_col = state.cursor_position;
                if state.field == EditField::Value {
                    self.recalculate_cursor_row_col();
                }
            }
        }

        Ok(())
    }

    /// Move cursor up in multi-line value field
    fn move_cursor_up_in_value(&mut self) {
        let Some(ref mut state) = self.edit_state else {
            return;
        };

        if state.cursor_row == 0 {
            return;
        }

        let lines: Vec<&str> = state.value_buffer.lines().collect();
        let target_row = state.cursor_row - 1;
        let target_line = lines.get(target_row).unwrap_or(&"");
        let target_col = state.cursor_col.min(target_line.len());

        // Calculate new cursor position
        let mut new_pos = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == target_row {
                new_pos += target_col;
                break;
            }
            new_pos += line.len() + 1; // +1 for newline
        }

        state.cursor_position = new_pos.min(state.value_buffer.len());
        state.cursor_row = target_row;
        state.cursor_col = target_col;
    }

    /// Move cursor down in multi-line value field
    fn move_cursor_down_in_value(&mut self) {
        let Some(ref mut state) = self.edit_state else {
            return;
        };

        let lines: Vec<&str> = state.value_buffer.lines().collect();
        let line_count = lines.len().max(1);

        if state.cursor_row >= line_count - 1 {
            return;
        }

        let target_row = state.cursor_row + 1;
        let target_line = lines.get(target_row).unwrap_or(&"");
        let target_col = state.cursor_col.min(target_line.len());

        // Calculate new cursor position
        let mut new_pos = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == target_row {
                new_pos += target_col;
                break;
            }
            new_pos += line.len() + 1; // +1 for newline
        }

        state.cursor_position = new_pos.min(state.value_buffer.len());
        state.cursor_row = target_row;
        state.cursor_col = target_col;
    }

    /// Recalculate cursor row and column from cursor position
    fn recalculate_cursor_row_col(&mut self) {
        let Some(ref mut state) = self.edit_state else {
            return;
        };

        // Ensure cursor_position is at a valid char boundary
        let safe_pos = find_char_boundary(&state.value_buffer, state.cursor_position);
        let before_cursor = &state.value_buffer[..safe_pos];
        state.cursor_row = before_cursor.matches('\n').count();
        state.cursor_col = before_cursor
            .rfind('\n')
            .map_or(before_cursor.chars().count(), |pos| {
                before_cursor[pos + 1..].chars().count()
            });
    }

    /// Handle moving mode
    fn handle_moving_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_entry_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_entry_down();
            }
            KeyCode::Enter => {
                // Confirm move - update line numbers to reflect new order
                self.reindex_line_numbers();
                self.save_to_file()?;
                self.mode = AppMode::Normal;
                self.move_original_index = None;
                self.message = Some("Entry moved successfully".to_string());
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                // Cancel move - restore original position
                if let Some(original) = self.move_original_index {
                    // Swap back to original position
                    if original != self.selected_index {
                        let entry = self.entries.remove(self.selected_index);
                        self.entries.insert(original, entry);
                        self.selected_index = original;
                    }
                }
                self.mode = AppMode::Normal;
                self.move_original_index = None;
                self.message = None;
            }
            _ => {}
        }
        Ok(())
    }

    /// Reindex line numbers to match current entry order
    /// This ensures the formatter preserves the user's intended order
    fn reindex_line_numbers(&mut self) {
        let mut line = 1;
        for entry in &mut self.entries {
            entry.line_number = Some(line);
            // For multi-line entries, calculate end_line based on content
            let line_count = entry.value.lines().count().max(1);
            if line_count > 1 {
                entry.end_line = Some(line + line_count - 1);
                line += line_count;
            } else {
                entry.end_line = None;
                line += 1;
            }
        }
    }

    /// Move entry up in the list
    fn move_entry_up(&mut self) {
        if self.selected_index > 0 {
            self.entries
                .swap(self.selected_index, self.selected_index - 1);
            self.selected_index -= 1;
        }
    }

    /// Move entry down in the list
    fn move_entry_down(&mut self) {
        if self.selected_index < self.entries.len().saturating_sub(1) {
            self.entries
                .swap(self.selected_index, self.selected_index + 1);
            self.selected_index += 1;
        }
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.adjust_scroll_for_selection();
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        if self.selected_index < self.entries.len().saturating_sub(1) {
            self.selected_index += 1;
            self.adjust_scroll_for_selection();
        }
    }

    /// Jump to first entry
    fn jump_to_first(&mut self) {
        self.selected_index = 0;
        self.list_scroll_offset = 0;
    }

    /// Jump to last entry
    fn jump_to_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = self.entries.len() - 1;
            self.adjust_scroll_for_selection();
        }
    }

    /// Page up (move 10 entries up)
    fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
        self.adjust_scroll_for_selection();
    }

    /// Page down (move 10 entries down)
    fn page_down(&mut self) {
        let max_index = self.entries.len().saturating_sub(1);
        self.selected_index = std::cmp::min(self.selected_index + 10, max_index);
        self.adjust_scroll_for_selection();
    }

    /// Adjust scroll offset to keep selection visible
    /// Only scrolls when selection reaches the exact edge of visible area
    pub fn adjust_scroll_for_selection(&mut self) {
        // Visible height includes: border (2) + header (1) + separator (1)
        // So actual visible entries = height - 4
        let visible_entries = self.list_visible_height.saturating_sub(4);

        if visible_entries == 0 || self.entries.is_empty() {
            return;
        }

        // Clamp scroll offset to valid range first
        let max_scroll = self.entries.len().saturating_sub(visible_entries);
        if self.list_scroll_offset > max_scroll {
            self.list_scroll_offset = max_scroll;
        }

        // If selection is above visible area, scroll up (only when at exact top edge)
        if self.selected_index < self.list_scroll_offset {
            self.list_scroll_offset = self.selected_index;
        }

        // If selection is at or beyond bottom edge, scroll down
        if self.selected_index >= self.list_scroll_offset + visible_entries {
            self.list_scroll_offset = self.selected_index.saturating_sub(visible_entries) + 1;
        }
    }

    /// Get the currently selected entry
    pub fn get_selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected_index)
    }

    /// Get all selected indices (for multi-select operations)
    fn get_selected_indices(&self) -> Vec<usize> {
        if let Some((min, max)) = self.selected_range {
            (min..=max).collect()
        } else {
            vec![self.selected_index]
        }
    }

    /// Delete selected entries (supports multi-select)
    /// Directly removes lines from temp file, then reloads
    fn delete_selected_entries(&mut self) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        let indices = self.get_selected_indices();
        let count = indices.len();

        // Collect all line numbers to remove (1-indexed)
        let mut lines_to_remove: Vec<usize> = Vec::new();
        for idx in &indices {
            if let Some(entry) = self.entries.get(*idx) {
                let start = entry.line_number.unwrap_or(1);
                let end = entry.end_line.unwrap_or(start);
                for line in start..=end {
                    lines_to_remove.push(line);
                }
            }
        }
        lines_to_remove.sort();
        lines_to_remove.dedup();

        // Read current content and remove specified lines
        let content = self.read_current_content()?;
        let lines: Vec<&str> = content.lines().collect();
        let new_lines: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !lines_to_remove.contains(&(i + 1))) // 1-indexed
            .map(|(_, line)| *line)
            .collect();

        // Write to temp file and reload
        let new_content = if new_lines.is_empty() {
            String::new()
        } else {
            new_lines.join("\n") + "\n"
        };
        std::fs::write(&self.temp_file_path, &new_content)?;
        self.dirty = true;
        self.reload_from_temp()?;

        // Adjust selection
        let min_deleted = indices.iter().min().copied().unwrap_or(0);
        self.selected_index = min_deleted.min(self.entries.len().saturating_sub(1));

        // Clear selection
        self.clear_selection();

        self.message = Some(format!("{} entry(s) deleted", count));

        Ok(())
    }

    /// Refresh entries from file
    pub fn refresh(&mut self) -> Result<()> {
        self.file_content = crate::utils::path::read_file(&self.file_path)?;
        let parser = crate::parser::get_parser(self.shell_type);
        let parse_result = parser.parse(&self.file_content);
        self.entries = parse_result.entries;

        // Try to keep selection on same entry by name
        // If not found, reset to 0
        if self.selected_index >= self.entries.len() {
            self.selected_index = 0;
        }

        Ok(())
    }

    /// Format the configuration file
    fn format_file(&mut self) -> Result<()> {
        let config = crate::config::load_or_create_config()?;
        let formatter = crate::formatter::get_formatter(self.shell_type);
        let formatted = formatter.format(&self.entries, &config);

        // Create backup before writing
        let backup_manager = crate::backup::BackupManager::new(self.shell_type, &config);
        backup_manager.create_backup(&self.file_path)?;

        // Write formatted content
        std::fs::write(&self.file_path, formatted)?;

        // Refresh entries
        self.refresh()?;
        self.message = Some("File formatted successfully".to_string());

        Ok(())
    }

    /// Check the current entry or all entries
    fn check_entry(&mut self) -> Result<()> {
        use crate::checker::{Checker, DuplicateChecker};

        let checker = DuplicateChecker;
        let result = checker.check(&self.entries);

        if result.issues.is_empty() {
            self.message = Some("No issues found".to_string());
        } else {
            self.message = Some(format!("Found {} issue(s)", result.issues.len()));
        }

        Ok(())
    }

    /// Start adding a new entry (show type selection menu)
    fn start_adding_entry(&mut self) {
        self.mode = AppMode::SelectingType;
        self.type_selection_index = 0;
        self.type_list_scroll_offset = 0;
        self.message = Some("Select entry type:".to_string());
    }

    /// Start editing the selected entry
    fn start_editing(&mut self) {
        if let Some(entry) = self.get_selected_entry() {
            let name = entry.name.clone();
            let value = entry.value.clone();
            let entry_type = entry.entry_type;

            // For Source/Code/Comment, start at Value field (skip Name)
            let (start_field, cursor_pos, cursor_col) = if matches!(
                entry_type,
                EntryType::Source | EntryType::Code | EntryType::Comment
            ) {
                (EditField::Value, value.len(), value.len())
            } else {
                (EditField::Name, name.len(), name.len())
            };

            self.edit_state = Some(EditState {
                field: start_field,
                name_buffer: name,
                value_buffer: value,
                entry_type,
                is_new: false,
                cursor_position: cursor_pos,
                cursor_row: 0,
                cursor_col,
                scroll_offset: 0,
            });
            self.mode = AppMode::Editing;
            self.message = None;
        }
    }

    /// Start moving the selected entry
    fn start_moving(&mut self) {
        if !self.entries.is_empty() {
            self.mode = AppMode::Moving;
            self.move_original_index = Some(self.selected_index);
            self.message = Some("Use ↑/↓ to move, Enter to confirm, Esc to cancel".to_string());
        }
    }

    /// Cancel editing
    fn cancel_editing(&mut self) {
        self.edit_state = None;
        self.mode = AppMode::Normal;
        self.message = None;
    }

    /// Submit editing (save changes)
    /// Directly writes to temp file, then reloads
    fn submit_editing(&mut self) -> Result<()> {
        let Some(state) = self.edit_state.take() else {
            self.mode = AppMode::Normal;
            return Ok(());
        };

        // For Source/Code, name can be empty (auto-generated)
        let skip_name_validation = matches!(state.entry_type, EntryType::Source | EntryType::Code);

        // Validate name for types that need it
        if !skip_name_validation && state.name_buffer.trim().is_empty() {
            self.edit_state = Some(state);
            self.message = Some("Name cannot be empty".to_string());
            return Ok(());
        }

        // Read current content
        let content = self.read_current_content()?;
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        if state.is_new {
            // Get insert line number (after current entry's last line)
            let insert_line = if let Some(current) = self.entries.get(self.selected_index) {
                current
                    .end_line
                    .or(current.line_number)
                    .unwrap_or(0)
                    .saturating_add(1)
            } else {
                1
            };

            // Format the new entry
            let new_text = self.format_new_entry(&state);

            // Insert at the specified line (0-indexed)
            let insert_idx = insert_line.saturating_sub(1).min(lines.len());

            // Handle multi-line entries
            for (i, new_line) in new_text.lines().enumerate() {
                lines.insert(insert_idx + i, new_line.to_string());
            }

            // Write to temp file and reload
            let new_content = lines.join("\n") + "\n";
            std::fs::write(&self.temp_file_path, &new_content)?;
            self.dirty = true;
            self.reload_from_temp()?;

            // Select the newly inserted entry
            self.select_entry_at_line(insert_line);
            self.message = Some("Entry added successfully".to_string());
        } else {
            // Update existing entry - replace lines in range
            if let Some(entry) = self.entries.get(self.selected_index) {
                let start = entry.line_number.unwrap_or(1).saturating_sub(1); // 0-indexed
                let end = entry
                    .end_line
                    .unwrap_or(entry.line_number.unwrap_or(1))
                    .saturating_sub(1);

                // Format the updated entry
                let new_text = self.format_new_entry(&state);
                let new_lines: Vec<&str> = new_text.lines().collect();

                // Remove old lines
                let remove_count = (end - start + 1).min(lines.len() - start);
                for _ in 0..remove_count {
                    if start < lines.len() {
                        lines.remove(start);
                    }
                }

                // Insert new lines
                for (i, new_line) in new_lines.iter().enumerate() {
                    lines.insert(start + i, new_line.to_string());
                }

                let entry_line = entry.line_number.unwrap_or(1);

                // Write to temp file and reload
                let new_content = lines.join("\n") + "\n";
                std::fs::write(&self.temp_file_path, &new_content)?;
                self.dirty = true;
                self.reload_from_temp()?;

                // Re-select the entry
                self.select_entry_at_line(entry_line);
            }
            self.message = Some("Entry updated successfully".to_string());
        }

        self.mode = AppMode::Normal;
        Ok(())
    }

    /// Generate file content from current entries
    fn generate_file_content(&self) -> String {
        let formatter = crate::formatter::get_formatter(self.shell_type);

        // Sort entries by line number
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by_key(|e| e.line_number.unwrap_or(0));

        let mut lines: Vec<String> = Vec::new();

        for entry in sorted {
            // For Code/Comment, use raw_line; for others, use formatter
            let formatted = match entry.entry_type {
                EntryType::Code | EntryType::Comment => entry
                    .raw_line
                    .clone()
                    .unwrap_or_else(|| entry.value.clone()),
                _ => formatter.format_entry(entry),
            };
            lines.push(formatted);
        }

        lines.join("\n") + "\n"
    }

    /// Save entries to temp file (preserves original order, no reformatting)
    /// After writing, reloads from temp to ensure entries match file content
    fn save_to_temp(&mut self) -> Result<()> {
        let content = self.generate_file_content();
        std::fs::write(&self.temp_file_path, &content)?;
        self.dirty = true;
        self.reload_from_temp()?;
        Ok(())
    }

    /// Reload entries from temp file by re-parsing
    /// This ensures entries always reflect the actual file content
    fn reload_from_temp(&mut self) -> Result<()> {
        if !self.temp_file_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.temp_file_path)?;
        let parser = crate::parser::get_parser(self.shell_type);
        let parse_result = parser.parse(&content);
        self.entries = parse_result.entries;

        // Adjust selection to avoid out-of-bounds
        if self.selected_index >= self.entries.len() {
            self.selected_index = self.entries.len().saturating_sub(1);
        }

        Ok(())
    }

    /// Save entries to original file (with backup)
    fn save_to_original_file(&mut self) -> Result<()> {
        // Create backup
        let config = crate::config::load_or_create_config()?;
        let backup_manager = crate::backup::BackupManager::new(self.shell_type, &config);
        backup_manager.create_backup(&self.file_path)?;

        // Generate content and write
        let content = self.generate_file_content();
        std::fs::write(&self.file_path, &content)?;

        // Clean up temp file and reset dirty flag
        self.cleanup_temp_file();
        self.dirty = false;

        self.message = Some("File saved".to_string());
        Ok(())
    }

    /// Clean up temp file
    fn cleanup_temp_file(&self) {
        if self.temp_file_path.exists() {
            let _ = std::fs::remove_file(&self.temp_file_path);
        }
    }

    /// Read current content from temp file or original file
    fn read_current_content(&self) -> Result<String> {
        if self.temp_file_path.exists() {
            Ok(std::fs::read_to_string(&self.temp_file_path)?)
        } else {
            Ok(std::fs::read_to_string(&self.file_path)?)
        }
    }

    /// Remove comment prefix from a line, preserving leading whitespace
    fn remove_comment_prefix(&self, line: &str) -> String {
        if let Some(first_non_ws) = line.find(|c: char| !c.is_whitespace()) {
            let (prefix, rest) = line.split_at(first_non_ws);
            if let Some(stripped) = rest.strip_prefix("# ") {
                format!("{}{}", prefix, stripped)
            } else if let Some(stripped) = rest.strip_prefix("#") {
                format!("{}{}", prefix, stripped)
            } else {
                line.to_string()
            }
        } else {
            line.to_string()
        }
    }

    /// Format a new entry for insertion into the file
    fn format_new_entry(&self, state: &EditState) -> String {
        match state.entry_type {
            EntryType::Code | EntryType::Comment => state.value_buffer.clone(),
            _ => {
                let formatter = crate::formatter::get_formatter(self.shell_type);
                let entry = Entry::new(
                    state.entry_type,
                    state.name_buffer.trim().to_string(),
                    state.value_buffer.clone(),
                );
                formatter.format_entry(&entry)
            }
        }
    }

    /// Select entry at a specific line number
    fn select_entry_at_line(&mut self, line: usize) {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.line_number == Some(line) {
                self.selected_index = i;
                return;
            }
        }
        // If not found, keep current selection but bound it
        if self.selected_index >= self.entries.len() {
            self.selected_index = self.entries.len().saturating_sub(1);
        }
    }

    /// Clear multi-selection
    fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selected_range = None;
    }

    /// Extend selection upward (Shift+Up)
    fn extend_selection_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Initialize anchor if not set
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.selected_index);
        }

        // Move up
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }

        // Update selection range
        let anchor = self.selection_anchor.unwrap();
        let min = self.selected_index.min(anchor);
        let max = self.selected_index.max(anchor);
        self.selected_range = Some((min, max));

        self.adjust_scroll_for_selection();
    }

    /// Extend selection downward (Shift+Down)
    fn extend_selection_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Initialize anchor if not set
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.selected_index);
        }

        // Move down
        if self.selected_index < self.entries.len() - 1 {
            self.selected_index += 1;
        }

        // Update selection range
        let anchor = self.selection_anchor.unwrap();
        let min = self.selected_index.min(anchor);
        let max = self.selected_index.max(anchor);
        self.selected_range = Some((min, max));

        self.adjust_scroll_for_selection();
    }

    /// Legacy save_to_file - now calls save_to_temp for backward compatibility
    fn save_to_file(&mut self) -> Result<()> {
        self.save_to_temp()
    }

    /// Toggle the selected entry between Comment and its original type
    /// Uses pure text manipulation (add/remove # prefix per line)
    /// Supports multi-select
    fn toggle_comment(&mut self) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        let indices = self.get_selected_indices();
        let count = indices.len();

        // Read current file content
        let content = self.read_current_content()?;
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // Process each selected entry - modify lines directly
        for idx in &indices {
            if let Some(entry) = self.entries.get(*idx) {
                let start = entry.line_number.unwrap_or(1).saturating_sub(1); // 0-indexed
                let end = entry
                    .end_line
                    .unwrap_or(entry.line_number.unwrap_or(1))
                    .saturating_sub(1);

                let is_comment = entry.entry_type == EntryType::Comment;

                for line_idx in start..=end.min(lines.len().saturating_sub(1)) {
                    if is_comment {
                        // Remove "# " or "#" prefix
                        lines[line_idx] = self.remove_comment_prefix(&lines[line_idx]);
                    } else {
                        // Add "# " prefix
                        lines[line_idx] = format!("# {}", lines[line_idx]);
                    }
                }
            }
        }

        // Write to temp file and reload
        let new_content = lines.join("\n") + "\n";
        std::fs::write(&self.temp_file_path, &new_content)?;
        self.dirty = true;
        self.reload_from_temp()?;

        self.clear_selection();
        self.message = Some(if count > 1 {
            format!("{} entries toggled", count)
        } else {
            "Entry toggled".to_string()
        });

        Ok(())
    }
}

/// Find a valid character boundary at or before the given byte position
/// This ensures safe string slicing by backing up to a valid boundary
fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    if s.is_char_boundary(pos) {
        return pos;
    }
    // Back up to find a valid boundary
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Find the previous character boundary (for backspace/left movement)
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos - 1;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Find the next character boundary (for delete/right movement)
fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos + 1;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}
