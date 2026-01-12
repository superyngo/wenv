//! Internationalization (i18n) module for wenv
//!
//! Provides multi-language support for UI messages.

mod en;
mod zh_tw;

use std::sync::OnceLock;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    English,
    TraditionalChinese,
    SimplifiedChinese,
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "en" | "english" => Ok(Language::English),
            "zh-tw" | "zh_tw" | "traditional" => Ok(Language::TraditionalChinese),
            "zh-cn" | "zh_cn" | "simplified" => Ok(Language::SimplifiedChinese),
            _ => Err(format!("Unknown language: {}", s)),
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::English => write!(f, "en"),
            Language::TraditionalChinese => write!(f, "zh-TW"),
            Language::SimplifiedChinese => write!(f, "zh-CN"),
        }
    }
}

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
    pub file_formatted: &'static str,

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
    pub tui_entry_added: &'static str,
    pub tui_name_updated: &'static str,
    pub tui_value_updated: &'static str,
    pub tui_file_formatted: &'static str,
    pub tui_no_issues: &'static str,
    pub tui_found_issues: &'static str,

    // TUI help text
    pub tui_help_navigate: &'static str,
    pub tui_help_info: &'static str,
    pub tui_help_delete: &'static str,
    pub tui_help_new: &'static str,
    pub tui_help_rename: &'static str,
    pub tui_help_edit: &'static str,
    pub tui_help_check: &'static str,
    pub tui_help_format: &'static str,
    pub tui_help_help: &'static str,
    pub tui_help_quit: &'static str,

    // TUI status bar
    pub tui_status_normal: &'static str,
    pub tui_status_detail: &'static str,
    pub tui_status_help: &'static str,
    pub tui_status_confirm_delete: &'static str,
    pub tui_status_input: &'static str,
    pub tui_status_exiting: &'static str,
}

/// Global messages instance
static MESSAGES: OnceLock<Messages> = OnceLock::new();

/// Get messages for the specified language
pub fn get_messages(lang: Language) -> &'static Messages {
    match lang {
        Language::English => en::messages(),
        Language::TraditionalChinese | Language::SimplifiedChinese => zh_tw::messages(),
    }
}

/// Initialize and get the global messages instance
pub fn init_messages(lang: Language) -> &'static Messages {
    MESSAGES.get_or_init(|| get_messages(lang).clone())
}

/// Get the current global messages (defaults to English if not initialized)
pub fn messages() -> &'static Messages {
    MESSAGES.get_or_init(|| en::messages().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_str() {
        assert_eq!("en".parse::<Language>().unwrap(), Language::English);
        assert_eq!(
            "zh-TW".parse::<Language>().unwrap(),
            Language::TraditionalChinese
        );
        assert_eq!(
            "zh-CN".parse::<Language>().unwrap(),
            Language::SimplifiedChinese
        );
    }

    #[test]
    fn test_language_display() {
        assert_eq!(format!("{}", Language::English), "en");
        assert_eq!(format!("{}", Language::TraditionalChinese), "zh-TW");
    }

    #[test]
    fn test_get_messages() {
        let en = get_messages(Language::English);
        assert_eq!(en.no_entries_found, "No entries found.");

        let zh = get_messages(Language::TraditionalChinese);
        assert_eq!(zh.no_entries_found, "找不到任何條目。");
    }
}
