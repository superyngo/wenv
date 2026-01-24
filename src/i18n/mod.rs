//! Internationalization (i18n) module for wenv
//!
//! Provides English UI messages.

use serde::Deserialize;
use std::sync::OnceLock;

/// All translatable messages in the application
#[derive(Debug, Clone)]
pub struct Messages {
    // === General ===
    pub no_entries_found: &'static str,
    pub total_entries: &'static str,
    pub entry_not_found: &'static str,
    pub entry_added: &'static str,
    pub entry_removed: &'static str,
    pub entry_updated: &'static str,
    pub skipped: &'static str,
    pub cancelled: &'static str,

    // === Headers ===
    pub header_type: &'static str,
    pub header_name: &'static str,
    pub header_value: &'static str,
    pub header_line_num: &'static str,
    pub header_line: &'static str,
    pub header_lines: &'static str,
    pub header_comment: &'static str,
    pub header_raw: &'static str,

    // === Check Command ===
    pub no_issues_found: &'static str,
    pub parse_warnings: &'static str,
    pub issues_found: &'static str,
    pub checked_entries: &'static str,
    pub found_errors_warnings: &'static str,
    pub found_warnings: &'static str,

    // === Add/Remove/Edit ===
    pub already_exists_skip: &'static str,
    pub already_exists_value: &'static str,
    pub overwrite_prompt: &'static str,
    pub remove_prompt: &'static str,
    pub invalid_alias_format: &'static str,
    pub invalid_env_format: &'static str,

    // === Backup ===
    pub backup_created: &'static str,
    pub backup_restored: &'static str,
    pub no_backups_found: &'static str,
    pub backup_list_header: &'static str,

    // === Format ===
    pub msg_file_formatted: &'static str,

    // === Import/Export ===
    pub imported_entries: &'static str,
    pub exported_entries: &'static str,

    // === Reload Hint ===
    pub reload_hint: &'static str,

    // === TUI ===
    pub tui_title: &'static str,
    pub tui_entries: &'static str,
    pub tui_help_title: &'static str,
    pub tui_confirm_delete_title: &'static str,
    pub tui_delete_prompt: &'static str,
    pub tui_yes_no: &'static str,
    pub tui_add_entry_title: &'static str,
    pub tui_edit_name_title: &'static str,
    pub tui_edit_value_title: &'static str,
    pub tui_input_title: &'static str,
    pub tui_enter_submit_esc_cancel: &'static str,
    pub tui_entry_details_title: &'static str,

    // TUI prompts
    pub tui_enter_entry_type: &'static str,
    pub tui_enter_name: &'static str,
    pub tui_enter_value: &'static str,
    pub tui_invalid_type: &'static str,
    pub tui_name_value_empty: &'static str,
    pub tui_edit_name_for: &'static str,
    pub tui_edit_value_for: &'static str,

    // TUI messages
    pub tui_entry_deleted: &'static str,
    pub msg_entry_added: &'static str,
    pub tui_name_updated: &'static str,
    pub tui_value_updated: &'static str,
    pub tui_no_issues: &'static str,
    pub tui_found_issues: &'static str,

    // TUI help text (old fields - only keep ones still in TOML)
    pub tui_help_delete: &'static str,
    pub tui_help_check: &'static str,
    pub tui_help_quit: &'static str,

    // TUI status bar
    pub tui_status_normal: &'static str,
    pub tui_status_detail: &'static str,
    pub tui_status_help: &'static str,
    pub tui_status_confirm_delete: &'static str,
    pub tui_status_input: &'static str,
    pub tui_status_exiting: &'static str,

    // TUI new status messages (Phase 3)
    pub tui_status_searching: &'static str,
    pub tui_status_detail_extended: &'static str,
    pub tui_status_confirm_delete_extended: &'static str,
    pub tui_status_confirm_quit: &'static str,
    pub tui_status_confirm_format: &'static str,
    pub tui_status_confirm_save_errors: &'static str,
    pub tui_status_selecting_type: &'static str,
    pub tui_status_editing: &'static str,
    pub tui_status_moving: &'static str,

    // TUI search popup
    pub tui_search_title: &'static str,
    pub tui_search_query: &'static str,
    pub tui_search_matches: &'static str,
    pub tui_search_no_matches: &'static str,
    pub tui_search_hint: &'static str,

    // TUI detail/edit labels (shared)
    pub label_type: &'static str,
    pub label_name: &'static str,
    pub label_value: &'static str,
    pub tui_detail_hint: &'static str,

    // TUI help popup
    pub tui_help_keyboard_shortcuts: &'static str,
    pub tui_help_section_navigation: &'static str,
    pub tui_help_section_editing: &'static str,
    pub tui_help_section_other: &'static str,

    // TUI confirm/edit popup
    pub tui_confirm_quit_title: &'static str,
    pub tui_confirm_quit_msg: &'static str,
    pub tui_confirm_quit_question: &'static str,
    pub tui_type_alias_desc: &'static str,
    pub tui_type_func_desc: &'static str,
    pub tui_type_env_desc: &'static str,
    pub tui_type_source_desc: &'static str,
    pub tui_type_code_desc: &'static str,
    pub tui_type_comment_desc: &'static str,
    pub tui_edit_submit: &'static str,

    // TUI dynamic messages
    pub tui_msg_selection_cleared: &'static str,
    pub tui_msg_use_arrows_to_move: &'static str,
    pub tui_msg_moving_entries: &'static str,
    pub tui_msg_move_cancelled: &'static str,
    pub tui_msg_moved_entries: &'static str,
    pub tui_msg_entries_deleted: &'static str,
    pub tui_msg_format_cancelled_validation: &'static str,
    pub tui_msg_save_cancelled_validation: &'static str,
    pub tui_msg_review_changes: &'static str,
    pub tui_msg_validation_failed: &'static str,
    pub tui_msg_format_cancelled: &'static str,
    pub tui_msg_format_bypassed: &'static str,
    pub tui_msg_select_entry_type: &'static str,
    pub tui_msg_failed_consolidate: &'static str,
    pub tui_msg_name_empty: &'static str,
    pub tui_msg_source_path_empty: &'static str,
    pub tui_msg_invalid_path_format: &'static str,
    pub tui_msg_alias_value_empty: &'static str,
    pub tui_msg_formatter_empty: &'static str,
    pub tui_msg_entry_updated: &'static str,
    pub tui_msg_file_saved: &'static str,
    pub tui_msg_file_saved_bypassed: &'static str,
    pub tui_msg_undo_successful: &'static str,
    pub tui_msg_nothing_to_undo: &'static str,
    pub tui_msg_redo_successful: &'static str,
    pub tui_msg_nothing_to_redo: &'static str,
    pub tui_msg_temp_file_reloaded: &'static str,
    pub tui_msg_no_changes_detected: &'static str,
    pub tui_msg_editor_error: &'static str,
    pub tui_msg_entries_selected: &'static str,
    pub tui_msg_entry_selected: &'static str,
    pub tui_msg_no_entry_to_copy: &'static str,
    pub tui_msg_entries_copied: &'static str,
    pub tui_msg_entry_copied: &'static str,
    pub tui_msg_entry_pasted: &'static str,
    pub tui_msg_clipboard_empty: &'static str,

    // TUI help detailed shortcuts
    pub tui_help_nav_updown: &'static str,
    pub tui_help_nav_scroll: &'static str,
    pub tui_help_nav_home_end: &'static str,
    pub tui_help_nav_pgup_pgdn: &'static str,
    pub tui_help_search: &'static str,
    pub tui_help_info_detail: &'static str,
    pub tui_help_add: &'static str,
    pub tui_help_edit_entry: &'static str,
    pub tui_help_move: &'static str,
    pub tui_help_toggle_select: &'static str,
    pub tui_help_select_range: &'static str,
    pub tui_help_format_file: &'static str,
    pub tui_help_save: &'static str,
    pub tui_help_undo: &'static str,
    pub tui_help_redo: &'static str,
    pub tui_help_copy: &'static str,
    pub tui_help_paste: &'static str,
    pub tui_help_help_key: &'static str,

    // TUI edit hints
    pub tui_hint_tab_next: &'static str,
    pub tui_hint_arrows_navigate: &'static str,
    pub tui_hint_enter_submit: &'static str,
    pub tui_hint_esc_cancel: &'static str,

    // TUI additional popup elements
    pub tui_edit_hint: &'static str,
    pub tui_edit_hint_source: &'static str,
    pub tui_type_select_hint: &'static str,
    pub tui_confirm_delete_hint: &'static str,
    pub tui_confirm_quit_hint: &'static str,
    pub tui_format_preview_title: &'static str,
    pub tui_validation_error_title: &'static str,
    pub tui_validation_error_hint: &'static str,
    pub tui_submit_button: &'static str,
    pub tui_submit_button_focused: &'static str,
    pub tui_help_edit_mode: &'static str,
    pub tui_help_next_field: &'static str,
    pub tui_help_prev_field: &'static str,
    pub tui_help_submit_on_button: &'static str,
    pub tui_help_cancel: &'static str,
    pub tui_delete_multi_prompt: &'static str,

    // TUI format preview messages
    pub tui_fmt_sorting_aliases: &'static str,
    pub tui_fmt_sorting_functions: &'static str,
    pub tui_fmt_sorting_envvars: &'static str,
    pub tui_fmt_sorting_sources: &'static str,
    pub tui_fmt_grouping_entries: &'static str,
    pub tui_fmt_no_changes_needed: &'static str,

    // TUI toggle comment messages
    pub tui_msg_entry_toggled: &'static str,
    pub tui_msg_entries_toggled: &'static str,
}

/// Temporary structure for deserializing TOML messages
#[derive(Debug, Deserialize)]
struct MessagesToml {
    // === General ===
    no_entries_found: String,
    total_entries: String,
    entry_not_found: String,
    entry_added: String,
    entry_removed: String,
    entry_updated: String,
    skipped: String,
    cancelled: String,

    // === Headers ===
    header_type: String,
    header_name: String,
    header_value: String,
    header_line_num: String,
    header_line: String,
    header_lines: String,
    header_comment: String,
    header_raw: String,

    // === Check Command ===
    no_issues_found: String,
    parse_warnings: String,
    issues_found: String,
    checked_entries: String,
    found_errors_warnings: String,
    found_warnings: String,

    // === Add/Remove/Edit ===
    already_exists_skip: String,
    already_exists_value: String,
    overwrite_prompt: String,
    remove_prompt: String,
    invalid_alias_format: String,
    invalid_env_format: String,

    // === Backup ===
    backup_created: String,
    backup_restored: String,
    no_backups_found: String,
    backup_list_header: String,

    // === Format ===
    msg_file_formatted: String,

    // === Import/Export ===
    imported_entries: String,
    exported_entries: String,

    // === Reload Hint ===
    reload_hint: String,

    // === TUI ===
    tui_title: String,
    tui_entries: String,
    tui_help_title: String,
    tui_confirm_delete_title: String,
    tui_delete_prompt: String,
    tui_yes_no: String,
    tui_add_entry_title: String,
    tui_edit_name_title: String,
    tui_edit_value_title: String,
    tui_input_title: String,
    tui_enter_submit_esc_cancel: String,
    tui_entry_details_title: String,

    // TUI prompts
    tui_enter_entry_type: String,
    tui_enter_name: String,
    tui_enter_value: String,
    tui_invalid_type: String,
    tui_name_value_empty: String,
    tui_edit_name_for: String,
    tui_edit_value_for: String,

    // TUI messages
    tui_entry_deleted: String,
    msg_entry_added: String,
    tui_name_updated: String,
    tui_value_updated: String,
    tui_no_issues: String,
    tui_found_issues: String,

    // TUI status bar
    tui_status_normal: String,
    tui_status_detail: String,
    tui_status_help: String,
    tui_status_confirm_delete: String,
    tui_status_input: String,
    tui_status_exiting: String,

    // TUI new status messages (Phase 3)
    tui_status_searching: String,
    tui_status_detail_extended: String,
    tui_status_confirm_delete_extended: String,
    tui_status_confirm_quit: String,
    tui_status_confirm_format: String,
    tui_status_confirm_save_errors: String,
    tui_status_selecting_type: String,
    tui_status_editing: String,
    tui_status_moving: String,

    // TUI search popup
    tui_search_title: String,
    tui_search_query: String,
    tui_search_matches: String,
    tui_search_no_matches: String,
    tui_search_hint: String,

    // TUI detail/edit labels (shared)
    label_type: String,
    label_name: String,
    label_value: String,
    tui_detail_hint: String,

    // TUI help popup
    tui_help_keyboard_shortcuts: String,
    tui_help_section_navigation: String,
    tui_help_section_editing: String,
    tui_help_section_other: String,

    // TUI help text (old fields - only keep ones still in TOML)
    tui_help_delete: String,
    tui_help_check: String,
    tui_help_quit: String,

    // TUI confirm/edit popup
    tui_confirm_quit_title: String,
    tui_confirm_quit_msg: String,
    tui_confirm_quit_question: String,
    tui_type_alias_desc: String,
    tui_type_func_desc: String,
    tui_type_env_desc: String,
    tui_type_source_desc: String,
    tui_type_code_desc: String,
    tui_type_comment_desc: String,
    tui_edit_submit: String,

    // TUI dynamic messages
    tui_msg_selection_cleared: String,
    tui_msg_use_arrows_to_move: String,
    tui_msg_moving_entries: String,
    tui_msg_move_cancelled: String,
    tui_msg_moved_entries: String,
    tui_msg_entries_deleted: String,
    tui_msg_format_cancelled_validation: String,
    tui_msg_save_cancelled_validation: String,
    tui_msg_review_changes: String,
    tui_msg_validation_failed: String,
    tui_msg_format_cancelled: String,
    tui_msg_format_bypassed: String,
    tui_msg_select_entry_type: String,
    tui_msg_failed_consolidate: String,
    tui_msg_name_empty: String,
    tui_msg_source_path_empty: String,
    tui_msg_invalid_path_format: String,
    tui_msg_alias_value_empty: String,
    tui_msg_formatter_empty: String,
    tui_msg_entry_updated: String,
    tui_msg_file_saved: String,
    tui_msg_file_saved_bypassed: String,
    tui_msg_undo_successful: String,
    tui_msg_nothing_to_undo: String,
    tui_msg_redo_successful: String,
    tui_msg_nothing_to_redo: String,
    tui_msg_temp_file_reloaded: String,
    tui_msg_no_changes_detected: String,
    tui_msg_editor_error: String,
    tui_msg_entries_selected: String,
    tui_msg_entry_selected: String,
    tui_msg_no_entry_to_copy: String,
    tui_msg_entries_copied: String,
    tui_msg_entry_copied: String,
    tui_msg_entry_pasted: String,
    tui_msg_clipboard_empty: String,

    // TUI help detailed shortcuts
    tui_help_nav_updown: String,
    tui_help_nav_scroll: String,
    tui_help_nav_home_end: String,
    tui_help_nav_pgup_pgdn: String,
    tui_help_search: String,
    tui_help_info_detail: String,
    tui_help_add: String,
    tui_help_edit_entry: String,
    tui_help_move: String,
    tui_help_toggle_select: String,
    tui_help_select_range: String,
    tui_help_format_file: String,
    tui_help_save: String,
    tui_help_undo: String,
    tui_help_redo: String,
    tui_help_copy: String,
    tui_help_paste: String,
    tui_help_help_key: String,

    // TUI edit hints
    tui_hint_tab_next: String,
    tui_hint_arrows_navigate: String,
    tui_hint_enter_submit: String,
    tui_hint_esc_cancel: String,

    // TUI additional popup elements
    tui_edit_hint: String,
    tui_edit_hint_source: String,
    tui_type_select_hint: String,
    tui_confirm_delete_hint: String,
    tui_confirm_quit_hint: String,
    tui_format_preview_title: String,
    tui_validation_error_title: String,
    tui_validation_error_hint: String,
    tui_submit_button: String,
    tui_submit_button_focused: String,
    tui_help_edit_mode: String,
    tui_help_next_field: String,
    tui_help_prev_field: String,
    tui_help_submit_on_button: String,
    tui_help_cancel: String,
    tui_delete_multi_prompt: String,

    // TUI format preview messages
    tui_fmt_sorting_aliases: String,
    tui_fmt_sorting_functions: String,
    tui_fmt_sorting_envvars: String,
    tui_fmt_sorting_sources: String,
    tui_fmt_grouping_entries: String,
    tui_fmt_no_changes_needed: String,

    // TUI toggle comment messages
    tui_msg_entry_toggled: String,
    tui_msg_entries_toggled: String,
}

/// Helper macro to leak a string and get a &'static str
macro_rules! leak {
    ($s:expr) => {
        Box::leak($s.into_boxed_str())
    };
}

impl From<MessagesToml> for Messages {
    fn from(toml: MessagesToml) -> Self {
        Messages {
            // === General ===
            no_entries_found: leak!(toml.no_entries_found),
            total_entries: leak!(toml.total_entries),
            entry_not_found: leak!(toml.entry_not_found),
            entry_added: leak!(toml.entry_added),
            entry_removed: leak!(toml.entry_removed),
            entry_updated: leak!(toml.entry_updated),
            skipped: leak!(toml.skipped),
            cancelled: leak!(toml.cancelled),

            // === Headers ===
            header_type: leak!(toml.header_type),
            header_name: leak!(toml.header_name),
            header_value: leak!(toml.header_value),
            header_line_num: leak!(toml.header_line_num),
            header_line: leak!(toml.header_line),
            header_lines: leak!(toml.header_lines),
            header_comment: leak!(toml.header_comment),
            header_raw: leak!(toml.header_raw),

            // === Check Command ===
            no_issues_found: leak!(toml.no_issues_found),
            parse_warnings: leak!(toml.parse_warnings),
            issues_found: leak!(toml.issues_found),
            checked_entries: leak!(toml.checked_entries),
            found_errors_warnings: leak!(toml.found_errors_warnings),
            found_warnings: leak!(toml.found_warnings),

            // === Add/Remove/Edit ===
            already_exists_skip: leak!(toml.already_exists_skip),
            already_exists_value: leak!(toml.already_exists_value),
            overwrite_prompt: leak!(toml.overwrite_prompt),
            remove_prompt: leak!(toml.remove_prompt),
            invalid_alias_format: leak!(toml.invalid_alias_format),
            invalid_env_format: leak!(toml.invalid_env_format),

            // === Backup ===
            backup_created: leak!(toml.backup_created),
            backup_restored: leak!(toml.backup_restored),
            no_backups_found: leak!(toml.no_backups_found),
            backup_list_header: leak!(toml.backup_list_header),

            // === Format ===
            msg_file_formatted: leak!(toml.msg_file_formatted),

            // === Import/Export ===
            imported_entries: leak!(toml.imported_entries),
            exported_entries: leak!(toml.exported_entries),

            // === Reload Hint ===
            reload_hint: leak!(toml.reload_hint),

            // === TUI ===
            tui_title: leak!(toml.tui_title),
            tui_entries: leak!(toml.tui_entries),
            tui_help_title: leak!(toml.tui_help_title),
            tui_confirm_delete_title: leak!(toml.tui_confirm_delete_title),
            tui_delete_prompt: leak!(toml.tui_delete_prompt),
            tui_yes_no: leak!(toml.tui_yes_no),
            tui_add_entry_title: leak!(toml.tui_add_entry_title),
            tui_edit_name_title: leak!(toml.tui_edit_name_title),
            tui_edit_value_title: leak!(toml.tui_edit_value_title),
            tui_input_title: leak!(toml.tui_input_title),
            tui_enter_submit_esc_cancel: leak!(toml.tui_enter_submit_esc_cancel),
            tui_entry_details_title: leak!(toml.tui_entry_details_title),

            // TUI prompts
            tui_enter_entry_type: leak!(toml.tui_enter_entry_type),
            tui_enter_name: leak!(toml.tui_enter_name),
            tui_enter_value: leak!(toml.tui_enter_value),
            tui_invalid_type: leak!(toml.tui_invalid_type),
            tui_name_value_empty: leak!(toml.tui_name_value_empty),
            tui_edit_name_for: leak!(toml.tui_edit_name_for),
            tui_edit_value_for: leak!(toml.tui_edit_value_for),

            // TUI messages
            tui_entry_deleted: leak!(toml.tui_entry_deleted),
            msg_entry_added: leak!(toml.msg_entry_added),
            tui_name_updated: leak!(toml.tui_name_updated),
            tui_value_updated: leak!(toml.tui_value_updated),
            tui_no_issues: leak!(toml.tui_no_issues),
            tui_found_issues: leak!(toml.tui_found_issues),

            // TUI status bar
            tui_status_normal: leak!(toml.tui_status_normal),
            tui_status_detail: leak!(toml.tui_status_detail),
            tui_status_help: leak!(toml.tui_status_help),
            tui_status_confirm_delete: leak!(toml.tui_status_confirm_delete),
            tui_status_input: leak!(toml.tui_status_input),
            tui_status_exiting: leak!(toml.tui_status_exiting),

            // TUI new status messages (Phase 3)
            tui_status_searching: leak!(toml.tui_status_searching),
            tui_status_detail_extended: leak!(toml.tui_status_detail_extended),
            tui_status_confirm_delete_extended: leak!(toml.tui_status_confirm_delete_extended),
            tui_status_confirm_quit: leak!(toml.tui_status_confirm_quit),
            tui_status_confirm_format: leak!(toml.tui_status_confirm_format),
            tui_status_confirm_save_errors: leak!(toml.tui_status_confirm_save_errors),
            tui_status_selecting_type: leak!(toml.tui_status_selecting_type),
            tui_status_editing: leak!(toml.tui_status_editing),
            tui_status_moving: leak!(toml.tui_status_moving),

            // TUI search popup
            tui_search_title: leak!(toml.tui_search_title),
            tui_search_query: leak!(toml.tui_search_query),
            tui_search_matches: leak!(toml.tui_search_matches),
            tui_search_no_matches: leak!(toml.tui_search_no_matches),
            tui_search_hint: leak!(toml.tui_search_hint),

            // TUI detail/edit labels (shared)
            label_type: leak!(toml.label_type),
            label_name: leak!(toml.label_name),
            label_value: leak!(toml.label_value),
            tui_detail_hint: leak!(toml.tui_detail_hint),

            // TUI help popup
            tui_help_keyboard_shortcuts: leak!(toml.tui_help_keyboard_shortcuts),
            tui_help_section_navigation: leak!(toml.tui_help_section_navigation),
            tui_help_section_editing: leak!(toml.tui_help_section_editing),
            tui_help_section_other: leak!(toml.tui_help_section_other),

            // TUI help text (old fields - only keep ones still in TOML)
            tui_help_delete: leak!(toml.tui_help_delete),
            tui_help_check: leak!(toml.tui_help_check),
            tui_help_quit: leak!(toml.tui_help_quit),

            // TUI confirm/edit popup
            tui_confirm_quit_title: leak!(toml.tui_confirm_quit_title),
            tui_confirm_quit_msg: leak!(toml.tui_confirm_quit_msg),
            tui_confirm_quit_question: leak!(toml.tui_confirm_quit_question),
            tui_type_alias_desc: leak!(toml.tui_type_alias_desc),
            tui_type_func_desc: leak!(toml.tui_type_func_desc),
            tui_type_env_desc: leak!(toml.tui_type_env_desc),
            tui_type_source_desc: leak!(toml.tui_type_source_desc),
            tui_type_code_desc: leak!(toml.tui_type_code_desc),
            tui_type_comment_desc: leak!(toml.tui_type_comment_desc),
            tui_edit_submit: leak!(toml.tui_edit_submit),

            // TUI dynamic messages
            tui_msg_selection_cleared: leak!(toml.tui_msg_selection_cleared),
            tui_msg_use_arrows_to_move: leak!(toml.tui_msg_use_arrows_to_move),
            tui_msg_moving_entries: leak!(toml.tui_msg_moving_entries),
            tui_msg_move_cancelled: leak!(toml.tui_msg_move_cancelled),
            tui_msg_moved_entries: leak!(toml.tui_msg_moved_entries),
            tui_msg_entries_deleted: leak!(toml.tui_msg_entries_deleted),
            tui_msg_format_cancelled_validation: leak!(toml.tui_msg_format_cancelled_validation),
            tui_msg_save_cancelled_validation: leak!(toml.tui_msg_save_cancelled_validation),
            tui_msg_review_changes: leak!(toml.tui_msg_review_changes),
            tui_msg_validation_failed: leak!(toml.tui_msg_validation_failed),
            tui_msg_format_cancelled: leak!(toml.tui_msg_format_cancelled),
            tui_msg_format_bypassed: leak!(toml.tui_msg_format_bypassed),
            tui_msg_select_entry_type: leak!(toml.tui_msg_select_entry_type),
            tui_msg_failed_consolidate: leak!(toml.tui_msg_failed_consolidate),
            tui_msg_name_empty: leak!(toml.tui_msg_name_empty),
            tui_msg_source_path_empty: leak!(toml.tui_msg_source_path_empty),
            tui_msg_invalid_path_format: leak!(toml.tui_msg_invalid_path_format),
            tui_msg_alias_value_empty: leak!(toml.tui_msg_alias_value_empty),
            tui_msg_formatter_empty: leak!(toml.tui_msg_formatter_empty),
            tui_msg_entry_updated: leak!(toml.tui_msg_entry_updated),
            tui_msg_file_saved: leak!(toml.tui_msg_file_saved),
            tui_msg_file_saved_bypassed: leak!(toml.tui_msg_file_saved_bypassed),
            tui_msg_undo_successful: leak!(toml.tui_msg_undo_successful),
            tui_msg_nothing_to_undo: leak!(toml.tui_msg_nothing_to_undo),
            tui_msg_redo_successful: leak!(toml.tui_msg_redo_successful),
            tui_msg_nothing_to_redo: leak!(toml.tui_msg_nothing_to_redo),
            tui_msg_temp_file_reloaded: leak!(toml.tui_msg_temp_file_reloaded),
            tui_msg_no_changes_detected: leak!(toml.tui_msg_no_changes_detected),
            tui_msg_editor_error: leak!(toml.tui_msg_editor_error),
            tui_msg_entries_selected: leak!(toml.tui_msg_entries_selected),
            tui_msg_entry_selected: leak!(toml.tui_msg_entry_selected),
            tui_msg_no_entry_to_copy: leak!(toml.tui_msg_no_entry_to_copy),
            tui_msg_entries_copied: leak!(toml.tui_msg_entries_copied),
            tui_msg_entry_copied: leak!(toml.tui_msg_entry_copied),
            tui_msg_entry_pasted: leak!(toml.tui_msg_entry_pasted),
            tui_msg_clipboard_empty: leak!(toml.tui_msg_clipboard_empty),

            // TUI help detailed shortcuts
            tui_help_nav_updown: leak!(toml.tui_help_nav_updown),
            tui_help_nav_scroll: leak!(toml.tui_help_nav_scroll),
            tui_help_nav_home_end: leak!(toml.tui_help_nav_home_end),
            tui_help_nav_pgup_pgdn: leak!(toml.tui_help_nav_pgup_pgdn),
            tui_help_search: leak!(toml.tui_help_search),
            tui_help_info_detail: leak!(toml.tui_help_info_detail),
            tui_help_add: leak!(toml.tui_help_add),
            tui_help_edit_entry: leak!(toml.tui_help_edit_entry),
            tui_help_move: leak!(toml.tui_help_move),
            tui_help_toggle_select: leak!(toml.tui_help_toggle_select),
            tui_help_select_range: leak!(toml.tui_help_select_range),
            tui_help_format_file: leak!(toml.tui_help_format_file),
            tui_help_save: leak!(toml.tui_help_save),
            tui_help_undo: leak!(toml.tui_help_undo),
            tui_help_redo: leak!(toml.tui_help_redo),
            tui_help_copy: leak!(toml.tui_help_copy),
            tui_help_paste: leak!(toml.tui_help_paste),
            tui_help_help_key: leak!(toml.tui_help_help_key),

            // TUI edit hints
            tui_hint_tab_next: leak!(toml.tui_hint_tab_next),
            tui_hint_arrows_navigate: leak!(toml.tui_hint_arrows_navigate),
            tui_hint_enter_submit: leak!(toml.tui_hint_enter_submit),
            tui_hint_esc_cancel: leak!(toml.tui_hint_esc_cancel),

            // TUI additional popup elements
            tui_edit_hint: leak!(toml.tui_edit_hint),
            tui_edit_hint_source: leak!(toml.tui_edit_hint_source),
            tui_type_select_hint: leak!(toml.tui_type_select_hint),
            tui_confirm_delete_hint: leak!(toml.tui_confirm_delete_hint),
            tui_confirm_quit_hint: leak!(toml.tui_confirm_quit_hint),
            tui_format_preview_title: leak!(toml.tui_format_preview_title),
            tui_validation_error_title: leak!(toml.tui_validation_error_title),
            tui_validation_error_hint: leak!(toml.tui_validation_error_hint),
            tui_submit_button: leak!(toml.tui_submit_button),
            tui_submit_button_focused: leak!(toml.tui_submit_button_focused),
            tui_help_edit_mode: leak!(toml.tui_help_edit_mode),
            tui_help_next_field: leak!(toml.tui_help_next_field),
            tui_help_prev_field: leak!(toml.tui_help_prev_field),
            tui_help_submit_on_button: leak!(toml.tui_help_submit_on_button),
            tui_help_cancel: leak!(toml.tui_help_cancel),
            tui_delete_multi_prompt: leak!(toml.tui_delete_multi_prompt),

            // TUI format preview messages
            tui_fmt_sorting_aliases: leak!(toml.tui_fmt_sorting_aliases),
            tui_fmt_sorting_functions: leak!(toml.tui_fmt_sorting_functions),
            tui_fmt_sorting_envvars: leak!(toml.tui_fmt_sorting_envvars),
            tui_fmt_sorting_sources: leak!(toml.tui_fmt_sorting_sources),
            tui_fmt_grouping_entries: leak!(toml.tui_fmt_grouping_entries),
            tui_fmt_no_changes_needed: leak!(toml.tui_fmt_no_changes_needed),

            // TUI toggle comment messages
            tui_msg_entry_toggled: leak!(toml.tui_msg_entry_toggled),
            tui_msg_entries_toggled: leak!(toml.tui_msg_entries_toggled),
        }
    }
}

/// Embedded English messages (fallback)
const EMBEDDED_EN: &str = include_str!("../../assets/i18n/en.toml");

/// Global messages instance
static MESSAGES: OnceLock<Messages> = OnceLock::new();

/// Load messages from external file or embedded English TOML
fn load_messages_from_toml(lang: &str) -> Messages {
    // If not English, try external file first
    if lang != "en" {
        let config_dir = crate::Config::config_dir();
        let lang_file = config_dir.join("i18n").join(format!("{}.toml", lang));

        match std::fs::read_to_string(&lang_file) {
            Ok(content) => match toml::from_str::<MessagesToml>(&content) {
                Ok(toml_messages) => return toml_messages.into(),
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse language file {}: {}",
                        lang_file.display(),
                        e
                    );
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read language file {}: {}",
                    lang_file.display(),
                    e
                );
            }
        }
    }

    // Fallback to embedded English
    let toml_messages: MessagesToml =
        toml::from_str(EMBEDDED_EN).expect("Failed to parse embedded English messages");
    toml_messages.into()
}

/// Initialize and get the global messages instance
pub fn init_messages(lang: &str) -> &'static Messages {
    MESSAGES.get_or_init(|| load_messages_from_toml(lang))
}

/// Get the current global messages (defaults to English if not initialized)
pub fn messages() -> &'static Messages {
    MESSAGES.get_or_init(|| load_messages_from_toml("en"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_load_embedded_english() {
        let messages = super::init_messages("en");
        assert_eq!(messages.no_entries_found, "No entries found.");
    }
}
