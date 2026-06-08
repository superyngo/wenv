//! Key event to action mapping

use crate::tui::state::AppMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    EditExternal,
    InlineInput(char),
    InlineBackspace,
    InlineDelete,
    InlineLeft,
    InlineRight,
    InlineHome,
    InlineEnd,
    Add,
    Delete,
    ToggleSelect,
    RangeSelectUp,
    RangeSelectDown,
    Cut,
    Copy,
    StartMove,
    Search,
    Undo,
    Redo,
    Remark,
    AddFile,
    Help,
    Save,
    Quit,
    Confirm,
    Cancel,
    SearchInput(char),
    SearchBackspace,
    TextInputChar(char),
    TextInputBackspace,
    TextInputLeft,
    TextInputRight,
    DetailScrollUp,
    DetailScrollDown,
    DetailPageUp,
    DetailPageDown,
    DetailHome,
    DetailEnd,
    SnippetUp,
    SnippetDown,
    SnippetSelect,
    SnippetCancel,
    Noop,
}

pub fn map_key(mode: &AppMode, key: KeyEvent) -> Action {
    match mode {
        AppMode::Normal => map_normal_key(key),
        AppMode::Moving => map_moving_key(key),
        AppMode::MovingFile => map_moving_key(key),
        AppMode::FilterInput => map_filter_input_key(key),
        AppMode::FilterActive => map_filter_active_key(key),
        AppMode::ShowingDetail => map_detail_key(key),
        AppMode::TextInput => map_text_input_key(key),
        AppMode::SelectingSnippet => map_snippet_key(key),
        AppMode::InlineEdit => map_inline_edit_key(key),
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
        KeyCode::Char('E') => Action::EditExternal,
        KeyCode::Char('a') => Action::Add,
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('s') => Action::ToggleSelect,
        KeyCode::Char('x') => Action::Cut,
        KeyCode::Char('c') => Action::Copy,
        KeyCode::Char('m') => Action::StartMove,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('z') => Action::Undo,
        KeyCode::Char('y') => Action::Redo,
        KeyCode::Char('r') => Action::Remark,
        KeyCode::Char('n') => Action::AddFile,
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
        KeyCode::Enter | KeyCode::Char('v') => Action::Confirm,
        KeyCode::Esc => Action::Cancel,
        _ => Action::Noop,
    }
}

fn map_filter_input_key(key: KeyEvent) -> Action {
    // During filter input, only typing, backspace, enter to confirm, and esc to cancel.
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(c) => Action::SearchInput(c),
        _ => Action::Noop,
    }
}

fn map_filter_active_key(key: KeyEvent) -> Action {
    // In filter-active mode, normal navigation and operations are available.
    // '/' re-opens filter input to edit the query; Esc clears the filter.
    map_normal_key(key)
}

fn map_detail_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Esc | KeyCode::Char('q') => Action::Cancel,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('r') => Action::Remark,
        KeyCode::Up | KeyCode::Char('k') => Action::DetailScrollUp,
        KeyCode::Down | KeyCode::Char('j') => Action::DetailScrollDown,
        KeyCode::PageUp => Action::DetailPageUp,
        KeyCode::PageDown => Action::DetailPageDown,
        KeyCode::Home => Action::DetailHome,
        KeyCode::End => Action::DetailEnd,
        _ => Action::Noop,
    }
}

fn map_text_input_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::TextInputBackspace,
        KeyCode::Left => Action::TextInputLeft,
        KeyCode::Right => Action::TextInputRight,
        KeyCode::Char(c) => Action::TextInputChar(c),
        _ => Action::Noop,
    }
}

fn map_inline_edit_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::InlineBackspace,
        KeyCode::Delete => Action::InlineDelete,
        KeyCode::Left => Action::InlineLeft,
        KeyCode::Right => Action::InlineRight,
        KeyCode::Home => Action::InlineHome,
        KeyCode::End => Action::InlineEnd,
        KeyCode::Char(c) => Action::InlineInput(c),
        _ => Action::Noop,
    }
}

fn map_snippet_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::SnippetUp,
        KeyCode::Down | KeyCode::Char('j') => Action::SnippetDown,
        KeyCode::Enter => Action::SnippetSelect,
        KeyCode::Esc => Action::SnippetCancel,
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
