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
}

pub struct ClipboardState {
    pub entries: Vec<crate::model::Entry>,
}

impl ClipboardState {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub struct UndoSnapshot {
    pub file_states: Vec<(std::path::PathBuf, String, Vec<crate::model::Entry>)>,
}