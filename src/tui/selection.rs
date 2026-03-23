//! Selection state management for TUI

use std::collections::HashSet;
use crate::model::profile::ListItem;

pub struct SelectionState {
    pub selected_indices: HashSet<usize>,
    pub anchor: Option<usize>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected_indices: HashSet::new(),
            anchor: None,
        }
    }

    pub fn clear(&mut self) {
        self.selected_indices.clear();
        self.anchor = None;
    }

    /// Toggle selection at index. Only select Entry items, not FileHeaders.
    pub fn toggle(&mut self, index: usize, items: &[ListItem]) {
        if matches!(items.get(index), Some(ListItem::Entry(_, _))) {
            if self.selected_indices.contains(&index) {
                self.selected_indices.remove(&index);
            } else {
                self.selected_indices.insert(index);
            }
            self.anchor = Some(index);
        }
    }

    /// Extend selection range from anchor (or current) to target index.
    /// Only selects Entry items in the range.
    pub fn extend_range(&mut self, from: usize, to: usize, items: &[ListItem]) {
        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        for i in start..=end {
            if matches!(items.get(i), Some(ListItem::Entry(_, _))) {
                self.selected_indices.insert(i);
            }
        }
        self.anchor = Some(to);
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    pub fn selected_count(&self) -> usize {
        self.selected_indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected_indices.is_empty()
    }

    /// Return selected indices sorted ascending
    pub fn sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self.selected_indices.iter().copied().collect();
        indices.sort();
        indices
    }
}