//! TUI application state and logic

use std::io;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind},
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
    Searching, // Search mode
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    ConfirmQuit,           // Confirm quit with unsaved changes
    ConfirmFormat,         // Confirm format with preview
    ConfirmSaveWithErrors, // Confirm save despite shell validation errors
    SelectingType,         // For [a]Add - selecting entry type
    Editing,               // Unified editing mode
    Moving,                // Moving entry up/down
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

/// Format preview information
#[derive(Debug, Clone)]
pub struct FormatPreview {
    pub summary: Vec<String>,      // Summary lines to display
    pub formatted_content: String, // Full formatted content ready to apply
    pub scroll_offset: usize,      // Scroll offset for preview display
}

impl FormatPreview {
    pub fn new(summary: Vec<String>, formatted_content: String) -> Self {
        Self {
            summary,
            formatted_content,
            scroll_offset: 0,
        }
    }
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

    // Format preview state
    pub format_preview: Option<FormatPreview>,

    // Type selection for Add
    pub type_selection_index: usize,
    pub type_list_scroll_offset: usize,

    // Move mode - no longer needed (using reload on cancel)

    // Detail popup scroll offset
    pub detail_scroll: usize,

    // Delete confirm popup scroll offset (for viewing entry details before deletion)
    pub delete_confirm_scroll: usize,

    // i18n
    pub messages: &'static Messages,

    // Dirty flag and temp file for unsaved changes
    pub dirty: bool,
    pub temp_file_path: PathBuf,

    // Multi-selection state
    pub selection_anchor: Option<usize>,

    // Non-contiguous selection state (also used for contiguous selection)
    pub non_contiguous_mode: bool,
    pub selected_indices: std::collections::HashSet<usize>,

    // Move mode state - save original selection for cancel
    pub pre_move_selected_indices: Option<std::collections::HashSet<usize>>,
    pub pre_move_selection_anchor: Option<Option<usize>>,

    // Clipboard state (internal buffer, not system clipboard)
    pub clipboard_buffer: Option<String>,

    // Shell validation state
    pub validation_errors: Option<String>,
    pub validation_scroll_offset: usize,

    // Undo/Redo state (stores full temp file content snapshots)
    pub undo_stack: Vec<String>,
    pub redo_stack: Vec<String>,
    pub max_undo_history: usize,

    // Search state
    pub search_query: String,       // Search query (persistent)
    pub search_active: bool,        // Search mode active
    pub search_matches: Vec<usize>, // Matched entry indices
    pub search_cursor: usize,       // Cursor position in search input

    // Full redraw flag (set after external editor to clear artifacts)
    pub needs_full_redraw: bool,
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

        // Initialize temp file with original content at startup
        std::fs::write(&temp_file_path, &file_content)?;

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
            format_preview: None,
            type_selection_index: 0,
            type_list_scroll_offset: 0,

            detail_scroll: 0,
            delete_confirm_scroll: 0,
            messages,
            dirty: false,
            temp_file_path,
            selection_anchor: None,
            non_contiguous_mode: false,
            selected_indices: std::collections::HashSet::new(),
            pre_move_selected_indices: None,
            pre_move_selection_anchor: None,
            clipboard_buffer: None,
            validation_errors: None,
            validation_scroll_offset: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo_history: 50,
            search_query: String::new(),
            search_active: false,
            search_matches: Vec::new(),
            search_cursor: 0,
            needs_full_redraw: false,
        })
    }

    /// Run the TUI application
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the main loop
        let result = self.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop
    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            // Force full redraw if flag is set (e.g., after external editor)
            if self.needs_full_redraw {
                terminal.clear()?;
                self.needs_full_redraw = false;
            }

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
                        self.move_selection_up();
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
                    AppMode::ConfirmFormat => {
                        if let Some(ref mut preview) = self.format_preview {
                            preview.scroll_offset = preview.scroll_offset.saturating_sub(3);
                        }
                    }
                    AppMode::SelectingType => {
                        if self.type_selection_index > 0 {
                            self.type_selection_index -= 1;
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
                        self.move_selection_down();
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
                    AppMode::ConfirmFormat => {
                        if let Some(ref mut preview) = self.format_preview {
                            preview.scroll_offset = preview.scroll_offset.saturating_add(3);
                        }
                    }
                    AppMode::SelectingType => {
                        const NUM_TYPES: usize = 5;
                        if self.type_selection_index < NUM_TYPES - 1 {
                            self.type_selection_index += 1;
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
            AppMode::Searching => self.handle_searching_mode(key.code)?,
            AppMode::ShowingDetail => self.handle_detail_mode(key.code)?,
            AppMode::ShowingHelp => self.handle_help_mode(key.code)?,
            AppMode::ConfirmDelete => self.handle_confirm_delete_mode(key.code)?,
            AppMode::ConfirmQuit => self.handle_confirm_quit_mode(key.code)?,
            AppMode::ConfirmFormat => self.handle_confirm_format_mode(key.code)?,
            AppMode::ConfirmSaveWithErrors => {
                self.handle_confirm_save_with_errors_mode(key.code)?
            }
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
        let has_alt = key.modifiers.contains(KeyModifiers::ALT);

        match key.code {
            // Ctrl+S or w: Save to original file
            KeyCode::Char('s') if has_ctrl => {
                self.save_to_original_file()?;
            }
            KeyCode::Char('w') if !has_ctrl && !has_shift && !has_alt => {
                self.save_to_original_file()?;
            }
            // Ctrl+C or Alt+C: Copy selected entries
            KeyCode::Char('c') if has_ctrl => {
                self.copy_selected()?;
            }
            KeyCode::Char('c') if has_alt => {
                self.copy_selected()?;
            }
            // Ctrl+V or Alt+V: Paste from clipboard
            KeyCode::Char('v') if has_ctrl => {
                self.paste_entry()?;
            }
            KeyCode::Char('v') if has_alt => {
                self.paste_entry()?;
            }
            // Ctrl+Z: Undo
            KeyCode::Char('z') if has_ctrl => {
                self.undo()?;
            }
            // Ctrl+Y: Redo
            KeyCode::Char('y') if has_ctrl => {
                self.redo()?;
            }
            // s: Enter selection mode and toggle current item
            KeyCode::Char('s') if !has_ctrl && !has_shift && !has_alt => {
                // Enter selection mode (if not already in)
                self.non_contiguous_mode = true;

                // Toggle current item in/out of selection
                if self.selected_indices.contains(&self.selected_index) {
                    self.selected_indices.remove(&self.selected_index);
                } else {
                    self.selected_indices.insert(self.selected_index);
                }
            }
            // Quit
            KeyCode::Char('q') => {
                if self.dirty {
                    self.mode = AppMode::ConfirmQuit;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Esc => {
                if self.search_active {
                    // Close search mode: clear highlights and search state
                    self.search_active = false;
                    self.search_query.clear();
                    self.search_matches.clear();
                    self.search_cursor = 0;
                } else if self.non_contiguous_mode || !self.selected_indices.is_empty() {
                    // Exit selection mode and clear all selections
                    self.clear_selection();
                    self.message = Some(self.messages.tui_msg_selection_cleared.to_string());
                } else {
                    // When not in selection mode, behave same as 'q'
                    if self.dirty {
                        self.mode = AppMode::ConfirmQuit;
                    } else {
                        self.should_quit = true;
                    }
                }
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::ShowingHelp;
            }
            // Navigation with Shift for multi-select
            KeyCode::Up | KeyCode::Char('k') => {
                if has_shift {
                    // Enter selection mode and extend selection
                    self.non_contiguous_mode = true;
                    self.extend_selection_up();
                } else {
                    // Move cursor without clearing selection (unified selection mode)
                    self.move_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if has_shift {
                    // Enter selection mode and extend selection
                    self.non_contiguous_mode = true;
                    self.extend_selection_down();
                } else {
                    // Move cursor without clearing selection (unified selection mode)
                    self.move_down();
                }
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                self.clear_selection();
                self.mode = AppMode::ShowingDetail;
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                self.delete_confirm_scroll = 0;
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
                if self.search_active && !self.search_matches.is_empty() {
                    self.jump_to_prev_match();
                } else {
                    self.page_up();
                }
            }
            KeyCode::PageDown => {
                self.clear_selection();
                if self.search_active && !self.search_matches.is_empty() {
                    self.jump_to_next_match();
                } else {
                    self.page_down();
                }
            }
            KeyCode::Char('f') => {
                self.non_contiguous_mode = false;
                self.selected_indices.clear();
                self.clear_selection();
                self.start_search();
            }
            KeyCode::Char('r') => {
                self.format_file()?;
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
                // Don't clear selection - support moving selected block
                self.start_moving();
            }
            KeyCode::Char('t') => {
                self.toggle_comment()?;
            }
            KeyCode::Char('o') => {
                self.open_temp_file_in_editor()?;
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
            // Scroll support for viewing entry details before deletion
            KeyCode::Up | KeyCode::Char('k') => {
                self.delete_confirm_scroll = self.delete_confirm_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.delete_confirm_scroll = self.delete_confirm_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.delete_confirm_scroll = self.delete_confirm_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.delete_confirm_scroll = self.delete_confirm_scroll.saturating_add(10);
            }
            KeyCode::Home => {
                self.delete_confirm_scroll = 0;
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

    /// Handle keys in confirm format mode
    fn handle_confirm_format_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.apply_format()?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.cancel_format();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(ref mut preview) = self.format_preview {
                    preview.scroll_offset = preview.scroll_offset.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(ref mut preview) = self.format_preview {
                    preview.scroll_offset = preview.scroll_offset.saturating_add(1);
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut preview) = self.format_preview {
                    preview.scroll_offset = preview.scroll_offset.saturating_sub(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut preview) = self.format_preview {
                    preview.scroll_offset = preview.scroll_offset.saturating_add(10);
                }
            }
            KeyCode::Home => {
                if let Some(ref mut preview) = self.format_preview {
                    preview.scroll_offset = 0;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle keys in confirm save with errors mode
    fn handle_confirm_save_with_errors_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                // User confirmed, force save without validation
                self.validation_errors = None;
                self.validation_scroll_offset = 0;

                // Check if this is from format or normal save
                if self.format_preview.is_some() {
                    // Apply format without validation
                    self.force_apply_format()?;
                } else {
                    self.force_save_to_original_file()?;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // Cancel save/format
                self.validation_errors = None;
                self.validation_scroll_offset = 0;

                if self.format_preview.is_some() {
                    // Return to format preview mode
                    self.mode = AppMode::ConfirmFormat;
                    self.message = Some(
                        self.messages
                            .tui_msg_format_cancelled_validation
                            .to_string(),
                    );
                } else {
                    self.mode = AppMode::Normal;
                    self.message =
                        Some(self.messages.tui_msg_save_cancelled_validation.to_string());
                }
            }
            // Scroll through error message
            KeyCode::Up | KeyCode::Char('k') => {
                self.validation_scroll_offset = self.validation_scroll_offset.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.validation_scroll_offset = self.validation_scroll_offset.saturating_add(1);
            }
            KeyCode::PageUp => {
                self.validation_scroll_offset = self.validation_scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.validation_scroll_offset = self.validation_scroll_offset.saturating_add(10);
            }
            KeyCode::Home => {
                self.validation_scroll_offset = 0;
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

    /// Get entry template with initial cursor position
    /// Returns: (template_string, cursor_position, cursor_row, cursor_col)
    fn get_entry_template(&self, entry_type: &EntryType) -> (String, usize, usize, usize) {
        match self.shell_type {
            ShellType::PowerShell => match entry_type {
                EntryType::Alias => ("Set-Alias  ''".to_string(), 10, 0, 10),
                EntryType::Function => ("function  {\n    \n}".to_string(), 9, 0, 9),
                EntryType::EnvVar => ("$env: = ''".to_string(), 5, 0, 5),
                EntryType::Source => (". ".to_string(), 2, 0, 2),
                EntryType::Comment => ("# ".to_string(), 2, 0, 2),
                EntryType::Code => (String::new(), 0, 0, 0),
            },
            _ => match entry_type {
                // Bash/Zsh
                EntryType::Alias => ("alias =''".to_string(), 6, 0, 6),
                EntryType::Function => ("() {\n    \n}".to_string(), 0, 0, 0),
                EntryType::EnvVar => ("export =''".to_string(), 7, 0, 7),
                EntryType::Source => ("source ".to_string(), 7, 0, 7),
                EntryType::Comment => ("# ".to_string(), 2, 0, 2),
                EntryType::Code => (String::new(), 0, 0, 0),
            },
        }
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

        // Get template for the selected entry type
        let (value_buffer, cursor_position, cursor_row, cursor_col) =
            self.get_entry_template(&entry_type);

        // All entry types start at Value field (Name field is skipped)
        let start_field = EditField::Value;

        // Create edit state for new entry with template
        self.edit_state = Some(EditState {
            field: start_field,
            name_buffer: String::new(),
            value_buffer, // Use template instead of empty string
            entry_type,
            is_new: true,
            cursor_position, // Position cursor intelligently
            cursor_row,
            cursor_col,
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
                // All entry types skip the Name field
                state.field = state.field.next_skip_name();
            }
            KeyCode::BackTab => {
                // All entry types skip the Name field
                state.field = state.field.prev_skip_name();
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
                    // All entry types allow multi-line (trailing blanks are part of value)
                    // For PowerShell Alias, keep single-line behavior
                    let is_single_line = if self.shell_type == ShellType::PowerShell {
                        state.entry_type == EntryType::Alias
                    } else {
                        false // Bash/Zsh allow multi-line for all types
                    };

                    if is_single_line {
                        // Jump to Submit button (user needs to press Enter again to confirm)
                        state.field = EditField::Submit;
                    } else {
                        // Insert newline in value field for multi-line types
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
                        // Ensure cursor is at a valid character boundary
                        let safe_pos =
                            find_char_boundary(&state.name_buffer, state.cursor_position);
                        state.name_buffer.insert(safe_pos, c);
                        state.cursor_position = safe_pos + c.len_utf8();
                    }
                    EditField::Value => {
                        // Ensure cursor is at a valid character boundary
                        let safe_pos =
                            find_char_boundary(&state.value_buffer, state.cursor_position);
                        state.value_buffer.insert(safe_pos, c);
                        state.cursor_position = safe_pos + c.len_utf8();
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
                        // Ensure cursor_position doesn't exceed buffer length
                        let safe_cursor = state.cursor_position.min(state.value_buffer.len());
                        let removed: String =
                            state.value_buffer.drain(new_pos..safe_cursor).collect();
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

        // Use split('\n') to preserve trailing empty lines (separator format)
        let lines: Vec<&str> = state.value_buffer.split('\n').collect();

        if state.cursor_row == 0 {
            return;
        }

        let target_row = state.cursor_row - 1;
        let target_line = lines.get(target_row).unwrap_or(&"");

        // Calculate target column in characters (not bytes)
        let target_line_chars = target_line.chars().count();
        let target_col_chars = state.cursor_col.min(target_line_chars);

        // Convert character offset to byte offset
        let target_col_bytes = target_line
            .char_indices()
            .nth(target_col_chars)
            .map(|(i, _)| i)
            .unwrap_or(target_line.len());

        // Calculate new cursor position in bytes
        let mut new_pos = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == target_row {
                new_pos += target_col_bytes;
                break;
            }
            new_pos += line.len() + 1; // +1 for newline
        }

        // Ensure the position is at a valid character boundary
        new_pos = new_pos.min(state.value_buffer.len());
        new_pos = find_char_boundary(&state.value_buffer, new_pos);

        state.cursor_position = new_pos;
        state.cursor_row = target_row;
        state.cursor_col = target_col_chars;
    }

    /// Move cursor down in multi-line value field
    fn move_cursor_down_in_value(&mut self) {
        let Some(ref mut state) = self.edit_state else {
            return;
        };

        // Use split('\n') to preserve trailing empty lines (separator format)
        let lines: Vec<&str> = state.value_buffer.split('\n').collect();
        // Check for trailing newline - lines() doesn't count it
        let has_trailing_newline = state.value_buffer.ends_with('\n');
        let line_count = if has_trailing_newline {
            lines.len() + 1 // +1 for the empty line after trailing newline
        } else {
            lines.len().max(1)
        };

        if state.cursor_row >= line_count - 1 {
            return;
        }

        let target_row = state.cursor_row + 1;
        let target_line = if target_row >= lines.len() {
            "" // Empty line after trailing newline
        } else {
            lines.get(target_row).unwrap_or(&"")
        };

        // Calculate target column in characters (not bytes)
        let target_line_chars = target_line.chars().count();
        let target_col_chars = state.cursor_col.min(target_line_chars);

        // Convert character offset to byte offset
        let target_col_bytes = target_line
            .char_indices()
            .nth(target_col_chars)
            .map(|(i, _)| i)
            .unwrap_or(target_line.len());

        // Calculate new cursor position in bytes
        let mut new_pos = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == target_row {
                new_pos += target_col_bytes;
                break;
            }
            new_pos += line.len() + 1; // +1 for newline
        }
        // If target_row is beyond lines (trailing newline case)
        if target_row >= lines.len() {
            // Position is at the end of content
            for line in lines.iter() {
                new_pos += line.len() + 1;
            }
            new_pos = new_pos.min(state.value_buffer.len());
        }

        // Ensure the position is at a valid character boundary
        new_pos = new_pos.min(state.value_buffer.len());
        new_pos = find_char_boundary(&state.value_buffer, new_pos);

        state.cursor_position = new_pos;
        state.cursor_row = target_row;
        state.cursor_col = target_col_chars;
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
                self.move_selection_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection_down();
            }
            KeyCode::Enter => {
                // Confirm move - write to file using line-based cut-paste
                self.confirm_move()?;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                // Cancel move - undo to restore original positions
                self.undo()?;

                // Restore original selection state
                if let Some(indices) = self.pre_move_selected_indices.take() {
                    self.selected_indices = indices;
                }
                if let Some(anchor) = self.pre_move_selection_anchor.take() {
                    self.selection_anchor = anchor;
                }

                self.mode = AppMode::Normal;
                self.message = Some(self.messages.tui_msg_move_cancelled.to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Get sorted indices of selected entries (or current index if no selection)
    fn get_selected_indices_sorted(&self) -> Vec<usize> {
        if !self.selected_indices.is_empty() {
            // Multi-selection: get sorted indices
            let mut indices: Vec<_> = self.selected_indices.iter().copied().collect();
            indices.sort_unstable();
            indices
        } else {
            // Single selection: just current index
            vec![self.selected_index]
        }
    }

    /// Move selected block up (buffer-level operation for preview)
    fn move_selection_up(&mut self) {
        let indices = self.get_selected_indices_sorted();
        if indices.is_empty() || indices[0] == 0 {
            return; // Already at top
        }

        let first = indices[0];
        let count = indices.len();

        // Extract selected entries (cut)
        let mut extracted: Vec<Entry> = indices
            .iter()
            .rev()
            .map(|&i| self.entries.remove(i))
            .collect();
        extracted.reverse();

        // Insert above (paste)
        let insert_pos = first - 1;
        for (offset, entry) in extracted.into_iter().enumerate() {
            self.entries.insert(insert_pos + offset, entry);
        }

        // Update selected indices (shift all indices up by 1)
        if !self.selected_indices.is_empty() {
            let new_indices: std::collections::HashSet<usize> = self
                .selected_indices
                .iter()
                .map(|&i| i.saturating_sub(1))
                .collect();
            self.selected_indices = new_indices;
            self.selection_anchor = self.selection_anchor.map(|a| a.saturating_sub(1));
        }

        // Update cursor position
        if self.selected_index >= first && self.selected_index < first + count {
            self.selected_index = self.selected_index.saturating_sub(1);
        }

        self.adjust_scroll_for_selection();
    }

    /// Move selected block down (buffer-level operation for preview)
    fn move_selection_down(&mut self) {
        let indices = self.get_selected_indices_sorted();
        if indices.is_empty() || *indices.last().unwrap() >= self.entries.len() - 1 {
            return; // Already at bottom
        }

        let first = indices[0];
        let count = indices.len();

        // Extract selected entries (cut)
        let mut extracted: Vec<Entry> = indices
            .iter()
            .rev()
            .map(|&i| self.entries.remove(i))
            .collect();
        extracted.reverse();

        // Insert below (paste)
        let insert_pos = first + 1;
        for (offset, entry) in extracted.into_iter().enumerate() {
            self.entries.insert(insert_pos + offset, entry);
        }

        // Update selected indices (shift all indices down by 1)
        if !self.selected_indices.is_empty() {
            let new_indices: std::collections::HashSet<usize> =
                self.selected_indices.iter().map(|&i| i + 1).collect();
            self.selected_indices = new_indices;
            self.selection_anchor = self.selection_anchor.map(|a| a + 1);
        }

        // Update cursor position
        if self.selected_index >= first && self.selected_index < first + count {
            self.selected_index += 1;
        }

        self.adjust_scroll_for_selection();
    }

    /// Confirm move: regenerate file from current entry order using formatter
    /// This avoids issues with outdated line numbers by rebuilding the entire file
    fn confirm_move(&mut self) -> Result<()> {
        let indices = self.get_selected_indices_sorted();
        if indices.is_empty() {
            self.mode = AppMode::Normal;
            return Ok(());
        }

        // Get formatter for current shell type
        let formatter = crate::formatter::get_formatter(self.shell_type);

        // Rebuild file content from current entries order
        let mut new_lines: Vec<String> = Vec::new();
        for entry in &self.entries {
            let formatted = formatter.format_entry(entry);
            new_lines.push(formatted);
        }

        let new_content = new_lines.join("\n") + "\n";
        self.write_temp_with_undo(&new_content)?;
        self.reload_from_temp()?;

        self.mode = AppMode::Normal;

        // Clear saved selection state (move confirmed, don't need to restore)
        self.pre_move_selected_indices = None;
        self.pre_move_selection_anchor = None;

        self.clear_selection();
        self.message = Some(
            self.messages
                .tui_msg_moved_entries
                .replace("{}", &indices.len().to_string()),
        );

        Ok(())
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

    /// Get all selected entries (for multi-select operations)
    pub fn get_selected_entries(&self) -> Vec<&Entry> {
        self.get_selected_indices()
            .iter()
            .filter_map(|&i| self.entries.get(i))
            .collect()
    }

    /// Get all selected indices (for multi-select operations)
    fn get_selected_indices(&self) -> Vec<usize> {
        if !self.selected_indices.is_empty() {
            // Multi-selection (continuous or non-contiguous)
            let mut indices: Vec<usize> = self.selected_indices.iter().copied().collect();
            indices.sort_unstable();
            indices
        } else {
            // Single selection
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
        self.write_temp_with_undo(&new_content)?;
        self.reload_from_temp()?;

        // Adjust selection
        let min_deleted = indices.iter().min().copied().unwrap_or(0);
        self.selected_index = min_deleted.min(self.entries.len().saturating_sub(1));

        // Clear selection
        self.clear_selection();

        self.message = Some(
            self.messages
                .tui_msg_entries_deleted
                .replace("{}", &count.to_string()),
        );

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

    /// Format the configuration file (with preview)
    fn format_file(&mut self) -> Result<()> {
        self.preview_format()?;
        Ok(())
    }

    /// Generate format preview
    fn preview_format(&mut self) -> Result<()> {
        use crate::checker::{check_all, Severity};
        use crate::utils::path_merge;

        let config = crate::config::load_or_create_config()?;

        // Clone entries and merge PATH if needed
        let mut entries_to_format = self.entries.clone();
        let mut path_merge_info: Option<path_merge::PathMergeResult> = None;

        // Check for PATH merging
        let path_entries: Vec<&Entry> = entries_to_format
            .iter()
            .filter(|e| e.entry_type == EntryType::EnvVar && e.name.to_uppercase() == "PATH")
            .collect();

        if let Some(merge_result) = path_merge::merge_path_definitions(&path_entries) {
            // Remove all PATH entries and add merged one
            entries_to_format.retain(|e| {
                !(e.entry_type == EntryType::EnvVar && e.name.to_uppercase() == "PATH")
            });

            let merged_entry = Entry::new(
                EntryType::EnvVar,
                "PATH".to_string(),
                merge_result.merged_value.clone(),
            )
            .with_line_number(merge_result.source_lines.first().copied().unwrap_or(0));

            entries_to_format.push(merged_entry);
            path_merge_info = Some(merge_result);
        }

        let formatter = crate::formatter::get_formatter(self.shell_type);
        let formatted = formatter.format(&entries_to_format, &config);

        // Build summary
        let mut summary = Vec::new();

        // 1. Check for duplicates
        let check_result = check_all(&self.entries);
        if !check_result.issues.is_empty() {
            summary.push(format!("⚠ Found {} issues:", check_result.issues.len()));
            for issue in check_result.issues.iter().take(10) {
                let prefix = match issue.severity {
                    Severity::Warning => "  •",
                    Severity::Error => "  ✗",
                };
                summary.push(format!("{} {}", prefix, issue.message));
            }
            if check_result.issues.len() > 10 {
                summary.push(format!("  ... and {} more", check_result.issues.len() - 10));
            }
            summary.push(String::new());
        }

        // 2. Show PATH merging info
        if let Some(merge_info) = path_merge_info {
            summary.push(format!(
                "✓ Merging {} PATH definitions (lines: {})",
                merge_info.source_lines.len(),
                merge_info
                    .source_lines
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            summary.push(format!("  → {}", merge_info.merged_value));
            summary.push(String::new());
        }

        // 3. Count entries by type
        let mut alias_count = 0;
        let mut func_count = 0;
        let mut env_count = 0;
        let mut source_count = 0;

        for entry in &self.entries {
            match entry.entry_type {
                EntryType::Alias => alias_count += 1,
                EntryType::Function => func_count += 1,
                EntryType::EnvVar => env_count += 1,
                EntryType::Source => source_count += 1,
                _ => {}
            }
        }

        if config.format.sort_alphabetically {
            if alias_count > 0 {
                summary.push(
                    self.messages
                        .tui_fmt_sorting_aliases
                        .replace("{}", &alias_count.to_string()),
                );
            }
            if func_count > 0 {
                summary.push(
                    self.messages
                        .tui_fmt_sorting_functions
                        .replace("{}", &func_count.to_string()),
                );
            }
            if env_count > 0 {
                summary.push(
                    self.messages
                        .tui_fmt_sorting_envvars
                        .replace("{}", &env_count.to_string()),
                );
            }
            if source_count > 0 {
                summary.push(
                    self.messages
                        .tui_fmt_sorting_sources
                        .replace("{}", &source_count.to_string()),
                );
            }
        }

        if config.format.group_by_type {
            summary.push(String::new());
            summary.push(self.messages.tui_fmt_grouping_entries.to_string());
        }

        if summary.is_empty() {
            summary.push(self.messages.tui_fmt_no_changes_needed.to_string());
        }

        // Create preview and switch to ConfirmFormat mode
        self.format_preview = Some(FormatPreview::new(summary, formatted));
        self.mode = AppMode::ConfirmFormat;
        self.message = Some(self.messages.tui_msg_review_changes.to_string());

        Ok(())
    }

    /// Apply the format (after confirmation)
    fn apply_format(&mut self) -> Result<()> {
        if let Some(preview) = self.format_preview.take() {
            let config = crate::config::load_or_create_config()?;

            // Write formatted content to temp file for validation
            std::fs::write(&self.temp_file_path, &preview.formatted_content)?;

            // Validate with shell
            match self.validate_with_shell()? {
                Some(error_msg) => {
                    // Validation failed, show confirmation dialog
                    self.validation_errors = Some(error_msg);
                    self.validation_scroll_offset = 0;
                    self.mode = AppMode::ConfirmSaveWithErrors;
                    self.message = Some(self.messages.tui_msg_validation_failed.to_string());
                    // Restore preview for potential retry
                    self.format_preview = Some(preview);
                    return Ok(());
                }
                None => {
                    // Validation passed
                }
            }

            // Create backup before writing
            let backup_manager = crate::backup::BackupManager::new(self.shell_type, &config);
            backup_manager.create_backup(&self.file_path)?;

            // Write formatted content
            std::fs::write(&self.file_path, preview.formatted_content)?;

            // Refresh entries
            self.refresh()?;
            self.message = Some("File formatted successfully".to_string());
        }

        self.mode = AppMode::Normal;
        Ok(())
    }

    /// Cancel format preview
    fn cancel_format(&mut self) {
        self.format_preview = None;
        self.mode = AppMode::Normal;
        self.message = Some(self.messages.tui_msg_format_cancelled.to_string());
    }

    /// Force apply format without validation (after user confirmation)
    fn force_apply_format(&mut self) -> Result<()> {
        if let Some(preview) = self.format_preview.take() {
            let config = crate::config::load_or_create_config()?;

            // Create backup before writing
            let backup_manager = crate::backup::BackupManager::new(self.shell_type, &config);
            backup_manager.create_backup(&self.file_path)?;

            // Write formatted content
            std::fs::write(&self.file_path, preview.formatted_content)?;

            // Refresh entries
            self.refresh()?;
            self.mode = AppMode::Normal;
            self.message = Some(self.messages.tui_msg_format_bypassed.to_string());
        }

        Ok(())
    }

    /// Start adding a new entry (show type selection menu)
    fn start_adding_entry(&mut self) {
        self.mode = AppMode::SelectingType;
        self.type_selection_index = 0;
        self.type_list_scroll_offset = 0;
        self.message = Some(self.messages.tui_msg_select_entry_type.to_string());
    }

    /// Start editing the selected entry
    fn start_editing(&mut self) {
        if let Some(entry) = self.get_selected_entry() {
            let name = entry.name.clone();
            let entry_type = entry.entry_type;

            // For Comment/Code, use value (contains complete original content)
            // For other types, use value
            let value = entry.value.clone();

            // All entry types start at Value field (Name field is skipped)
            let (start_field, cursor_pos, cursor_col) =
                (EditField::Value, value.len(), value.len());

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

    /// Start moving the selected entry or selection
    fn start_moving(&mut self) {
        if !self.entries.is_empty() {
            // Don't clear selection - support multi-select moving
            // If no selection exists, auto-select current entry
            if self.selected_indices.is_empty() {
                self.selected_indices.insert(self.selected_index);
            }

            // Save selection state for cancel restoration
            self.pre_move_selected_indices = Some(self.selected_indices.clone());
            self.pre_move_selection_anchor = Some(self.selection_anchor);

            // Always save state for undo before any move operation
            if let Ok(content) = self.read_current_content() {
                let _ = self.write_temp_with_undo(&content);
            }

            // If multiple entries are selected, consolidate them first
            let indices = self.get_selected_indices_sorted();
            if indices.len() > 1 {
                // Consolidate selected entries
                if let Err(e) = self.consolidate_selected_entries(&indices) {
                    self.message = Some(
                        self.messages
                            .tui_msg_failed_consolidate
                            .replace("{}", &e.to_string()),
                    );
                    return;
                }
            }

            self.mode = AppMode::Moving;

            let count = self.get_selected_indices_sorted().len();
            if count == 1 {
                self.message = Some(self.messages.tui_msg_use_arrows_to_move.to_string());
            } else {
                self.message = Some(
                    self.messages
                        .tui_msg_moving_entries
                        .replace("{}", &count.to_string()),
                );
            }
        }
    }

    /// Cancel editing
    fn cancel_editing(&mut self) {
        self.edit_state = None;
        self.mode = AppMode::Normal;
        self.message = None;
    }

    /// Consolidate non-contiguous selected entries into a continuous block
    /// Moves all selected entries to follow the first selected entry
    fn consolidate_selected_entries(&mut self, sorted_indices: &[usize]) -> Result<()> {
        if sorted_indices.is_empty() {
            return Ok(());
        }

        // Read current file content
        let content = self.read_current_content()?;
        let lines: Vec<&str> = content.lines().collect();

        // Extract entry blocks and mark their positions for removal
        let mut entry_blocks: Vec<(usize, String)> = Vec::new(); // (original_index, content)

        for &idx in sorted_indices {
            if let Some(entry) = self.entries.get(idx) {
                let start = entry.line_number.unwrap_or(1).saturating_sub(1);
                let end = entry.end_line.unwrap_or(start + 1).saturating_sub(1);

                if end < lines.len() {
                    let block = lines[start..=end].join("\n");
                    entry_blocks.push((start, block));
                }
            }
        }

        // Build new content: remove selected entries, then insert consolidated block at first entry position
        let first_entry = &self.entries[sorted_indices[0]];
        let insert_before_line = first_entry.line_number.unwrap_or(1).saturating_sub(1); // 0-indexed

        let mut new_lines: Vec<String> = Vec::new();
        let mut removed_line_indices: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        // Mark all lines in selected entries for removal
        for &idx in sorted_indices {
            if let Some(entry) = self.entries.get(idx) {
                let start = entry.line_number.unwrap_or(1).saturating_sub(1);
                let end = entry.end_line.unwrap_or(start + 1).saturating_sub(1);
                for line_idx in start..=end {
                    removed_line_indices.insert(line_idx);
                }
            }
        }

        // Build new content
        let mut inserted = false;
        for (i, line) in lines.iter().enumerate() {
            // Insert consolidated blocks at the first entry's position
            if !inserted && i == insert_before_line {
                for (_, block) in &entry_blocks {
                    new_lines.push(block.clone());
                }
                inserted = true;
            }

            // Skip lines that are part of selected entries
            if removed_line_indices.contains(&i) {
                continue;
            }

            new_lines.push(line.to_string());
        }

        // If insert position was at the end, append blocks
        if !inserted {
            for (_, block) in &entry_blocks {
                new_lines.push(block.clone());
            }
        }

        let new_content = new_lines.join("\n") + "\n";
        std::fs::write(&self.temp_file_path, &new_content)?;
        self.reload_from_temp()?;

        // Update cursor to first entry position
        self.selected_index = sorted_indices[0].min(self.entries.len().saturating_sub(1));

        // Update selection to be continuous from first entry
        self.selected_indices.clear();
        for i in 0..sorted_indices.len() {
            let idx = self.selected_index + i;
            if idx < self.entries.len() {
                self.selected_indices.insert(idx);
            }
        }

        Ok(())
    }

    /// Submit editing (save changes)
    /// Directly writes to temp file, then reloads
    fn submit_editing(&mut self) -> Result<()> {
        let Some(state) = self.edit_state.take() else {
            self.mode = AppMode::Normal;
            return Ok(());
        };

        // Value buffer must not be empty for all types
        if state.value_buffer.trim().is_empty() {
            self.edit_state = Some(state);
            self.message = Some(self.messages.tui_msg_alias_value_empty.to_string());
            return Ok(());
        }

        // Auto-extract name from value_buffer for UI display
        let mut state = state;
        state.name_buffer = extract_name_from_value(&state.entry_type, &state.value_buffer);

        // Read current content
        let content = self.read_current_content()?;

        if state.is_new {
            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

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
            // Use split('\n') to preserve trailing empty lines (separator format)
            for (i, new_line) in new_text.split('\n').enumerate() {
                lines.insert(insert_idx + i, new_line.to_string());
            }

            // Write to temp file and reload
            let new_content = lines.join("\n") + "\n";
            self.write_temp_with_undo(&new_content)?;
            self.reload_from_temp()?;

            // Select the newly inserted entry
            self.select_entry_at_line(insert_line);
            self.message = Some(self.messages.msg_entry_added.to_string());
        } else {
            // Update existing entry - replace lines in range
            if let Some(entry) = self.entries.get(self.selected_index) {
                let start_line = entry.line_number.unwrap_or(1);
                let end_line = entry.end_line.unwrap_or(start_line);
                let entry_line = start_line;

                // For Comment/Code, use direct byte-range replacement to preserve exact content
                if matches!(state.entry_type, EntryType::Comment | EntryType::Code) {
                    let new_content = self.replace_line_range(
                        &content,
                        start_line,
                        end_line,
                        &state.value_buffer,
                    );
                    self.write_temp_with_undo(&new_content)?;
                } else {
                    // For structured entries (Alias, Function, EnvVar, Source), use line-based approach
                    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                    let start = start_line.saturating_sub(1); // 0-indexed
                    let end = end_line.saturating_sub(1);

                    // Format the updated entry
                    let new_text = self.format_new_entry(&state);

                    // Safety check: ensure formatter didn't return completely empty content
                    if new_text.trim().is_empty() {
                        self.edit_state = Some(state);
                        self.message = Some(self.messages.tui_msg_formatter_empty.to_string());
                        self.mode = AppMode::Normal;
                        return Ok(());
                    }

                    // Use split('\n') to preserve trailing empty lines (separator format)
                    let new_lines: Vec<&str> = new_text.split('\n').collect();

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

                    // Write to temp file
                    let new_content = lines.join("\n") + "\n";
                    self.write_temp_with_undo(&new_content)?;
                }

                self.reload_from_temp()?;

                // Re-select the entry
                self.select_entry_at_line(entry_line);
            }
            self.message = Some(self.messages.tui_msg_entry_updated.to_string());
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
            // For Code/Comment, use value; for others, use formatter
            let formatted = match entry.entry_type {
                EntryType::Code | EntryType::Comment => entry.value.clone(),
                _ => formatter.format_entry(entry),
            };
            lines.push(formatted);
        }

        lines.join("\n") + "\n"
    }

    /// Save entries to temp file (preserves original order, no reformatting)
    /// After writing, reloads from temp to ensure entries match file content
    #[allow(dead_code)]
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

    /// Save entries to original file (with backup and validation)
    fn save_to_original_file(&mut self) -> Result<()> {
        // Validate with shell first
        match self.validate_with_shell()? {
            Some(error_msg) => {
                // Validation failed, show confirmation dialog
                self.validation_errors = Some(error_msg);
                self.validation_scroll_offset = 0;
                self.mode = AppMode::ConfirmSaveWithErrors;
                self.message = Some(
                    "Shell validation failed. Review errors and confirm to save anyway."
                        .to_string(),
                );
                return Ok(());
            }
            None => {
                // Validation passed, proceed with save
            }
        }

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

        self.message = Some(self.messages.tui_msg_file_saved.to_string());
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

    /// Write to temp file with undo support
    /// Saves current content to undo stack before writing
    fn write_temp_with_undo(&mut self, new_content: &str) -> Result<()> {
        // Save current content to undo stack before modifying
        let current_content = self.read_current_content()?;
        self.undo_stack.push(current_content);

        // Limit undo stack size
        if self.undo_stack.len() > self.max_undo_history {
            self.undo_stack.remove(0);
        }

        // Clear redo stack (new action invalidates redo history)
        self.redo_stack.clear();

        // Write new content
        std::fs::write(&self.temp_file_path, new_content)?;
        self.dirty = true;

        Ok(())
    }

    /// Undo last change
    pub fn undo(&mut self) -> Result<()> {
        if let Some(previous_content) = self.undo_stack.pop() {
            // Save current content to redo stack
            let current_content = self.read_current_content()?;
            self.redo_stack.push(current_content);

            // Restore previous content
            std::fs::write(&self.temp_file_path, &previous_content)?;
            self.dirty = true;
            self.reload_from_temp()?;

            self.message = Some(self.messages.tui_msg_undo_successful.to_string());
        } else {
            self.message = Some(self.messages.tui_msg_nothing_to_undo.to_string());
        }
        Ok(())
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> Result<()> {
        if let Some(next_content) = self.redo_stack.pop() {
            // Save current content to undo stack
            let current_content = self.read_current_content()?;
            self.undo_stack.push(current_content);

            // Restore next content
            std::fs::write(&self.temp_file_path, &next_content)?;
            self.dirty = true;
            self.reload_from_temp()?;

            self.message = Some(self.messages.tui_msg_redo_successful.to_string());
        } else {
            self.message = Some(self.messages.tui_msg_nothing_to_redo.to_string());
        }
        Ok(())
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

    /// Replace a range of lines (1-indexed, inclusive) in content with replacement text.
    /// This preserves the exact content without any line-based parsing.
    fn replace_line_range(
        &self,
        content: &str,
        start_line: usize,
        end_line: usize,
        replacement: &str,
    ) -> String {
        let mut result = String::new();
        let mut current_line = 1;
        let chars = content.chars();
        let mut range_started = false;

        for c in chars {
            if current_line < start_line {
                // Before the range - copy as-is
                result.push(c);
                if c == '\n' {
                    current_line += 1;
                }
            } else if current_line >= start_line && current_line <= end_line {
                // Inside the range - skip original content
                if !range_started {
                    // Insert replacement at the start of the range
                    result.push_str(replacement);
                    // Ensure replacement ends with newline if we're replacing multiple lines
                    // and the replacement doesn't already end with one.
                    // value_buffer uses separator format, always add terminator
                    result.push('\n');
                    range_started = true;
                }
                // Skip original content in range
                if c == '\n' {
                    current_line += 1;
                }
            } else {
                // After the range - copy as-is
                result.push(c);
                if c == '\n' {
                    current_line += 1;
                }
            }
        }

        // Handle case where replacement is at end of file and original didn't have trailing newline
        if !range_started && current_line >= start_line {
            result.push_str(replacement);
        }

        result
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
        self.non_contiguous_mode = false;
        self.selected_indices.clear();
    }

    /// Extend selection upward (Shift+Up)
    /// Merges with existing non-contiguous selections instead of replacing
    fn extend_selection_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Reset anchor if cursor moved outside current selection (new Shift sequence)
        // or initialize anchor if not set
        if self.selection_anchor.is_none() || !self.selected_indices.contains(&self.selected_index)
        {
            self.selection_anchor = Some(self.selected_index);
            self.selected_indices.insert(self.selected_index);
        }

        // Record old range for shrinking detection
        let old_range = self.selection_anchor.map(|anchor| {
            let min = self.selected_index.min(anchor);
            let max = self.selected_index.max(anchor);
            (min, max)
        });

        // Move up
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }

        // Update selection range (merge, not replace)
        if let Some(anchor) = self.selection_anchor {
            let min = self.selected_index.min(anchor);
            let max = self.selected_index.max(anchor);

            // Remove items from old range that are no longer in new range (handle shrinking)
            if let Some((old_min, old_max)) = old_range {
                for i in old_min..=old_max {
                    if i < min || i > max {
                        self.selected_indices.remove(&i);
                    }
                }
            }

            // Add items in new range
            for i in min..=max {
                self.selected_indices.insert(i);
            }
        }

        self.adjust_scroll_for_selection();
    }

    /// Extend selection downward (Shift+Down)
    /// Merges with existing non-contiguous selections instead of replacing
    fn extend_selection_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Reset anchor if cursor moved outside current selection (new Shift sequence)
        // or initialize anchor if not set
        if self.selection_anchor.is_none() || !self.selected_indices.contains(&self.selected_index)
        {
            self.selection_anchor = Some(self.selected_index);
            self.selected_indices.insert(self.selected_index);
        }

        // Record old range for shrinking detection
        let old_range = self.selection_anchor.map(|anchor| {
            let min = self.selected_index.min(anchor);
            let max = self.selected_index.max(anchor);
            (min, max)
        });

        // Move down
        if self.selected_index < self.entries.len() - 1 {
            self.selected_index += 1;
        }

        // Update selection range (merge, not replace)
        if let Some(anchor) = self.selection_anchor {
            let min = self.selected_index.min(anchor);
            let max = self.selected_index.max(anchor);

            // Remove items from old range that are no longer in new range (handle shrinking)
            if let Some((old_min, old_max)) = old_range {
                for i in old_min..=old_max {
                    if i < min || i > max {
                        self.selected_indices.remove(&i);
                    }
                }
            }

            // Add items in new range
            for i in min..=max {
                self.selected_indices.insert(i);
            }
        }

        self.adjust_scroll_for_selection();
    }

    /// Legacy save_to_file - now calls save_to_temp for backward compatibility
    #[allow(dead_code)]
    fn save_to_file(&mut self) -> Result<()> {
        self.save_to_temp()
    }

    /// Open temp file in external editor
    fn open_temp_file_in_editor(&mut self) -> Result<()> {
        use std::process::Command;

        // Get editor from environment (fallback to vi)
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        // Get modification time before editing
        let metadata_before = std::fs::metadata(&self.temp_file_path).ok();
        let mtime_before = metadata_before.and_then(|m| m.modified().ok());

        // Suspend TUI
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        // Open editor (no line number targeting - open full file)
        let status = Command::new(&editor).arg(&self.temp_file_path).status()?;

        // Resume TUI
        execute!(io::stdout(), EnterAlternateScreen)?;
        enable_raw_mode()?;

        // Set flag to force full redraw (clears artifacts after editor)
        self.needs_full_redraw = true;

        if status.success() {
            // Check if file was actually modified
            let metadata_after = std::fs::metadata(&self.temp_file_path).ok();
            let mtime_after = metadata_after.and_then(|m| m.modified().ok());

            if mtime_after != mtime_before {
                // Reload temp file to reflect manual edits
                self.reload_from_temp()?;
                self.dirty = true;
                self.message = Some(self.messages.tui_msg_temp_file_reloaded.to_string());
            } else {
                self.message = Some(self.messages.tui_msg_no_changes_detected.to_string());
            }
        } else {
            self.message = Some(self.messages.tui_msg_editor_error.to_string());
        }

        Ok(())
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
        self.write_temp_with_undo(&new_content)?;
        self.reload_from_temp()?;

        self.clear_selection();
        self.message = Some(if count > 1 {
            self.messages
                .tui_msg_entries_toggled
                .replace("{}", &count.to_string())
        } else {
            self.messages.tui_msg_entry_toggled.to_string()
        });

        Ok(())
    }

    /// Copy selected entries to internal clipboard buffer
    fn copy_selected(&mut self) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        let indices = self.get_selected_indices();
        if indices.is_empty() {
            self.message = Some(self.messages.tui_msg_no_entry_to_copy.to_string());
            return Ok(());
        }

        // Read current content
        let content = self.read_current_content()?;
        let all_lines: Vec<&str> = content.lines().collect();

        // Collect lines from all selected entries
        let mut copied_lines = Vec::new();

        for idx in &indices {
            if let Some(entry) = self.entries.get(*idx) {
                let start = entry.line_number.unwrap_or(1).saturating_sub(1); // 0-indexed
                let end = entry
                    .end_line
                    .unwrap_or(entry.line_number.unwrap_or(1))
                    .saturating_sub(1);

                // Extract lines for this entry
                for line_idx in start..=end.min(all_lines.len().saturating_sub(1)) {
                    if let Some(line) = all_lines.get(line_idx) {
                        copied_lines.push(line.to_string());
                    }
                }
            }
        }

        if !copied_lines.is_empty() {
            self.clipboard_buffer = Some(copied_lines.join("\n"));
            let count = indices.len();
            self.message = Some(if count > 1 {
                format!("Copied {} entries", count)
            } else {
                "Entry copied".to_string()
            });
        }

        Ok(())
    }

    /// Paste content from internal clipboard buffer
    fn paste_entry(&mut self) -> Result<()> {
        if let Some(ref clipboard_content) = self.clipboard_buffer {
            if self.entries.is_empty() {
                // Paste at end of file
                let content = self.read_current_content()?;
                let mut new_content = content;
                if !new_content.ends_with('\n') {
                    new_content.push('\n');
                }
                new_content.push_str(clipboard_content);
                new_content.push('\n');

                self.write_temp_with_undo(&new_content)?;
                self.reload_from_temp()?;
                self.message = Some(self.messages.tui_msg_entry_pasted.to_string());
            } else {
                // Paste after current entry
                let current_entry = self
                    .get_selected_entry()
                    .ok_or_else(|| anyhow::anyhow!("No entry selected"))?
                    .clone();
                let insert_line = current_entry
                    .end_line
                    .or(current_entry.line_number)
                    .unwrap_or(1);

                // Read current content
                let content = self.read_current_content()?;
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

                // Insert clipboard content after current entry
                let insert_idx = insert_line.min(lines.len()); // 0-indexed insert position

                for (i, line) in clipboard_content.lines().enumerate() {
                    lines.insert(insert_idx + i, line.to_string());
                }

                // Write to temp file and reload
                let new_content = lines.join("\n") + "\n";
                self.write_temp_with_undo(&new_content)?;
                self.reload_from_temp()?;

                self.message = Some(self.messages.tui_msg_entry_pasted.to_string());
            }
        } else {
            self.message = Some(self.messages.tui_msg_clipboard_empty.to_string());
        }

        Ok(())
    }

    /// Validate temp file using shell syntax check
    fn validate_with_shell(&self) -> Result<Option<String>> {
        let file_to_check = if self.temp_file_path.exists() {
            &self.temp_file_path
        } else {
            &self.file_path
        };

        let (cmd, args): (&str, Vec<String>) = match self.shell_type {
            ShellType::Bash | ShellType::Zsh => {
                let path_str = file_to_check.to_string_lossy().to_string();
                ("bash", vec!["-n".to_string(), path_str])
            }
            ShellType::PowerShell => {
                let path_str = file_to_check.display().to_string();
                let script = format!(
                    "try {{ $null = [ScriptBlock]::Create((Get-Content '{}' -Raw)) }} catch {{ Write-Error $_.Exception.Message; exit 1 }}",
                    path_str.replace('\\', "\\\\").replace('\'', "''")
                );
                ("pwsh", vec!["-Command".to_string(), script])
            }
        };

        let output = std::process::Command::new(cmd).args(&args).output();

        match output {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let error_msg = if !stderr.is_empty() {
                        stderr.to_string()
                    } else {
                        stdout.to_string()
                    };
                    Ok(Some(error_msg))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                // Return the error through the UI instead of printing to stderr
                Ok(Some(format!("Shell validation command failed: {}", e)))
            }
        }
    }

    /// Save to original file without validation (force save)
    fn force_save_to_original_file(&mut self) -> Result<()> {
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

        self.mode = AppMode::Normal;
        self.message = Some(self.messages.tui_msg_file_saved_bypassed.to_string());
        Ok(())
    }

    /// Start search mode
    fn start_search(&mut self) {
        self.mode = AppMode::Searching;
        self.search_active = true;
        self.search_cursor = self.search_query.len();
        self.update_search_matches();
    }

    /// Handle searching mode keys
    fn handle_searching_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Esc => {
                // Exit search mode, keep query
                self.mode = AppMode::Normal;
            }
            KeyCode::Enter => {
                // Confirm search, jump to first match
                self.mode = AppMode::Normal;
                if !self.search_matches.is_empty() {
                    self.selected_index = self.search_matches[0];
                    self.adjust_scroll_for_selection();
                }
            }
            KeyCode::Char(c) => {
                self.search_query.insert(self.search_cursor, c);
                self.search_cursor += c.len_utf8();
                self.update_search_matches();
            }
            KeyCode::Backspace => {
                if self.search_cursor > 0 {
                    let new_pos = prev_char_boundary(&self.search_query, self.search_cursor);
                    self.search_query.drain(new_pos..self.search_cursor);
                    self.search_cursor = new_pos;
                    self.update_search_matches();
                }
            }
            KeyCode::Left => {
                if self.search_cursor > 0 {
                    self.search_cursor = prev_char_boundary(&self.search_query, self.search_cursor);
                }
            }
            KeyCode::Right => {
                if self.search_cursor < self.search_query.len() {
                    self.search_cursor = next_char_boundary(&self.search_query, self.search_cursor);
                }
            }
            KeyCode::PageDown => self.jump_to_next_match(),
            KeyCode::PageUp => self.jump_to_prev_match(),
            _ => {}
        }
        Ok(())
    }

    /// Update search matches (search Name and Value)
    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.name.to_lowercase().contains(&query_lower)
                || entry.value.to_lowercase().contains(&query_lower)
            {
                self.search_matches.push(i);
            }
        }
    }

    /// Jump to next match
    fn jump_to_next_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        // Find first match after current position
        let next = self
            .search_matches
            .iter()
            .find(|&&idx| idx > self.selected_index)
            .copied()
            .unwrap_or(self.search_matches[0]); // Loop to first
        self.selected_index = next;
        self.adjust_scroll_for_selection();
    }

    /// Jump to previous match
    fn jump_to_prev_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        // Find last match before current position
        let prev = self
            .search_matches
            .iter()
            .rev()
            .find(|&&idx| idx < self.selected_index)
            .copied()
            .unwrap_or(*self.search_matches.last().unwrap()); // Loop to last
        self.selected_index = prev;
        self.adjust_scroll_for_selection();
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

/// Check if path format is valid (does not check file existence)
/// Rejects paths with invalid characters that would be problematic on most systems
/// Extract name from value for UI display purposes
/// This is called after the user submits editing, to populate name_buffer
fn extract_name_from_value(entry_type: &EntryType, value: &str) -> String {
    match entry_type {
        EntryType::Alias => {
            // Extract alias name from "alias name='value'" or similar
            // Find "alias" keyword and extract the name part
            if let Some(pos) = value.find("alias") {
                let after_alias = &value[pos + 5..]; // Skip "alias"
                let trimmed = after_alias.trim();
                // Get the identifier before '=' or space
                if let Some(eq_pos) = trimmed.find('=') {
                    trimmed[..eq_pos].trim().to_string()
                } else {
                    "unknown".to_string()
                }
            } else {
                "unknown".to_string()
            }
        }
        EntryType::Function => {
            // Extract function name from "name() { ... }" or "function name { ... }"
            let trimmed = value.trim();
            if let Some(after_kw) = trimmed.strip_prefix("function ") {
                // "function name {" format
                let after_kw = after_kw.trim();
                if let Some(space_pos) = after_kw.find(|c: char| c.is_whitespace() || c == '{') {
                    after_kw[..space_pos].trim().to_string()
                } else {
                    "unknown".to_string()
                }
            } else if let Some(paren_pos) = trimmed.find("()") {
                // "name() {" format
                trimmed[..paren_pos].trim().to_string()
            } else if trimmed.starts_with("() {") {
                // Anonymous function
                "anonymous".to_string()
            } else {
                "unknown".to_string()
            }
        }
        EntryType::EnvVar => {
            // Extract variable name from "export VAR='value'" or "VAR='value'"
            let trimmed = value.trim().trim_start_matches("export").trim();
            if let Some(eq_pos) = trimmed.find('=') {
                trimmed[..eq_pos].trim().to_string()
            } else {
                "unknown".to_string()
            }
        }
        EntryType::Source => {
            // Extract filename from "source /path/to/file" or ". /path/to/file"
            let path = value
                .trim()
                .trim_start_matches("source")
                .trim_start_matches('.')
                .trim();
            std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }
        EntryType::Code | EntryType::Comment => {
            // For Code/Comment, use a line-based identifier
            let line_count = value.split('\n').count();
            if line_count <= 1 {
                let preview: String = value.chars().take(20).collect();
                preview
            } else {
                format!("{} lines", line_count)
            }
        }
    }
}
