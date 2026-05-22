//! List navigation utilities

use crate::model::profile::ListItem;

pub fn navigate_up(_items: &[ListItem], current: usize) -> usize {
    if current > 0 {
        current - 1
    } else {
        current
    }
}

pub fn navigate_down(items: &[ListItem], current: usize) -> usize {
    if current + 1 < items.len() {
        current + 1
    } else {
        current
    }
}

pub fn navigate_home() -> usize {
    0
}

pub fn navigate_end(items: &[ListItem]) -> usize {
    if items.is_empty() {
        0
    } else {
        items.len() - 1
    }
}

pub fn file_index_of(item: &ListItem) -> usize {
    match item {
        ListItem::FileHeader(fi) => *fi,
        ListItem::Entry(fi, _) => *fi,
        ListItem::DirHeader(_) => 0, // DirHeader doesn't map to a file
    }
}
