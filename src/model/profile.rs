//! Multi-file profile model with three-layer (group → file → entry) tree.

use crate::model::{Entry, ShellType};
use std::path::PathBuf;

/// Item in the flat visible list for TUI navigation.
#[derive(Debug, Clone, PartialEq)]
pub enum ListItem {
    DirHeader(usize),    // index into ShellProfile.tree (must be Dir)
    FileHeader(usize),   // index into ShellProfile.files
    Entry(usize, usize), // (file_index, entry_index)
}

/// A single configuration file with its parsed entries.
pub struct ProfileFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub content: String,
    pub expanded: bool,
    pub dirty: bool,
    pub exists: bool,
    pub writable: bool,
    /// Display string from path_resolver (may contain "(varname)" suffix).
    pub display_label: String,
}

/// Top-level tree node. Groups bundle multiple files from a glob/dir/var
/// pattern; standalone Files have no Group parent.
pub enum TreeNode {
    Dir(DirGroup),
    File(usize), // index into ShellProfile.files
}

/// A top-level group bundling files from a single pattern (glob, dir, or
/// var that resolves to either). Identity = `source_pattern` (used as the
/// removal key when the user presses `d` on a group header).
pub struct DirGroup {
    pub source_pattern: String,
    pub display_label: String,
    pub file_indices: Vec<usize>,
    pub expanded: bool,
}

pub struct ShellProfile {
    pub shell_type: ShellType,
    pub files: Vec<ProfileFile>,
    pub tree: Vec<TreeNode>,
}

impl ProfileFile {
    pub fn new(path: PathBuf, exists: bool, display_label: String) -> Self {
        Self {
            path,
            entries: Vec::new(),
            content: String::new(),
            expanded: false,
            dirty: false,
            exists,
            writable: true,
            display_label,
        }
    }

    /// Convenience constructor for tests: create a file with known entries.
    pub fn new_with_entries(path: PathBuf, entries: Vec<Entry>, expanded: bool) -> Self {
        let exists = true;
        let display_label = path.to_string_lossy().to_string();
        Self {
            path,
            entries,
            content: String::new(),
            expanded,
            dirty: false,
            exists,
            writable: true,
            display_label,
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Recalculate line_number / end_line / name for all entries.
    pub fn recalculate_line_numbers(&mut self) {
        let mut current_line = 1usize;
        for entry in &mut self.entries {
            let line_count = entry.value.split('\n').count();
            entry.line_number = Some(current_line);
            let end = current_line + line_count - 1;
            entry.end_line = if end > current_line {
                Some(end)
            } else {
                entry.line_number
            };

            match entry.entry_type {
                crate::model::EntryType::Comment => {
                    entry.name = if end > current_line {
                        format!("#L{}-L{}", current_line, end)
                    } else {
                        format!("#L{}", current_line)
                    };
                }
                crate::model::EntryType::Code => {
                    entry.name = if end > current_line {
                        format!("L{}-L{}", current_line, end)
                    } else {
                        format!("L{}", current_line)
                    };
                }
                _ => {}
            }
            current_line = end + 1;
        }
    }

    /// Tilde-collapsed display name (for legacy callers; new code uses display_label).
    pub fn display_name(&self) -> String {
        if !self.display_label.is_empty() {
            return self.display_label.clone();
        }
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
        for (ti, node) in self.tree.iter().enumerate() {
            match node {
                TreeNode::Dir(g) => {
                    items.push(ListItem::DirHeader(ti));
                    if g.expanded {
                        for &fi in &g.file_indices {
                            items.push(ListItem::FileHeader(fi));
                            if self.files[fi].expanded {
                                for ei in 0..self.files[fi].entries.len() {
                                    items.push(ListItem::Entry(fi, ei));
                                }
                            }
                        }
                    }
                }
                TreeNode::File(fi) => {
                    items.push(ListItem::FileHeader(*fi));
                    if self.files[*fi].expanded {
                        for ei in 0..self.files[*fi].entries.len() {
                            items.push(ListItem::Entry(*fi, ei));
                        }
                    }
                }
            }
        }
        items
    }

    pub fn build_visible_list_filtered(
        &self,
        matched_files: &std::collections::HashSet<usize>,
    ) -> Vec<ListItem> {
        let mut items = Vec::new();
        for (ti, node) in self.tree.iter().enumerate() {
            match node {
                TreeNode::Dir(g) => {
                    let any_match = g.file_indices.iter().any(|fi| matched_files.contains(fi));
                    if !any_match {
                        continue;
                    }
                    items.push(ListItem::DirHeader(ti));
                    if g.expanded {
                        for &fi in &g.file_indices {
                            if !matched_files.contains(&fi) {
                                continue;
                            }
                            items.push(ListItem::FileHeader(fi));
                            if self.files[fi].expanded {
                                for ei in 0..self.files[fi].entries.len() {
                                    items.push(ListItem::Entry(fi, ei));
                                }
                            }
                        }
                    }
                }
                TreeNode::File(fi) => {
                    if !matched_files.contains(fi) {
                        continue;
                    }
                    items.push(ListItem::FileHeader(*fi));
                    if self.files[*fi].expanded {
                        for ei in 0..self.files[*fi].entries.len() {
                            items.push(ListItem::Entry(*fi, ei));
                        }
                    }
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
        for f in &mut self.files {
            f.expanded = expanded;
        }
        for n in &mut self.tree {
            if let TreeNode::Dir(g) = n {
                g.expanded = expanded;
            }
        }
    }

    /// Build a ShellProfile where every file maps to a top-level TreeNode::File.
    /// Convenience for tests and legacy construction.
    pub fn from_files(shell_type: ShellType, files: Vec<ProfileFile>) -> Self {
        let tree = (0..files.len()).map(TreeNode::File).collect();
        Self {
            shell_type,
            files,
            tree,
        }
    }
}

use crate::config::path_resolver::{self, ResolvedPattern};
use crate::model::Config;
use crate::parser::get_parser;

fn derive_file_display_in_group(path: &std::path::Path, original: &str) -> String {
    let tilde = {
        let s = path.to_string_lossy();
        if let Some(home) = dirs::home_dir() {
            let h = home.to_string_lossy();
            if s.starts_with(h.as_ref()) {
                format!("~{}", &s[h.len()..])
            } else {
                s.into_owned()
            }
        } else {
            s.into_owned()
        }
    };

    let re_unix = regex::Regex::new(r"\$[A-Za-z_][A-Za-z0-9_]*").unwrap();
    let re_win = regex::Regex::new(r"%[A-Za-z_][A-Za-z0-9_]*%").unwrap();
    let var_bearing = re_unix.is_match(original) || re_win.is_match(original);
    if !var_bearing {
        return tilde;
    }

    let prefix = if let Some(idx) = original.rfind('/') {
        original[..idx].to_string()
    } else {
        original.to_string()
    };

    let basename = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let var_form = if prefix.ends_with('/') {
        format!("{}{}", prefix, basename)
    } else {
        format!("{}/{}", prefix, basename)
    };

    format!("{} ({})", tilde, var_form)
}

pub fn load_shell_profile(config: &Config, shell_type: ShellType) -> anyhow::Result<ShellProfile> {
    let shell_key = shell_type.config_key();
    let file_configs = config
        .files
        .get(shell_key)
        .ok_or_else(|| anyhow::anyhow!("No file list for {}", shell_key))?;

    let resolved = path_resolver::resolve_patterns(&file_configs.paths);
    let parser = get_parser(shell_type);

    let mut files: Vec<ProfileFile> = Vec::new();
    let mut tree: Vec<TreeNode> = Vec::new();
    let mut seen_paths: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for rp in resolved {
        match rp {
            ResolvedPattern::File {
                path,
                exists,
                display,
                ..
            } => {
                if seen_paths.contains(&path) {
                    continue; // dedup
                }
                seen_paths.insert(path.clone());
                let fi = files.len();
                let mut pf = ProfileFile::new(path.clone(), exists, display);
                if exists {
                    let content = std::fs::read_to_string(&path)?;
                    let result = parser.parse(&content);
                    for mut entry in result.entries {
                        entry.file_index = fi;
                        pf.entries.push(entry);
                    }
                    pf.content = content;
                }
                pf.expanded = false;
                files.push(pf);
                tree.push(TreeNode::File(fi));
            }
            ResolvedPattern::Dir {
                original,
                display,
                files: dir_files,
            } => {
                let mut indices: Vec<usize> = Vec::new();
                for (path, exists) in dir_files {
                    if seen_paths.contains(&path) {
                        continue; // dedup
                    }
                    seen_paths.insert(path.clone());
                    let fi = files.len();
                    let mut pf = ProfileFile::new(
                        path.clone(),
                        exists,
                        derive_file_display_in_group(&path, &original),
                    );
                    if exists {
                        let content = std::fs::read_to_string(&path)?;
                        let result = parser.parse(&content);
                        for mut entry in result.entries {
                            entry.file_index = fi;
                            pf.entries.push(entry);
                        }
                        pf.content = content;
                    }
                    pf.expanded = false;
                    files.push(pf);
                    indices.push(fi);
                }
                tree.push(TreeNode::Dir(DirGroup {
                    source_pattern: original,
                    display_label: display,
                    file_indices: indices,
                    expanded: false,
                }));
            }
        }
    }

    Ok(ShellProfile {
        shell_type,
        files,
        tree,
    })
}
