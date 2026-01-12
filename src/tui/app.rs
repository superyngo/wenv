//! TUI application state and logic

use std::io;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::i18n::Messages;
use crate::model::{Entry, ShellType};

/// Application mode
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    AddingEntry,
    EditingName,
    EditingValue,
    Exiting,
}

/// TUI Application state
pub struct TuiApp {
    // State
    pub entries: Vec<Entry>,
    pub selected_index: usize,
    pub scroll_offset: usize,

    // File
    pub file_path: PathBuf,
    pub file_content: String,
    pub shell_type: ShellType,

    // UI state
    pub mode: AppMode,
    pub message: Option<String>,
    pub should_quit: bool,

    // Input state
    pub input_buffer: String,
    pub input_step: usize, // For multi-step input (type, name, value)
    pub entry_type_input: Option<crate::model::EntryType>,

    // i18n
    pub messages: &'static Messages,
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

        Ok(Self {
            entries,
            selected_index: 0,
            scroll_offset: 0,
            file_path,
            file_content,
            shell_type,
            mode: AppMode::Normal,
            message: None,
            should_quit: false,
            input_buffer: String::new(),
            input_step: 0,
            entry_type_input: None,
            messages,
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

            if let Event::Key(key) = event::read()? {
                // Only handle key press events, ignore release
                if key.kind == KeyEventKind::Press {
                    self.handle_key(key.code)?;
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle keyboard input
    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key)?,
            AppMode::ShowingDetail => self.handle_detail_mode(key)?,
            AppMode::ShowingHelp => self.handle_help_mode(key)?,
            AppMode::ConfirmDelete => self.handle_confirm_delete_mode(key)?,
            AppMode::AddingEntry | AppMode::EditingName | AppMode::EditingValue => {
                self.handle_input_mode(key)?
            }
            AppMode::Exiting => {}
        }

        Ok(())
    }

    /// Handle keys in normal mode
    fn handle_normal_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('?') => {
                self.mode = AppMode::ShowingHelp;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                self.mode = AppMode::ShowingDetail;
            }
            KeyCode::Char('d') => {
                self.mode = AppMode::ConfirmDelete;
            }
            KeyCode::Home => {
                self.jump_to_first();
            }
            KeyCode::End => {
                self.jump_to_last();
            }
            KeyCode::PageUp => {
                self.page_up();
            }
            KeyCode::PageDown => {
                self.page_down();
            }
            KeyCode::Char('f') => {
                self.format_file()?;
            }
            KeyCode::Char('c') => {
                self.check_entry()?;
            }
            KeyCode::Char('n') => {
                self.start_adding_entry();
            }
            KeyCode::Char('r') => {
                self.start_editing_name();
            }
            KeyCode::Char('e') => {
                self.start_editing_value();
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
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.delete_selected_entry()?;
                self.mode = AppMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle input mode (for adding/editing)
    fn handle_input_mode(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Enter => {
                self.submit_input()?;
            }
            KeyCode::Esc => {
                self.cancel_input();
            }
            _ => {}
        }

        Ok(())
    }

    /// Move selection up
    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    fn move_down(&mut self) {
        if self.selected_index < self.entries.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    /// Jump to first entry
    fn jump_to_first(&mut self) {
        self.selected_index = 0;
    }

    /// Jump to last entry
    fn jump_to_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = self.entries.len() - 1;
        }
    }

    /// Page up (move 10 entries up)
    fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
    }

    /// Page down (move 10 entries down)
    fn page_down(&mut self) {
        let max_index = self.entries.len().saturating_sub(1);
        self.selected_index = std::cmp::min(self.selected_index + 10, max_index);
    }

    /// Get the currently selected entry
    pub fn get_selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected_index)
    }

    /// Delete the selected entry
    fn delete_selected_entry(&mut self) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }

        // Remove from list
        self.entries.remove(self.selected_index);

        // Adjust selection
        if self.selected_index >= self.entries.len() && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        self.message = Some("Entry deleted (not saved yet)".to_string());

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

    /// Start adding a new entry
    fn start_adding_entry(&mut self) {
        self.mode = AppMode::AddingEntry;
        self.input_step = 0;
        self.input_buffer.clear();
        self.entry_type_input = None;
        self.message = Some("Enter entry type (alias/func/env/source):".to_string());
    }

    /// Start editing entry name
    fn start_editing_name(&mut self) {
        if let Some(entry) = self.get_selected_entry() {
            let name = entry.name.clone();
            self.mode = AppMode::EditingName;
            self.input_buffer = name.clone();
            self.message = Some(format!("Edit name: {}", name));
        }
    }

    /// Start editing entry value
    fn start_editing_value(&mut self) {
        if let Some(entry) = self.get_selected_entry() {
            let name = entry.name.clone();
            let value = entry.value.clone();
            self.mode = AppMode::EditingValue;
            self.input_buffer = value;
            self.message = Some(format!("Edit value for: {}", name));
        }
    }

    /// Submit input
    fn submit_input(&mut self) -> Result<()> {
        match self.mode {
            AppMode::AddingEntry => self.handle_add_input()?,
            AppMode::EditingName => self.handle_edit_name()?,
            AppMode::EditingValue => self.handle_edit_value()?,
            _ => {}
        }

        Ok(())
    }

    /// Cancel input
    fn cancel_input(&mut self) {
        self.mode = AppMode::Normal;
        self.input_buffer.clear();
        self.input_step = 0;
        self.entry_type_input = None;
        self.message = None;
    }

    /// Handle add entry input (multi-step)
    fn handle_add_input(&mut self) -> Result<()> {
        use crate::model::EntryType;

        match self.input_step {
            0 => {
                // Parse entry type
                match self.input_buffer.to_lowercase().as_str() {
                    "alias" | "a" => self.entry_type_input = Some(EntryType::Alias),
                    "func" | "function" | "f" => self.entry_type_input = Some(EntryType::Function),
                    "env" | "e" => self.entry_type_input = Some(EntryType::EnvVar),
                    "source" | "s" => self.entry_type_input = Some(EntryType::Source),
                    _ => {
                        self.message = Some("Invalid type. Try: alias/func/env/source".to_string());
                        self.input_buffer.clear();
                        return Ok(());
                    }
                }
                self.input_step = 1;
                self.input_buffer.clear();
                self.message = Some("Enter name:".to_string());
            }
            1 => {
                // Store name temporarily and ask for value
                let name = self.input_buffer.clone();
                self.input_step = 2;
                self.input_buffer = name; // Keep name for next step
                self.message = Some("Enter value:".to_string());
            }
            2 => {
                // Create entry
                let entry_type = self.entry_type_input.unwrap();
                let lines: Vec<&str> = self.input_buffer.splitn(2, '\n').collect();
                let name = lines.first().unwrap_or(&"").trim().to_string();
                let value = lines.get(1).unwrap_or(&"").trim().to_string();

                if name.is_empty() || value.is_empty() {
                    self.message = Some("Name and value cannot be empty".to_string());
                    return Ok(());
                }

                let new_entry = Entry::new(entry_type, name, value);
                self.entries.push(new_entry);

                // Write to file
                self.save_to_file()?;

                self.cancel_input();
                self.message = Some("Entry added successfully".to_string());
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle edit name
    fn handle_edit_name(&mut self) -> Result<()> {
        if let Some(entry) = self.entries.get_mut(self.selected_index) {
            entry.name = self.input_buffer.clone();
            self.save_to_file()?;
            self.cancel_input();
            self.message = Some("Name updated successfully".to_string());
        }

        Ok(())
    }

    /// Handle edit value
    fn handle_edit_value(&mut self) -> Result<()> {
        if let Some(entry) = self.entries.get_mut(self.selected_index) {
            entry.value = self.input_buffer.clone();
            self.save_to_file()?;
            self.cancel_input();
            self.message = Some("Value updated successfully".to_string());
        }

        Ok(())
    }

    /// Save entries to file
    fn save_to_file(&mut self) -> Result<()> {
        // Create backup
        let config = crate::config::load_or_create_config()?;
        let backup_manager = crate::backup::BackupManager::new(self.shell_type, &config);
        backup_manager.create_backup(&self.file_path)?;

        // Format and write
        let formatter = crate::formatter::get_formatter(self.shell_type);
        let formatted = formatter.format(&self.entries, &config);
        std::fs::write(&self.file_path, formatted)?;

        // Refresh
        self.refresh()?;

        Ok(())
    }
}
