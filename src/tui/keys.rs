//! Key event to action mapping

use crate::tui::line_edit::LineEditOp;
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
    FieldUndo,
    SearchEdit(LineEditOp),
    TextInputEdit(LineEditOp),
    InlineEdit(LineEditOp),
    SearchBackspace,
    TextInputChar(char),
    TextInputBackspace,
    TextInputDelete,
    TextInputLeft,
    TextInputRight,
    TextInputHome,
    TextInputEnd,
    SearchDelete,
    SearchLeft,
    SearchRight,
    SearchHome,
    SearchEnd,
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
        AppMode::Moving => map_placing_key(key),
        AppMode::MovingFile => map_moving_key(key),
        AppMode::FilterInput => map_filter_input_key(key),
        AppMode::FilterActive => map_filter_active_key(key),
        AppMode::ShowingDetail => map_detail_key(key),
        AppMode::TextInput => map_text_input_key(key),
        AppMode::SelectingSnippet => map_snippet_key(key),
        AppMode::InlineEdit => map_inline_edit_key(key),
        AppMode::ShowingHelp => map_help_key(key),
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
        KeyCode::Char('d') | KeyCode::Delete => Action::Delete,
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
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::Enter | KeyCode::Char('v') => Action::Confirm,
        KeyCode::Esc => Action::Cancel,
        _ => Action::Noop,
    }
}

/// Entry copy/cut placement: like `map_moving_key`, but `c`/`x` switch the
/// pending operation between copy and cut without leaving placement.
fn map_placing_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('c') => Action::Copy,
        KeyCode::Char('x') => Action::Cut,
        _ => map_moving_key(key),
    }
}

/// Emacs-style caret shortcuts shared by all text inputs. Returns `None` when the key is
/// not a shortcut; Ctrl/Alt chords that are not shortcuts must never insert text.
fn line_edit_shortcut(key: KeyEvent) -> Option<Result<LineEditOp, ()>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    if ctrl && alt {
        return None; // AltGr on Windows reports Ctrl+Alt: that is text, not a shortcut
    }
    match key.code {
        KeyCode::Left if ctrl || alt => Some(Ok(LineEditOp::WordLeft)),
        KeyCode::Right if ctrl || alt => Some(Ok(LineEditOp::WordRight)),
        KeyCode::Char(c) if ctrl => Some(match c.to_ascii_lowercase() {
            'u' => Ok(LineEditOp::KillToStart),
            'k' => Ok(LineEditOp::KillToEnd),
            'w' => Ok(LineEditOp::DeleteWordBack),
            'y' => Ok(LineEditOp::Yank),
            _ => Err(()),
        }),
        KeyCode::Char(c) if alt => Some(match c.to_ascii_lowercase() {
            'b' => Ok(LineEditOp::WordLeft),
            'f' => Ok(LineEditOp::WordRight),
            _ => Err(()),
        }),
        _ => None,
    }
}

fn ctrl_home_end(key: KeyEvent, home: Action, end: Action) -> Option<Action> {
    if let KeyCode::Char(c) = key.code {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            return match c.to_ascii_lowercase() {
                'a' => Some(home),
                'e' => Some(end),
                _ => None,
            };
        }
    }
    None
}

fn map_filter_input_key(key: KeyEvent) -> Action {
    // Filter input is a text field: typing plus the baseline editing contract
    // (←/→/Home/End/Backspace/Del), Enter to confirm, Esc to cancel.
    if let Some(a) = ctrl_home_end(key, Action::SearchHome, Action::SearchEnd) {
        return a;
    }
    if key.code == KeyCode::Char('z') && key.modifiers == KeyModifiers::CONTROL {
        return Action::FieldUndo;
    }
    match line_edit_shortcut(key) {
        Some(Ok(op)) => return Action::SearchEdit(op),
        Some(Err(())) => return Action::Noop,
        None => {}
    }
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Delete => Action::SearchDelete,
        KeyCode::Left => Action::SearchLeft,
        KeyCode::Right => Action::SearchRight,
        KeyCode::Home => Action::SearchHome,
        KeyCode::End => Action::SearchEnd,
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
    if let Some(a) = ctrl_home_end(key, Action::TextInputHome, Action::TextInputEnd) {
        return a;
    }
    if key.code == KeyCode::Char('z') && key.modifiers == KeyModifiers::CONTROL {
        return Action::FieldUndo;
    }
    match line_edit_shortcut(key) {
        Some(Ok(op)) => return Action::TextInputEdit(op),
        Some(Err(())) => return Action::Noop,
        None => {}
    }
    match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace => Action::TextInputBackspace,
        KeyCode::Delete => Action::TextInputDelete,
        KeyCode::Left => Action::TextInputLeft,
        KeyCode::Right => Action::TextInputRight,
        KeyCode::Home => Action::TextInputHome,
        KeyCode::End => Action::TextInputEnd,
        KeyCode::Char(c) => Action::TextInputChar(c),
        _ => Action::Noop,
    }
}

fn map_inline_edit_key(key: KeyEvent) -> Action {
    if let Some(a) = ctrl_home_end(key, Action::InlineHome, Action::InlineEnd) {
        return a;
    }
    if key.code == KeyCode::Char('z') && key.modifiers == KeyModifiers::CONTROL {
        return Action::FieldUndo;
    }
    match line_edit_shortcut(key) {
        Some(Ok(op)) => return Action::InlineEdit(op),
        Some(Err(())) => return Action::Noop,
        None => {}
    }
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

/// Help popup: scrollable read-only surface. Shares the detail popup's scroll
/// actions (only one of the two popups is ever open at a time).
fn map_help_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => Action::Cancel,
        KeyCode::Up | KeyCode::Char('k') => Action::DetailScrollUp,
        KeyCode::Down | KeyCode::Char('j') => Action::DetailScrollDown,
        KeyCode::PageUp => Action::DetailPageUp,
        KeyCode::PageDown => Action::DetailPageDown,
        KeyCode::Home => Action::DetailHome,
        KeyCode::End => Action::DetailEnd,
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

#[cfg(test)]
mod input_contract_tests {
    use super::*;

    fn chord(c: char, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), m)
    }

    #[test]
    fn test_ctrl_chords_never_insert_text_in_inputs() {
        for mode in [
            AppMode::FilterInput,
            AppMode::TextInput,
            AppMode::InlineEdit,
        ] {
            assert!(!matches!(
                map_key(&mode, chord('a', KeyModifiers::CONTROL)),
                Action::SearchInput(_) | Action::TextInputChar(_) | Action::InlineInput(_)
            ));
            assert!(matches!(
                map_key(&mode, chord('x', KeyModifiers::CONTROL)),
                Action::Noop
            ));
        }
    }

    #[test]
    fn test_emacs_shortcuts_map_to_edit_ops() {
        assert!(matches!(
            map_key(&AppMode::FilterInput, chord('a', KeyModifiers::CONTROL)),
            Action::SearchHome
        ));
        assert!(matches!(
            map_key(&AppMode::TextInput, chord('k', KeyModifiers::CONTROL)),
            Action::TextInputEdit(LineEditOp::KillToEnd)
        ));
        assert!(matches!(
            map_key(&AppMode::InlineEdit, chord('b', KeyModifiers::ALT)),
            Action::InlineEdit(LineEditOp::WordLeft)
        ));
    }

    #[test]
    fn test_ctrl_z_is_field_undo() {
        assert!(matches!(
            map_key(&AppMode::FilterInput, chord('z', KeyModifiers::CONTROL)),
            Action::FieldUndo
        ));
    }

    #[test]
    fn test_altgr_and_plain_chars_still_insert() {
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert!(matches!(
            map_key(&AppMode::FilterInput, chord('@', altgr)),
            Action::SearchInput('@')
        ));
        assert!(matches!(
            map_key(&AppMode::FilterInput, chord('A', KeyModifiers::SHIFT)),
            Action::SearchInput('A')
        ));
    }
}
