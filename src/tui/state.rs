//! TUI application state types

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Searching,
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    ConfirmQuit,
    Moving,
    TextInput,
}

pub struct ClipboardState {
    pub entries: Vec<crate::model::Entry>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardState {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub struct UndoSnapshot {
    pub file_states: Vec<(std::path::PathBuf, String, Vec<crate::model::Entry>, bool)>, // (path, content, entries, dirty)
}

pub struct MoveState {
    pub source_items: Vec<(usize, usize)>, // (file_index, entry_index) of entries being moved
    pub insertion_cursor: usize,           // visible-list index for drop target
    pub from_selection: bool,              // true if move was initiated from multi-selection
}
