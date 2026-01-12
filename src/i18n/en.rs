//! English language messages

use super::Messages;
use std::sync::OnceLock;

static EN_MESSAGES: OnceLock<Messages> = OnceLock::new();

pub fn messages() -> &'static Messages {
    EN_MESSAGES.get_or_init(|| Messages {
        // === General ===
        no_entries_found: "No entries found.",
        total_entries: "Total: {} entries",
        entry_not_found: "Entry not found: {} '{}'",
        entry_added: "Added {} '{}' = '{}'",
        entry_removed: "Removed {} '{}'",
        entry_updated: "Updated {} '{}'",
        skipped: "Skipped.",
        cancelled: "Cancelled.",

        // === Headers ===
        header_type: "TYPE",
        header_name: "NAME",
        header_value: "VALUE",
        header_line: "Line:",
        header_lines: "Lines:",
        header_comment: "Comment:",
        header_raw: "Raw:",

        // === Check Command ===
        no_issues_found: "No issues found!",
        parse_warnings: "Parse Warnings:",
        issues_found: "Issues Found:",
        checked_entries: "Checked {} entries",
        found_errors_warnings: "Found {} error(s), {} warning(s), {} parse warning(s)",
        found_warnings: "Found {} warning(s), {} parse warning(s)",

        // === Add/Remove/Edit ===
        already_exists_skip: "{} '{}' already exists, skipping",
        already_exists_value: "{} '{}' already exists with value: {}",
        overwrite_prompt: "Overwrite?",
        remove_prompt: "Remove this entry?",
        invalid_alias_format: "Invalid alias format. Use: NAME=VALUE",
        invalid_env_format: "Invalid env format. Use: NAME=VALUE",

        // === Backup ===
        backup_created: "Backup created: {}",
        backup_restored: "Backup restored from: {}",
        no_backups_found: "No backups found.",
        backup_list_header: "Available backups:",

        // === Format ===
        file_formatted: "File formatted successfully",

        // === Import/Export ===
        imported_entries: "Imported {} entries",
        exported_entries: "Exported {} entries to {}",

        // === Reload Hint ===
        reload_hint: "Run '{}' to apply changes",

        // === TUI ===
        tui_title: "wenv - {}",
        tui_entries: " Entries ({}) ",
        tui_help_title: " Help ",
        tui_confirm_delete_title: " Confirm Delete ",
        tui_delete_prompt: "Delete this entry?",
        tui_yes_no: "[Y]es / [N]o",
        tui_add_entry_title: " Add New Entry ",
        tui_edit_name_title: " Edit Name ",
        tui_edit_value_title: " Edit Value ",
        tui_input_title: " Input ",
        tui_enter_submit_esc_cancel: "[Enter] Submit  [Esc] Cancel",
        tui_entry_details_title: " Entry Details ",

        // TUI prompts
        tui_enter_entry_type: "Enter entry type (alias/func/env/source):",
        tui_enter_name: "Enter name:",
        tui_enter_value: "Enter value:",
        tui_invalid_type: "Invalid type. Try: alias/func/env/source",
        tui_name_value_empty: "Name and value cannot be empty",
        tui_edit_name_for: "Edit name: {}",
        tui_edit_value_for: "Edit value for: {}",

        // TUI messages
        tui_entry_deleted: "Entry deleted (not saved yet)",
        tui_entry_added: "Entry added successfully",
        tui_name_updated: "Name updated successfully",
        tui_value_updated: "Value updated successfully",
        tui_file_formatted: "File formatted successfully",
        tui_no_issues: "No issues found",
        tui_found_issues: "Found {} issue(s)",

        // TUI help text
        tui_help_navigate: "Navigate entries",
        tui_help_info: "Show entry details",
        tui_help_delete: "Delete entry",
        tui_help_new: "Create new entry",
        tui_help_rename: "Rename entry",
        tui_help_edit: "Edit entry value",
        tui_help_check: "Check entries",
        tui_help_format: "Format file",
        tui_help_help: "Show this help",
        tui_help_quit: "Quit",

        // TUI status bar
        tui_status_normal: "[Up/Down]Navigate [Home/End]Jump [PgUp/PgDn]Page [Enter/i]Info [d]Delete [n]New [r]Rename [e]Edit [c]Check [f]Format [?]Help [q/Esc]Quit",
        tui_status_detail: "[Enter/i/q/Esc]Close",
        tui_status_help: "[q/Esc]Close",
        tui_status_confirm_delete: "[y]Yes [n]No [Esc]Cancel",
        tui_status_input: "[Enter]Submit [Esc]Cancel [Backspace]Delete",
        tui_status_exiting: "Exiting...",
    })
}
