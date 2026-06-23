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
    ConfirmRemoveGroup,
    ConfirmCreateFile,
    /// Confirm moving a real on-disk file (a group member) to the trash.
    ConfirmDeleteFile,
    MovingFile,
    SelectingSnippet,
    /// Editing a single-line entry's raw value in-place (no external editor).
    InlineEdit,
}

/// In-flight inline editor state. `buffer` is the entry's full raw value line
/// (separator format, never contains `\n`); `cursor` is the char index within it;
/// `scroll` is the horizontal viewport offset (first visible char) kept in sync
/// with the cursor so long values scroll within the VALUE column.
pub struct InlineEditState {
    pub fi: usize,
    pub ei: usize,
    pub buffer: String,
    pub cursor: usize,
    pub scroll: usize,
}

pub struct UndoSnapshot {
    pub file_states: Vec<(std::path::PathBuf, String, Vec<crate::model::Entry>, bool)>, // (path, content, entries, dirty)
}

pub struct MoveState {
    pub source_items: Vec<(usize, usize)>, // (file_index, entry_index) of source entries (blue)
    pub insertion_cursor: usize,           // visible-list index for drop target (green)
    pub from_selection: bool,              // true if initiated from multi-selection
    pub cut: bool,                         // true = remove sources on drop (cut); false = copy
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputPurpose {
    AddFilePath,
    /// Create a new file inside a directory group. `dir` is the base directory
    /// the file is created in; `pattern` is the group's source pattern (used to
    /// warn when the chosen name won't rejoin the group on reload).
    NewFileInDir {
        dir: std::path::PathBuf,
        pattern: String,
    },
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
    pub original_fi: usize,               // file index being moved
    pub insertion_cursor: usize,          // visible-list index for drop target
    pub saved_expanded: ExpandedSnapshot, // original expanded state per file + dirs
}
