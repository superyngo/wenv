//! Multi-file profile model

use crate::model::{Entry, ShellType};
use std::path::PathBuf;

/// Item in the flat visible list for TUI navigation
#[derive(Debug, Clone, PartialEq)]
pub enum ListItem {
    FileHeader(usize),   // index into ShellProfile.files
    Entry(usize, usize), // (file_index, entry_index)
}

/// A single configuration file with its parsed entries
pub struct ProfileFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub content: String,
    pub expanded: bool,
    pub dirty: bool,
    pub exists: bool,
}

/// All configuration files for one shell session
pub struct ShellProfile {
    pub shell_type: ShellType,
    pub files: Vec<ProfileFile>,
}

impl ProfileFile {
    pub fn new(path: PathBuf, exists: bool) -> Self {
        Self {
            path,
            entries: Vec::new(),
            content: String::new(),
            expanded: false,
            dirty: false,
            exists,
        }
    }

    pub fn new_with_entries(path: PathBuf, entries: Vec<Entry>, expanded: bool) -> Self {
        Self {
            path,
            entries,
            content: String::new(),
            expanded,
            dirty: false,
            exists: true,
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn display_name(&self) -> String {
        let path_str = self.path.to_string_lossy();
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy();
            if path_str.starts_with(home_str.as_ref()) {
                return format!("~{}", &path_str[home_str.len()..]);
            }
        }
        path_str.to_string()
    }
}

impl ShellProfile {
    pub fn build_visible_list(&self) -> Vec<ListItem> {
        let mut items = Vec::new();
        for (fi, file) in self.files.iter().enumerate() {
            items.push(ListItem::FileHeader(fi));
            if file.expanded {
                for ei in 0..file.entries.len() {
                    items.push(ListItem::Entry(fi, ei));
                }
            }
        }
        items
    }

    pub fn total_entries(&self) -> usize {
        self.files.iter().map(|f| f.entries.len()).sum()
    }

    pub fn any_dirty(&self) -> bool {
        self.files.iter().any(|f| f.dirty)
    }

    pub fn dirty_files(&self) -> Vec<&ProfileFile> {
        self.files.iter().filter(|f| f.dirty).collect()
    }

    pub fn toggle_all(&mut self, expanded: bool) {
        for file in &mut self.files {
            file.expanded = expanded;
        }
    }
}

use crate::config::path_resolver;
use crate::model::Config;
use crate::parser::get_parser;

pub fn load_shell_profile(config: &Config, shell_type: ShellType) -> anyhow::Result<ShellProfile> {
    let shell_key = shell_type.config_key();
    let file_configs = config
        .files
        .get(shell_key)
        .ok_or_else(|| anyhow::anyhow!("No file list for {}", shell_key))?;

    let resolved = path_resolver::resolve_paths(&file_configs.paths);
    let parser = get_parser(shell_type);

    let mut files = Vec::new();
    for (path, exists) in resolved {
        let mut pf = ProfileFile::new(path.clone(), exists);
        if exists {
            let content = std::fs::read_to_string(&path)?;
            let result = parser.parse(&content);
            let file_idx = files.len();
            for mut entry in result.entries {
                entry.file_index = file_idx;
                pf.entries.push(entry);
            }
            pf.content = content;
        }
        pf.expanded = exists; // auto-expand files that exist
        files.push(pf);
    }

    Ok(ShellProfile { shell_type, files })
}
