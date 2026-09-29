//! Text input (inline editor, search filter, path prompt, snippet selection).

use crate::model::profile::ListItem;
use crate::tui::app::{EditorRequest, TuiApp};
use crate::tui::state::AppMode;

/// Slide a `width`-wide horizontal viewport so the cursor stays visible, scrolling
/// by the minimum needed. The virtual length includes the trailing cursor slot, so
/// no blank gap is left past the end after the buffer shrinks. (Ported from confy.)
pub(crate) fn clamp_inline_scroll(scroll: usize, cursor: usize, len: usize, width: usize) -> usize {
    let w = width.max(1);
    let cur = cursor.min(len);
    let mut s = scroll;
    if cur < s {
        s = cur;
    } else if cur >= s + w {
        s = cur + 1 - w;
    }
    s.min((len + 1).saturating_sub(w))
}

impl TuiApp {
    // --- Inline editor (single-line entries) -------------------------------

    /// Enter inline-edit mode for a single-line entry, seeding the buffer with its
    /// raw value and placing the cursor at the end.
    pub(super) fn begin_inline_edit(&mut self, fi: usize, ei: usize) {
        let buffer = self.profile.files[fi].entries[ei].value.clone();
        let cursor = buffer.chars().count();
        self.inline_edit = Some(crate::tui::state::InlineEditState {
            fi,
            ei,
            buffer,
            cursor,
            scroll: 0,
        });
        self.mode = AppMode::InlineEdit;
        self.message = None;
    }

    /// Keep the inline editor's horizontal viewport in sync with the cursor for a
    /// `width`-wide VALUE column. Called from the event loop before each draw.
    pub fn inline_clamp_scroll(&mut self, width: usize) {
        if let Some(e) = &mut self.inline_edit {
            let len = e.buffer.chars().count();
            e.scroll = clamp_inline_scroll(e.scroll, e.cursor.min(len), len, width);
        }
    }

    pub(super) fn inline_input_char(&mut self, c: char) {
        if let Some(e) = &mut self.inline_edit {
            let byte = e
                .buffer
                .char_indices()
                .nth(e.cursor)
                .map(|(b, _)| b)
                .unwrap_or(e.buffer.len());
            e.buffer.insert(byte, c);
            e.cursor += 1;
            self.message = None;
        }
    }

    pub(super) fn inline_backspace(&mut self) {
        if let Some(e) = &mut self.inline_edit {
            if e.cursor > 0 {
                let prev = e
                    .buffer
                    .char_indices()
                    .nth(e.cursor - 1)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                e.buffer.remove(prev);
                e.cursor -= 1;
                self.message = None;
            }
        }
    }

    pub(super) fn inline_delete(&mut self) {
        if let Some(e) = &mut self.inline_edit {
            let len = e.buffer.chars().count();
            if e.cursor < len {
                let at = e
                    .buffer
                    .char_indices()
                    .nth(e.cursor)
                    .map(|(b, _)| b)
                    .unwrap_or(e.buffer.len());
                e.buffer.remove(at);
                self.message = None;
            }
        }
    }

    pub(super) fn inline_cursor_left(&mut self) {
        if let Some(e) = &mut self.inline_edit {
            e.cursor = e.cursor.saturating_sub(1);
        }
    }

    pub(super) fn inline_cursor_right(&mut self) {
        if let Some(e) = &mut self.inline_edit {
            let len = e.buffer.chars().count();
            if e.cursor < len {
                e.cursor += 1;
            }
        }
    }

    pub(super) fn inline_cursor_home(&mut self) {
        if let Some(e) = &mut self.inline_edit {
            e.cursor = 0;
        }
    }

    pub(super) fn inline_cursor_end(&mut self) {
        if let Some(e) = &mut self.inline_edit {
            e.cursor = e.buffer.chars().count();
        }
    }

    pub(super) fn inline_cancel(&mut self) {
        self.inline_edit = None;
        self.mode = if self.is_filtering() {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
        self.message = None;
    }

    /// Commit the inline edit: re-parse and replace via the shared apply path, then
    /// return to the resting mode. Refreshes the filter when one is active.
    pub(super) fn inline_commit(&mut self) {
        let Some(e) = self.inline_edit.take() else {
            return;
        };
        let old_value = self.profile.files[e.fi].entries[e.ei].value.clone();
        self.apply_edited_value(e.fi, e.ei, &old_value, &e.buffer);
        self.mode = if self.is_filtering() {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
        if self.is_filtering() {
            self.update_filter();
        }
    }

    /// True when a filter query is active.
    pub(super) fn is_filtering(&self) -> bool {
        self.search.as_ref().is_some_and(|s| !s.query.is_empty())
    }

    /// Returns true when either filter mode is active.
    pub fn is_filter_mode(&self) -> bool {
        matches!(self.mode, AppMode::FilterInput | AppMode::FilterActive)
    }

    /// Update fuzzy-match results and rebuild the filtered list.
    pub(super) fn update_filter(&mut self) {
        if let Some(search) = &mut self.search {
            search.update_matches(&self.profile);
        }
        self.rebuild_list();
    }

    pub(super) fn action_search(&mut self) {
        if self.mode == AppMode::FilterActive {
            // Re-open input to edit the existing filter query
            if let Some(search) = &mut self.search {
                search.cursor_end();
            }
            self.mode = AppMode::FilterInput;
        } else {
            // Capture expanded state before filter
            let files: Vec<bool> = self.profile.files.iter().map(|f| f.expanded).collect();
            let dirs: Vec<bool> = self
                .profile
                .tree
                .iter()
                .map(|n| match n {
                    crate::model::profile::TreeNode::Dir(g) => g.expanded,
                    _ => false,
                })
                .collect();
            self.expanded_snapshot = Some(crate::tui::state::ExpandedSnapshot { files, dirs });
            self.search = Some(crate::tui::search::SearchState::new());
            self.mode = AppMode::FilterInput;
            self.message = None;
        }
    }

    pub(super) fn action_add(&mut self) -> anyhow::Result<EditorRequest> {
        // On a directory group header, `a` creates a new file in the group's
        // directory (prompt name → create → open $EDITOR) rather than adding
        // an entry. File headers (grouped or not) keep the add-entry flow.
        if let Some(ListItem::DirHeader(ti)) = self.visible_items.get(self.cursor) {
            if let Some(crate::model::profile::TreeNode::Dir(g)) = self.profile.tree.get(*ti) {
                let pattern = g.source_pattern.clone();
                let dir = self.group_base_dir(g);
                self.text_input = Some(crate::tui::state::TextInputState {
                    prompt: format!("New file name in {}", dir.display()),
                    value: String::new(),
                    cursor_pos: 0,
                    purpose: crate::tui::state::InputPurpose::NewFileInDir { dir, pattern },
                });
                self.mode = AppMode::TextInput;
                self.message = None;
            }
            return Ok(EditorRequest::None);
        }
        if !self.is_current_file_writable() {
            self.message = Some("File is read-only".into());
            return Ok(EditorRequest::None);
        }
        if self.snippets.is_empty() {
            let fi = self.current_file_index();
            return Ok(EditorRequest::AddEntry(fi));
        }
        self.snippet_cursor = 0;
        self.snippet_scroll_offset = 0;
        self.previous_mode = Some(self.mode.clone());
        self.mode = AppMode::SelectingSnippet;
        Ok(EditorRequest::None)
    }

    pub(super) fn action_snippet_up(&mut self) {
        if self.snippet_cursor > 0 {
            self.snippet_cursor -= 1;
            if self.snippet_cursor < self.snippet_scroll_offset {
                self.snippet_scroll_offset = self.snippet_cursor;
            }
        }
    }

    pub(super) fn action_snippet_down(&mut self) {
        if self.snippet_cursor < self.snippets.len().saturating_sub(1) {
            self.snippet_cursor += 1;
        }
    }

    pub(super) fn action_snippet_select(&mut self) -> EditorRequest {
        let fi = self.current_file_index();
        let template = self
            .snippets
            .get(self.snippet_cursor)
            .and_then(|s| s.template.clone());
        let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
        self.mode = if matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive) {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
        EditorRequest::AddEntryWithTemplate(fi, template)
    }

    pub(super) fn action_snippet_cancel(&mut self) {
        let return_mode = self.previous_mode.take().unwrap_or(AppMode::Normal);
        self.mode = if matches!(return_mode, AppMode::FilterInput | AppMode::FilterActive) {
            AppMode::FilterActive
        } else {
            AppMode::Normal
        };
    }

    pub(super) fn action_add_file(&mut self) {
        self.text_input = Some(crate::tui::state::TextInputState {
            prompt: "Add file, group, glob, or $VAR to config".into(),
            value: String::new(),
            cursor_pos: 0,
            purpose: crate::tui::state::InputPurpose::AddFilePath,
        });
        self.mode = AppMode::TextInput;
        self.message = None;
    }

    pub(super) fn action_text_input_char(&mut self, c: char) {
        if let Some(input) = &mut self.text_input {
            let byte = input
                .value
                .char_indices()
                .nth(input.cursor_pos)
                .map(|(b, _)| b)
                .unwrap_or(input.value.len());
            input.value.insert(byte, c);
            input.cursor_pos += 1;
        }
    }

    pub(super) fn action_text_input_backspace(&mut self) {
        if let Some(input) = &mut self.text_input {
            if input.cursor_pos > 0 {
                let prev = input
                    .value
                    .char_indices()
                    .nth(input.cursor_pos - 1)
                    .map(|(b, _)| b)
                    .unwrap_or(0);
                input.value.remove(prev);
                input.cursor_pos -= 1;
            }
        }
    }

    pub(super) fn action_text_input_delete(&mut self) {
        if let Some(input) = &mut self.text_input {
            if input.cursor_pos < input.value.chars().count() {
                let at = input
                    .value
                    .char_indices()
                    .nth(input.cursor_pos)
                    .map(|(b, _)| b)
                    .unwrap_or(input.value.len());
                input.value.remove(at);
            }
        }
    }

    pub(super) fn action_text_input_left(&mut self) {
        if let Some(input) = &mut self.text_input {
            input.cursor_pos = input.cursor_pos.saturating_sub(1);
        }
    }

    pub(super) fn action_text_input_right(&mut self) {
        if let Some(input) = &mut self.text_input {
            if input.cursor_pos < input.value.chars().count() {
                input.cursor_pos += 1;
            }
        }
    }

    pub(super) fn action_text_input_home(&mut self) {
        if let Some(input) = &mut self.text_input {
            input.cursor_pos = 0;
        }
    }

    pub(super) fn action_text_input_end(&mut self) {
        if let Some(input) = &mut self.text_input {
            input.cursor_pos = input.value.chars().count();
        }
    }
}
