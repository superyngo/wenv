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
    pub selected: usize,  // index into matches for navigation
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            matcher: SkimMatcherV2::default(),
            matches: Vec::new(),
            selected: 0,
        }
    }

    pub fn update_matches(&mut self, profile: &ShellProfile) {
        self.matches.clear();
        if self.query.is_empty() { return; }

        for (fi, file) in profile.files.iter().enumerate() {
            for (ei, entry) in file.entries.iter().enumerate() {
                // Search against both name and value
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
        self.matches.sort_by(|a, b| b.score.cmp(&a.score));
        // Reset selected to first match
        self.selected = 0;
    }

    pub fn input_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.query.pop();
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + 1).min(self.matches.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Returns (file_index, entry_index) of the currently selected match
    pub fn current_match(&self) -> Option<(usize, usize)> {
        self.matches.get(self.selected).map(|m| (m.file_index, m.entry_index))
    }

    /// Check if a given (file_index, entry_index) is in the match set
    pub fn is_match(&self, fi: usize, ei: usize) -> bool {
        self.matches.iter().any(|m| m.file_index == fi && m.entry_index == ei)
    }

    /// Check if a given (file_index, entry_index) is the currently selected match
    pub fn is_selected_match(&self, fi: usize, ei: usize) -> bool {
        self.matches.get(self.selected)
            .map_or(false, |m| m.file_index == fi && m.entry_index == ei)
    }
}