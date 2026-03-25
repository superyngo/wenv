//! Fuzzy search state and matching

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use crate::model::profile::ShellProfile;

pub struct SearchMatch {
    pub file_index: usize,
    pub entry_index: usize,
    pub score: i64,
}

pub struct SearchState {
    pub query: String,
    matcher: SkimMatcherV2,
    pub matches: Vec<SearchMatch>,
    pub selected: usize,  // index into position_order for navigation
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
            matcher: SkimMatcherV2::default(),
            matches: Vec::new(),
            selected: 0,
            position_order: Vec::new(),
        }
    }

    pub fn update_matches(&mut self, profile: &ShellProfile) {
        self.matches.clear();
        if self.query.is_empty() { return; }

        for (fi, file) in profile.files.iter().enumerate() {
            for (ei, entry) in file.entries.iter().enumerate() {
                let haystack = format!("{} {}", entry.name, entry.value);
                if let Some((score, _indices)) = self.matcher.fuzzy_indices(&haystack, &self.query) {
                    self.matches.push(SearchMatch {
                        file_index: fi,
                        entry_index: ei,
                        score,
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
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.query.pop();
    }

    /// Move to next match in file-position order
    pub fn select_next(&mut self) {
        if !self.position_order.is_empty() {
            self.selected = (self.selected + 1).min(self.position_order.len() - 1);
        }
    }

    /// Move to previous match in file-position order
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Returns (file_index, entry_index) of the currently selected match (position-ordered)
    pub fn current_match(&self) -> Option<(usize, usize)> {
        self.position_order.get(self.selected)
            .and_then(|&idx| self.matches.get(idx))
            .map(|m| (m.file_index, m.entry_index))
    }

    /// Check if a given (file_index, entry_index) is in the match set
    pub fn is_match(&self, fi: usize, ei: usize) -> bool {
        self.matches.iter().any(|m| m.file_index == fi && m.entry_index == ei)
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
}