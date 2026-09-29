//! Internationalization (i18n) module for wenv
//!
//! Provides English and localized UI messages.

use std::collections::HashMap;
use std::sync::OnceLock;

/// All translatable messages in the application, loaded from TOML catalogs.
#[derive(Debug, Clone)]
pub struct Messages {
    map: HashMap<String, String>,
}

impl Messages {
    /// Look up a translated message by key, falling back to the key itself if not found.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.map.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// Look up a translated message template by key and substitute positional `{}` placeholders
    /// with the provided arguments in order.
    pub fn fmt(&self, key: &str, args: &[&dyn std::fmt::Display]) -> String {
        let template = self.get(key);
        let mut result = String::with_capacity(template.len() + args.len() * 16);
        let mut arg_iter = args.iter();
        let mut rest = template;

        while let Some(pos) = rest.find("{}") {
            result.push_str(&rest[..pos]);
            if let Some(arg) = arg_iter.next() {
                use std::fmt::Write;
                let _ = write!(result, "{}", arg);
            } else {
                result.push_str("{}");
            }
            rest = &rest[pos + 2..];
        }
        result.push_str(rest);
        result
    }

    /// Number of message keys currently loaded in the catalog.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True when the catalog contains no messages.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Check if the catalog contains a translation for `key`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
}

/// Embedded English messages (fallback)
const EMBEDDED_EN: &str = include_str!("../../assets/i18n/en.toml");

/// Global messages instance
static MESSAGES: OnceLock<Messages> = OnceLock::new();

/// Languages compiled into the binary (external `i18n/<lang>.toml` files still take priority).
const EMBEDDED_LANGUAGES: &[(&str, &str)] =
    &[("zh-TW", include_str!("../../languages/zh-TW.toml"))];

/// Build messages: embedded English overlaid per key with the requested language, so a
/// missing key in a translation falls back to English instead of failing the whole file.
fn load_messages_from_toml(lang: &str) -> Messages {
    let mut base: toml::Table =
        toml::from_str(EMBEDDED_EN).expect("Failed to parse embedded English messages");

    if lang != "en" {
        let lang_file = crate::Config::config_dir()
            .join("i18n")
            .join(format!("{}.toml", lang));
        let source = std::fs::read_to_string(&lang_file).ok().or_else(|| {
            EMBEDDED_LANGUAGES
                .iter()
                .find(|(name, _)| *name == lang)
                .map(|(_, src)| src.to_string())
        });
        match source {
            Some(src) => match toml::from_str::<toml::Table>(&src) {
                Ok(table) => {
                    for (key, value) in table {
                        if value.is_str() && base.contains_key(&key) {
                            base.insert(key, value);
                        }
                    }
                }
                Err(e) => eprintln!("Warning: Failed to parse language '{}': {}", lang, e),
            },
            None => eprintln!("Warning: Unknown language '{}', using English", lang),
        }
    }

    let mut map = HashMap::with_capacity(base.len());
    for (key, value) in base {
        if let toml::Value::String(s) = value {
            map.insert(key, s);
        }
    }

    Messages { map }
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
    use super::*;

    use regex::Regex;

    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_load_embedded_english() {
        let messages = super::init_messages("en");
        assert_eq!(messages.get("no_entries_found"), "No entries found.");
    }

    #[test]
    fn test_embedded_zh_tw_loads_and_missing_keys_fall_back() {
        let zh = super::load_messages_from_toml("zh-TW");
        assert_ne!(zh.get("tui_help_quit"), "Quit");
        let en = super::load_messages_from_toml("en");
        assert_eq!(en.get("tui_help_quit"), "Quit");
    }

    #[test]
    fn test_en_and_zh_tw_have_identical_keys() {
        let en_table: toml::Table =
            toml::from_str(EMBEDDED_EN).expect("Failed to parse embedded en.toml");
        let zh_source = EMBEDDED_LANGUAGES
            .iter()
            .find(|(name, _)| *name == "zh-TW")
            .map(|(_, src)| *src)
            .expect("zh-TW embedded language must exist");
        let zh_table: toml::Table =
            toml::from_str(zh_source).expect("Failed to parse embedded zh-TW.toml");

        let en_keys: BTreeSet<&str> = en_table.keys().map(String::as_str).collect();
        let zh_keys: BTreeSet<&str> = zh_table.keys().map(String::as_str).collect();

        let missing_in_zh: Vec<_> = en_keys.difference(&zh_keys).copied().collect();
        let missing_in_en: Vec<_> = zh_keys.difference(&en_keys).copied().collect();

        assert!(
            missing_in_zh.is_empty(),
            "Keys present in en.toml but missing in zh-TW.toml: {:?}",
            missing_in_zh
        );
        assert!(
            missing_in_en.is_empty(),
            "Keys present in zh-TW.toml but missing in en.toml: {:?}",
            missing_in_en
        );
    }

    #[test]
    fn test_placeholder_counts_match_between_en_and_zh_tw() {
        let en_table: toml::Table =
            toml::from_str(EMBEDDED_EN).expect("Failed to parse embedded en.toml");
        let zh_source = EMBEDDED_LANGUAGES
            .iter()
            .find(|(name, _)| *name == "zh-TW")
            .map(|(_, src)| *src)
            .expect("zh-TW embedded language must exist");
        let zh_table: toml::Table =
            toml::from_str(zh_source).expect("Failed to parse embedded zh-TW.toml");

        for (key, en_val) in &en_table {
            let en_str = en_val.as_str().expect("en value must be string");
            let zh_val = zh_table.get(key).unwrap_or_else(|| {
                panic!("Key '{}' missing in zh-TW", key);
            });
            let zh_str = zh_val.as_str().expect("zh value must be string");

            let en_count = en_str.matches("{}").count();
            let zh_count = zh_str.matches("{}").count();

            assert_eq!(
                en_count, zh_count,
                "Placeholder count mismatch for key '{}': en has {}, zh-TW has {}",
                key, en_count, zh_count
            );
        }
    }

    fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_rs_files(&path, files);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    files.push(path);
                }
            }
        }
    }

    #[test]
    fn test_all_tui_catalog_keys_exist_in_en() {
        let en_table: toml::Table =
            toml::from_str(EMBEDDED_EN).expect("Failed to parse embedded en.toml");

        let tui_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
        let mut rs_files = Vec::new();
        collect_rs_files(&tui_dir, &mut rs_files);
        assert!(!rs_files.is_empty(), "src/tui rs files must be found");

        let re = Regex::new(
            r#"(?:messages(?:\(\))?|app\.messages|self\.messages)\.(?:get|fmt)\(\s*"([^"]+)""#,
        )
        .expect("Valid regex");

        let mut found_keys = BTreeSet::new();
        for file in &rs_files {
            let content = std::fs::read_to_string(file)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));
            for cap in re.captures_iter(&content) {
                let key = cap.get(1).unwrap().as_str().to_string();
                found_keys.insert(key);
            }
        }

        assert!(
            found_keys.len() >= 50,
            "Expected at least 50 catalog keys found in src/tui, got {}",
            found_keys.len()
        );

        let mut missing_keys = Vec::new();
        for key in &found_keys {
            if !en_table.contains_key(key) {
                missing_keys.push(key.clone());
            }
        }

        assert!(
            missing_keys.is_empty(),
            "Keys used in src/tui but missing from en.toml: {:?}",
            missing_keys
        );
    }

    #[test]
    fn test_messages_fmt() {
        let en = super::load_messages_from_toml("en");
        let formatted = en.fmt("msg_config_save_error", &[&"permission denied"]);
        assert_eq!(formatted, "Config save error: permission denied");

        // Key fallback when not in catalog
        let fallback = en.fmt("non_existent_template_{}", &[&"val"]);
        assert_eq!(fallback, "non_existent_template_val");
    }
}
