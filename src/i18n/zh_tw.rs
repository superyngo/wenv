//! Traditional Chinese (zh-TW) language messages

use super::Messages;
use std::sync::OnceLock;

static ZH_TW_MESSAGES: OnceLock<Messages> = OnceLock::new();

pub fn messages() -> &'static Messages {
    ZH_TW_MESSAGES.get_or_init(|| Messages {
        // === General ===
        no_entries_found: "找不到任何條目。",
        total_entries: "總計：{} 個條目",
        entry_not_found: "找不到條目：{} '{}'",
        entry_added: "已新增 {} '{}' = '{}'",
        entry_removed: "已移除 {} '{}'",
        entry_updated: "已更新 {} '{}'",
        skipped: "已跳過。",
        cancelled: "已取消。",

        // === Headers ===
        header_type: "類型",
        header_name: "名稱",
        header_value: "值",
        header_line: "行號：",
        header_lines: "行號：",
        header_comment: "註解：",
        header_raw: "原始：",

        // === Check Command ===
        no_issues_found: "未發現問題！",
        parse_warnings: "解析警告：",
        issues_found: "發現問題：",
        checked_entries: "已檢查 {} 個條目",
        found_errors_warnings: "發現 {} 個錯誤、{} 個警告、{} 個解析警告",
        found_warnings: "發現 {} 個警告、{} 個解析警告",

        // === Add/Remove/Edit ===
        already_exists_skip: "{} '{}' 已存在，跳過",
        already_exists_value: "{} '{}' 已存在，目前值為：{}",
        overwrite_prompt: "是否覆蓋？",
        remove_prompt: "是否移除此條目？",
        invalid_alias_format: "別名格式無效。請使用：NAME=VALUE",
        invalid_env_format: "環境變數格式無效。請使用：NAME=VALUE",

        // === Backup ===
        backup_created: "已建立備份：{}",
        backup_restored: "已從備份還原：{}",
        no_backups_found: "找不到任何備份。",
        backup_list_header: "可用的備份：",

        // === Format ===
        file_formatted: "檔案格式化成功",

        // === Import/Export ===
        imported_entries: "已匯入 {} 個條目",
        exported_entries: "已匯出 {} 個條目至 {}",

        // === Reload Hint ===
        reload_hint: "執行 '{}' 以套用變更",

        // === TUI ===
        tui_title: "wenv - {}",
        tui_entries: " 條目 ({}) ",
        tui_help_title: " 說明 ",
        tui_confirm_delete_title: " 確認刪除 ",
        tui_delete_prompt: "是否刪除此條目？",
        tui_yes_no: "[Y] 是 / [N] 否",
        tui_add_entry_title: " 新增條目 ",
        tui_edit_name_title: " 編輯名稱 ",
        tui_edit_value_title: " 編輯值 ",
        tui_input_title: " 輸入 ",
        tui_enter_submit_esc_cancel: "[Enter] 確認  [Esc] 取消",
        tui_entry_details_title: " 條目詳情 ",

        // TUI prompts
        tui_enter_entry_type: "輸入條目類型（alias/func/env/source）：",
        tui_enter_name: "輸入名稱：",
        tui_enter_value: "輸入值：",
        tui_invalid_type: "無效的類型。請嘗試：alias/func/env/source",
        tui_name_value_empty: "名稱和值不能為空",
        tui_edit_name_for: "編輯名稱：{}",
        tui_edit_value_for: "編輯 {} 的值：",

        // TUI messages
        tui_entry_deleted: "條目已刪除（尚未儲存）",
        tui_entry_added: "條目新增成功",
        tui_name_updated: "名稱更新成功",
        tui_value_updated: "值更新成功",
        tui_file_formatted: "檔案格式化成功",
        tui_no_issues: "未發現問題",
        tui_found_issues: "發現 {} 個問題",

        // TUI help text
        tui_help_navigate: "導航條目",
        tui_help_info: "顯示條目詳情",
        tui_help_delete: "刪除條目",
        tui_help_new: "建立新條目",
        tui_help_rename: "重新命名",
        tui_help_edit: "編輯值",
        tui_help_check: "檢查條目",
        tui_help_format: "格式化檔案",
        tui_help_help: "顯示此說明",
        tui_help_quit: "退出",

        // TUI status bar
        tui_status_normal: "[上/下]導航 [Home/End]跳至首尾 [PgUp/PgDn]翻頁 [Enter/i]詳情 [d]刪除 [n]新增 [r]重命名 [e]編輯 [c]檢查 [f]格式化 [?]說明 [q/Esc]退出",
        tui_status_detail: "[Enter/i/q/Esc]關閉",
        tui_status_help: "[q/Esc]關閉",
        tui_status_confirm_delete: "[y]是 [n]否 [Esc]取消",
        tui_status_input: "[Enter]確認 [Esc]取消 [Backspace]刪除",
        tui_status_exiting: "正在退出...",
    })
}
