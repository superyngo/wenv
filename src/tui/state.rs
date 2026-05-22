//! TUI application state types

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    /// Typing filter query — navigation and entry operations are disabled.
    FilterInput,
    /// Browsing filtered results — normal navigation and operations apply.
    FilterActive,
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    ConfirmQuit,
    Moving,
    TextInput,
    ConfirmRemoveFile,
    ConfirmCreateFile,
    MovingFile,
    SelectingSnippet,
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

#[derive(Debug, Clone, PartialEq)]
pub enum InputPurpose {
    AddFilePath,
}

pub struct TextInputState {
    pub prompt: String,
    pub value: String,
    pub cursor_pos: usize,
    pub purpose: InputPurpose,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExpandedSnapshot {
    pub files: Vec<bool>,
    pub dirs: Vec<bool>,
}

pub struct FileMovingState {
    pub original_fi: usize,        // file index being moved
    pub insertion_cursor: usize,   // visible-list index for drop target
    pub saved_expanded: ExpandedSnapshot, // original expanded state per file + dirs
}
