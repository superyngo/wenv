//! TUI application core

use std::io;
use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::i18n::Messages;
use crate::model::profile::{ListItem, ShellProfile};
use crate::tui::keys::{self, Action};
use crate::tui::list;
use crate::tui::state::AppMode;

pub struct TuiApp {
    pub profile: ShellProfile,
    pub visible_items: Vec<ListItem>,
    pub cursor: usize,
    pub mode: AppMode,
    pub should_quit: bool,
    pub message: Option<String>,
    pub messages: &'static Messages,
}

impl TuiApp {
    pub fn new(profile: ShellProfile, messages: &'static Messages) -> Result<Self> {
        let visible_items = profile.build_visible_list();
        Ok(Self {
            profile,
            visible_items,
            cursor: 0,
            mode: AppMode::Normal,
            should_quit: false,
            message: None,
            messages,
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
            terminal.draw(|f| crate::tui::ui::draw(f, self))?;

            if let Event::Key(key) = event::read()? {
                // Ignore key release events on Windows
                if key.kind != crossterm::event::KeyEventKind::Press {
                    continue;
                }
                let action = keys::map_key(&self.mode, key);
                self.handle_action(action)?;
            }
        }
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        self.message = None; // Clear message on any action

        match action {
            Action::NavigateUp => {
                self.cursor = list::navigate_up(&self.visible_items, self.cursor);
            }
            Action::NavigateDown => {
                self.cursor = list::navigate_down(&self.visible_items, self.cursor);
            }
            Action::Home => {
                self.cursor = list::navigate_home();
            }
            Action::End => {
                self.cursor = list::navigate_end(&self.visible_items);
            }
            Action::ToggleExpand => {
                self.toggle_at_cursor();
            }
            Action::CollapseAll => {
                self.profile.toggle_all(false);
                self.rebuild_list();
            }
            Action::ExpandAll => {
                self.profile.toggle_all(true);
                self.rebuild_list();
            }
            Action::Quit => {
                self.should_quit = true;
            }
            _ => {
                // Not implemented yet — stubs for Tasks 7-12
            }
        }
        Ok(())
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
}