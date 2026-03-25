//! Selection state management for TUI

use std::collections::HashSet;
use crate::model::profile::ListItem;

pub struct SelectionState {
    pub selected_indices: HashSet<usize>,   // manually toggled (Space)
    range_indices: HashSet<usize>,          // shift-arrow range selection
    pub anchor: Option<usize>,              // anchor for Space toggle
    shift_anchor: Option<usize>,            // fixed anchor for shift-arrow range
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected_indices: HashSet::new(),
            range_indices: HashSet::new(),
            anchor: None,
            shift_anchor: None,
        }
    }

    pub fn clear(&mut self) {
        self.selected_indices.clear();
        self.range_indices.clear();
        self.anchor = None;
        self.shift_anchor = None;
    }

    /// Toggle selection at index. Only select Entry items, not FileHeaders.
    /// Also commits any pending range selection and clears shift_anchor.
    pub fn toggle(&mut self, index: usize, items: &[ListItem]) {
        self.commit_range();
        if matches!(items.get(index), Some(ListItem::Entry(_, _))) {
            if self.selected_indices.contains(&index) {
                self.selected_indices.remove(&index);
            } else {
                self.selected_indices.insert(index);
            }
            self.anchor = Some(index);
        }
    }

    /// Start or continue a shift-arrow range selection.
    /// The shift_anchor is set once and stays fixed; the range dynamically
    /// updates between shift_anchor and the current cursor.
    pub fn set_range(&mut self, cursor: usize, items: &[ListItem]) {
        if self.shift_anchor.is_none() {
            self.shift_anchor = Some(cursor);
            return;
        }
        let anchor = self.shift_anchor.unwrap();
        self.range_indices.clear();
        let (start, end) = if anchor <= cursor { (anchor, cursor) } else { (cursor, anchor) };
        for i in start..=end {
            if matches!(items.get(i), Some(ListItem::Entry(_, _))) {
                self.range_indices.insert(i);
            }
        }
    }

    /// Commit range selection into manual selection and clear shift state.
    /// Call this when the user does a non-shift operation (normal move, etc.).
    pub fn commit_range(&mut self) {
        if !self.range_indices.is_empty() {
            self.selected_indices.extend(&self.range_indices);
            self.range_indices.clear();
        }
        self.shift_anchor = None;
    }

    /// Clear range selection without committing (discard).
    pub fn discard_range(&mut self) {
        self.range_indices.clear();
        self.shift_anchor = None;
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index) || self.range_indices.contains(&index)
    }

    pub fn selected_count(&self) -> usize {
        self.selected_indices.union(&self.range_indices).count()
    }

    pub fn is_empty(&self) -> bool {
        self.selected_indices.is_empty() && self.range_indices.is_empty()
    }

    /// Return selected indices sorted ascending (both manual and range)
    pub fn sorted_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self.selected_indices.union(&self.range_indices).copied().collect();
        indices.sort();
        indices
    }

    pub fn has_shift_anchor(&self) -> bool {
        self.shift_anchor.is_some()
    }
}