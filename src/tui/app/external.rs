//! External editor integration and subprocess execution for TUI.

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use std::io;

use crate::tui::app::TuiApp;

impl TuiApp {
    pub(super) fn suspend_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }

    pub(super) fn resume_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        enable_raw_mode()?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(())
    }

    /// Edit a file: suspend TUI, open in $EDITOR, re-parse, resume
    pub(super) fn run_edit_file(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
    ) -> Result<()> {
        let path = self.profile.files[fi].path.clone();
        if !self.profile.files[fi].exists {
            self.message = Some(self.messages.fmt("msg_file_not_exist", &[&path.display()]));
            return Ok(());
        }
        if self.profile.files[fi].dirty {
            self.message = Some(self.messages.get("msg_unsaved_external_edit").into());
            return Ok(());
        }

        Self::suspend_tui(terminal)?;
        let modified = crate::tui::editor::edit_file(&path);
        Self::resume_tui(terminal)?;

        match modified {
            Ok(true) => {
                // Re-parse the file
                let parser = crate::parser::get_parser(self.profile.shell_type);
                if let Err(e) = self.profile.files[fi].load_from_disk(parser.as_ref(), fi) {
                    self.message = Some(self.messages.fmt("msg_reload_failed", &[&e]));
                    return Ok(());
                }
                self.rebuild_list();
                self.message = Some(self.messages.fmt("msg_reloaded", &[&path.display()]));
            }
            Ok(false) => {
                self.message = Some(self.messages.get("msg_no_changes_detected").into());
            }
            Err(e) => {
                self.message = Some(self.messages.fmt("msg_editor_error", &[&e]));
            }
        }
        Ok(())
    }

    /// Edit an entry: write value to temp file, open in $EDITOR, update entry
    pub(super) fn run_edit_entry(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
        ei: usize,
    ) -> Result<()> {
        let suffix = match self.profile.shell_type {
            crate::model::ShellType::PowerShell => ".ps1",
            _ => ".sh",
        };
        let value = self.profile.files[fi].entries[ei].value.clone();

        Self::suspend_tui(terminal)?;
        let result = crate::tui::editor::edit_temp_content(&value, suffix);
        Self::resume_tui(terminal)?;

        match result {
            Ok(Some(new_content)) => {
                let new_content = new_content
                    .strip_suffix('\n')
                    .map(str::to_string)
                    .unwrap_or(new_content);
                self.apply_edited_value(fi, ei, &value, &new_content);
            }
            Ok(None) => {
                self.message = Some(self.messages.get("msg_no_changes").into());
            }
            Err(e) => {
                self.message = Some(self.messages.fmt("msg_editor_error", &[&e]));
            }
        }
        Ok(())
    }

    /// Apply an edited entry value (from the external editor or the inline editor):
    /// push an undo snapshot, re-parse, and replace the entry. Empty content deletes
    /// the entry. No-op (with a "No changes" message) when `new_content == old_value`.
    pub(super) fn apply_edited_value(
        &mut self,
        fi: usize,
        ei: usize,
        old_value: &str,
        new_content: &str,
    ) {
        if new_content == old_value {
            self.message = Some(self.messages.get("msg_no_changes").into());
            return;
        }
        let snapshot = crate::tui::operations::take_snapshot(&self.profile);
        crate::tui::operations::push_undo(&mut self.undo_stack, &mut self.redo_stack, snapshot);
        let parser = crate::parser::get_parser(self.profile.shell_type);
        let parsed = parser.parse(new_content);
        let new_entries: Vec<_> = parsed
            .entries
            .into_iter()
            .map(|mut e| {
                e.file_index = fi;
                e
            })
            .collect();

        if new_entries.is_empty() {
            // Empty edit = delete entry
            self.profile.files[fi].entries.remove(ei);
            self.profile.files[fi].dirty = true;
            self.profile.files[fi].recalculate_line_numbers();
            self.rebuild_list();
            self.message = Some(self.messages.get("msg_entry_deleted_empty").into());
        } else {
            let count = crate::tui::operations::replace_entry_with_parsed(
                &mut self.profile.files[fi],
                ei,
                new_entries,
                fi,
            );
            self.rebuild_list();
            self.message = Some(if count == 1 {
                self.messages.get("msg_entry_updated").into()
            } else {
                self.messages
                    .fmt("msg_entry_replaced_with_count", &[&count])
            });
        }
    }

    /// Add a new entry: open empty (or template) temp file in $EDITOR, parse result, insert
    pub(super) fn run_add_entry(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        fi: usize,
        template: Option<&str>,
    ) -> Result<()> {
        let suffix = match self.profile.shell_type {
            crate::model::ShellType::PowerShell => ".ps1",
            _ => ".sh",
        };

        Self::suspend_tui(terminal)?;
        let initial_content = template.unwrap_or("");
        let result = crate::tui::editor::edit_temp_content(initial_content, suffix);
        Self::resume_tui(terminal)?;

        match result {
            Ok(Some(content)) => {
                let content = content
                    .strip_suffix('\n')
                    .map(str::to_string)
                    .unwrap_or(content);
                if !content.is_empty() {
                    let parser = crate::parser::get_parser(self.profile.shell_type);
                    let parsed = parser.parse(&content);
                    let mut new_entries: Vec<_> = parsed
                        .entries
                        .into_iter()
                        .map(|mut e| {
                            e.file_index = fi;
                            e
                        })
                        .collect();

                    if !new_entries.is_empty() {
                        let snapshot = crate::tui::operations::take_snapshot(&self.profile);
                        crate::tui::operations::push_undo(
                            &mut self.undo_stack,
                            &mut self.redo_stack,
                            snapshot,
                        );
                        // Insert after current entry position, or at end of file
                        let insert_pos = match self.visible_items.get(self.cursor) {
                            Some(crate::model::profile::ListItem::Entry(_, ei)) => ei + 1,
                            Some(crate::model::profile::ListItem::FileHeader(_)) => 0, // Insert at beginning
                            _ => self.profile.files[fi].entries.len(),
                        };
                        let count = new_entries.len();
                        // Insert entries
                        for (i, entry) in new_entries.drain(..).enumerate() {
                            self.profile.files[fi].entries.insert(insert_pos + i, entry);
                        }
                        self.profile.files[fi].dirty = true;
                        self.profile.files[fi].expanded = true;
                        self.profile.files[fi].recalculate_line_numbers();
                        self.rebuild_list();
                        self.message = Some(self.messages.fmt("msg_added_entries", &[&count]));
                    }
                } else {
                    self.message = Some(self.messages.get("msg_empty_nothing_added").into());
                }
            }
            Ok(None) => {
                self.message = Some(self.messages.get("msg_cancelled").into());
            }
            Err(e) => {
                self.message = Some(self.messages.fmt("msg_editor_error", &[&e]));
            }
        }
        Ok(())
    }
}
