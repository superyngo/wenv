//! Internationalization (i18n) module for wenv
//!
//! Provides English UI messages.

use serde::Deserialize;
use std::sync::OnceLock;

/// All translatable messages in the application
#[derive(Debug, Clone)]
pub struct Messages {
    // === General ===
    pub cancelled: &'static str,
    pub no_entries_found: &'static str,

    // === Reload Hint ===
    pub reload_hint: &'static str,

    // === TUI Titles ===
    pub tui_title: &'static str,
    pub tui_help_title: &'static str,
    pub tui_entry_details_title: &'static str,

    // === TUI Prompts ===
    pub tui_confirm_delete_title: &'static str,
    pub tui_delete_prompt: &'static str,
    pub tui_yes_no: &'static str,
    pub tui_confirm_quit_hint: &'static str,

    // === TUI Messages ===
    pub tui_entry_deleted: &'static str,
    pub tui_msg_clipboard_empty: &'static str,
    pub tui_msg_nothing_to_undo: &'static str,

    // === TUI Help Keys ===
    pub tui_help_quit: &'static str,
    pub tui_help_save: &'static str,
    pub tui_help_delete: &'static str,
    pub tui_help_search: &'static str,
    pub tui_help_add: &'static str,
    pub tui_help_edit_entry: &'static str,
    pub tui_help_move: &'static str,
    pub tui_help_toggle_select: &'static str,
    pub tui_help_undo: &'static str,
    pub tui_help_paste: &'static str,
    pub tui_help_help_key: &'static str,
}

/// Temporary structure for deserializing TOML messages
#[derive(Debug, Deserialize)]
struct MessagesToml {
    // === General ===
    cancelled: String,
    no_entries_found: String,

    // === Reload Hint ===
    reload_hint: String,

    // === TUI Titles ===
    tui_title: String,
    tui_help_title: String,
    tui_entry_details_title: String,

    // === TUI Prompts ===
    tui_confirm_delete_title: String,
    tui_delete_prompt: String,
    tui_yes_no: String,
    tui_confirm_quit_hint: String,

    // === TUI Messages ===
    tui_entry_deleted: String,
    tui_msg_clipboard_empty: String,
    tui_msg_nothing_to_undo: String,

    // === TUI Help Keys ===
    tui_help_quit: String,
    tui_help_save: String,
    tui_help_delete: String,
    tui_help_search: String,
    tui_help_add: String,
    tui_help_edit_entry: String,
    tui_help_move: String,
    tui_help_toggle_select: String,
    tui_help_undo: String,
    tui_help_paste: String,
    tui_help_help_key: String,
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
            cancelled: leak!(toml.cancelled),
            no_entries_found: leak!(toml.no_entries_found),

            // === Reload Hint ===
            reload_hint: leak!(toml.reload_hint),

            // === TUI Titles ===
            tui_title: leak!(toml.tui_title),
            tui_help_title: leak!(toml.tui_help_title),
            tui_entry_details_title: leak!(toml.tui_entry_details_title),

            // === TUI Prompts ===
            tui_confirm_delete_title: leak!(toml.tui_confirm_delete_title),
            tui_delete_prompt: leak!(toml.tui_delete_prompt),
            tui_yes_no: leak!(toml.tui_yes_no),
            tui_confirm_quit_hint: leak!(toml.tui_confirm_quit_hint),

            // === TUI Messages ===
            tui_entry_deleted: leak!(toml.tui_entry_deleted),
            tui_msg_clipboard_empty: leak!(toml.tui_msg_clipboard_empty),
            tui_msg_nothing_to_undo: leak!(toml.tui_msg_nothing_to_undo),

            // === TUI Help Keys ===
            tui_help_quit: leak!(toml.tui_help_quit),
            tui_help_save: leak!(toml.tui_help_save),
            tui_help_delete: leak!(toml.tui_help_delete),
            tui_help_search: leak!(toml.tui_help_search),
            tui_help_add: leak!(toml.tui_help_add),
            tui_help_edit_entry: leak!(toml.tui_help_edit_entry),
            tui_help_move: leak!(toml.tui_help_move),
            tui_help_toggle_select: leak!(toml.tui_help_toggle_select),
            tui_help_undo: leak!(toml.tui_help_undo),
            tui_help_paste: leak!(toml.tui_help_paste),
            tui_help_help_key: leak!(toml.tui_help_help_key),
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
        let config_dir = crate::Config::config_dir(); // fallback: Config::config_dir retained for i18n; not used by resolve_or_create
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
