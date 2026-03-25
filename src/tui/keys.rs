//! Key event to action mapping

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::tui::state::AppMode;

pub enum Action {
    NavigateUp,
    NavigateDown,
    PageUp,
    PageDown,
    Home,
    End,
    ToggleExpand,
    CollapseAll,
    ExpandAll,
    Edit,
    Add,
    Delete,
    ToggleSelect,
    RangeSelectUp,
    RangeSelectDown,
    Cut,
    Paste,
    StartMove,
    Search,
    Undo,
    Help,
    Save,
    Quit,
    Confirm,
    Cancel,
    SearchInput(char),
    SearchBackspace,
    Noop,
}

pub fn map_key(mode: &AppMode, key: KeyEvent) -> Action {
    match mode {
        AppMode::Normal => map_normal_key(key),
        AppMode::Moving => map_moving_key(key),
        AppMode::Searching => map_search_key(key),
        AppMode::ShowingDetail => map_detail_key(key),
        _ => map_popup_key(key),
    }
}

fn map_normal_key(key: KeyEvent) -> Action {
    // Check Shift+arrow first for range selection
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Up => return Action::RangeSelectUp,
            KeyCode::Down => return Action::RangeSelectDown,
            _ => {}
        }
    }
    // Check Ctrl modifiers
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char('s') = key.code {
            return Action::Save;
        }
    }
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::NavigateUp,
        KeyCode::Down | KeyCode::Char('j') => Action::NavigateDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Char('0') => Action::CollapseAll,
        KeyCode::Char('9') => Action::ExpandAll,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('a') => Action::Add,
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('s') => Action::ToggleSelect,
        KeyCode::Char('x') => Action::Cut,
        KeyCode::Char('p') => Action::Paste,
        KeyCode::Char('m') => Action::StartMove,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('u') => Action::Undo,
        KeyCode::Char('?') => Action::Help,
        KeyCode::Char('w') => Action::Save,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Esc => Action::Cancel,
        _ => Action::Noop,
    }
}

fn map_moving_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::NavigateUp,
        KeyCode::Down | KeyCode::Char('j') => Action::NavigateDown,
        KeyCode::PageUp => Action::PageUp,
        KeyCode::PageDown => Action::PageDown,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Esc => Action::Cancel,
        _ => Action::Noop,
    }
}

fn map_search_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Up | KeyCode::PageUp => Action::NavigateUp,
        KeyCode::Down | KeyCode::PageDown => Action::NavigateDown,
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        // Toggle info popup
        KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Char(c) => Action::SearchInput(c),
        _ => Action::Noop,
    }
}

fn map_detail_key(key: KeyEvent) -> Action {
    match key.code {
        // Toggle: Enter/Space close the detail popup
        KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Esc | KeyCode::Char('q') => Action::Cancel,
        _ => Action::Noop,
    }
}

fn map_popup_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter | KeyCode::Char('y') => Action::Confirm,
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => Action::Cancel,
        _ => Action::Noop,
    }
}