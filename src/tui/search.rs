//! Fuzzy search state and matching

use crate::model::profile::ShellProfile;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub struct SearchMatch {
    pub file_index: usize,
    pub entry_index: usize,
    pub score: i64,
    pub indices: Vec<usize>, // matched character positions in "{name} {value}" haystack
}

pub struct SearchState {
    pub query: String,
    /// Char index of the editing caret within `query` (multibyte-safe).
    pub cursor: usize,
    matcher: SkimMatcherV2,
    pub matches: Vec<SearchMatch>,
    pub selected: usize, // index into position_order for navigation
    /// Indices into `matches` sorted by file position (for sequential navigation)
    position_order: Vec<usize>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            matcher: SkimMatcherV2::default(),
            matches: Vec::new(),
            selected: 0,
            position_order: Vec::new(),
        }
    }

    /// Byte offset of char index `i` in `query` (query.len() when past the end).
    fn byte_at(&self, i: usize) -> usize {
        self.query
            .char_indices()
            .nth(i)
            .map(|(b, _)| b)
            .unwrap_or(self.query.len())
    }

    pub fn update_matches(&mut self, profile: &ShellProfile) {
        self.matches.clear();
        if self.query.is_empty() {
            return;
        }

        for (fi, file) in profile.files.iter().enumerate() {
            for (ei, entry) in file.entries.iter().enumerate() {
                let haystack = format!("{} {}", entry.name, entry.value);
                if let Some((score, indices)) = self.matcher.fuzzy_indices(&haystack, &self.query) {
                    self.matches.push(SearchMatch {
                        file_index: fi,
                        entry_index: ei,
                        score,
                        indices,
                    });
                }
            }
        }

        // Build position_order: indices into matches sorted by (file_index, entry_index)
        self.position_order = (0..self.matches.len()).collect();
        self.position_order.sort_by(|&a, &b| {
            let ma = &self.matches[a];
            let mb = &self.matches[b];
            (ma.file_index, ma.entry_index).cmp(&(mb.file_index, mb.entry_index))
        });

        self.selected = 0;
    }

    pub fn input_char(&mut self, c: char) {
        let byte = self.byte_at(self.cursor);
        self.query.insert(byte, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.byte_at(self.cursor - 1);
            self.query.remove(prev);
            self.cursor -= 1;
        }
    }

    /// Forward-delete the char under the caret.
    pub fn delete(&mut self) {
        if self.cursor < self.query.chars().count() {
            let at = self.byte_at(self.cursor);
            self.query.remove(at);
        }
    }

    pub fn cursor_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn cursor_right(&mut self) {
        if self.cursor < self.query.chars().count() {
            self.cursor += 1;
        }
    }

    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor = self.query.chars().count();
    }

    /// Move to next match in file-position order (wraps around)
    pub fn select_next(&mut self) {
        if !self.position_order.is_empty() {
            self.selected = (self.selected + 1) % self.position_order.len();
        }
    }

    /// Move to previous match in file-position order (wraps around)
    pub fn select_prev(&mut self) {
        if !self.position_order.is_empty() {
            if self.selected == 0 {
                self.selected = self.position_order.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Jump to first match in file-position order
    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    /// Jump to last match in file-position order
    pub fn select_last(&mut self) {
        if !self.position_order.is_empty() {
            self.selected = self.position_order.len() - 1;
        }
    }

    /// Returns (file_index, entry_index) of the currently selected match (position-ordered)
    pub fn current_match(&self) -> Option<(usize, usize)> {
        self.position_order
            .get(self.selected)
            .and_then(|&idx| self.matches.get(idx))
            .map(|m| (m.file_index, m.entry_index))
    }

    /// Check if a given (file_index, entry_index) is in the match set
    pub fn is_match(&self, fi: usize, ei: usize) -> bool {
        self.matches
            .iter()
            .any(|m| m.file_index == fi && m.entry_index == ei)
    }

    /// Check if a given (file_index, entry_index) is the currently selected match
    pub fn is_selected_match(&self, fi: usize, ei: usize) -> bool {
        self.current_match()
            .is_some_and(|(f, e)| f == fi && e == ei)
    }

    /// Return set of file indices that have at least one match
    pub fn matched_file_indices(&self) -> std::collections::HashSet<usize> {
        self.matches.iter().map(|m| m.file_index).collect()
    }

    /// Return set of (file_index, entry_index) pairs that matched
    pub fn matched_entry_indices(&self) -> std::collections::HashSet<(usize, usize)> {
        self.matches
            .iter()
            .map(|m| (m.file_index, m.entry_index))
            .collect()
    }

    /// Get matched character indices for a given (file_index, entry_index).
    /// Returns indices split into (name_indices, value_indices) relative to
    /// the entry's name and value strings respectively.
    pub fn matched_char_indices(
        &self,
        fi: usize,
        ei: usize,
        name_len: usize,
    ) -> Option<(Vec<usize>, Vec<usize>)> {
        self.matches
            .iter()
            .find(|m| m.file_index == fi && m.entry_index == ei)
            .map(|m| {
                let mut name_indices = Vec::new();
                let mut value_indices = Vec::new();
                let separator = name_len; // index of the space between name and value
                for &idx in &m.indices {
                    if idx < name_len {
                        name_indices.push(idx);
                    } else if idx > separator {
                        value_indices.push(idx - separator - 1);
                    }
                }
                (name_indices, value_indices)
            })
    }
}
