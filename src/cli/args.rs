//! CLI argument definitions using Clap

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wenv")]
#[command(about = "Shell 配置文件管理工具 - Shell configuration file manager")]
#[command(version)]
#[command(author)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 指定配置檔路徑 (Specify configuration file path)
    #[arg(short, long, global = true)]
    pub file: Option<PathBuf>,

    /// 指定 shell 類型 (Specify shell type)
    #[arg(short, long, global = true)]
    pub shell: Option<ShellArg>,

    /// 衝突處理策略 (Conflict handling strategy)
    #[arg(long, global = true, default_value = "ask")]
    pub on_conflict: ConflictStrategy,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 列出條目 (List entries)
    #[command(visible_alias = "ls")]
    List {
        /// 條目類型 (Entry type): alias|func|env|source (a/f/e/s)
        entry_type: Option<EntryTypeArg>,
    },

    /// 檢查問題 (Check for issues)
    Check,

    /// 顯示條目詳細資訊 (Show entry details)
    #[command(visible_alias = "i")]
    Info {
        /// 條目類型 (Entry type): a/f/e/s
        entry_type: EntryTypeArg,
        /// 條目名稱 (Entry name)
        name: String,
    },

    /// 新增條目 (Add entry)
    Add {
        #[command(subcommand)]
        add_command: AddCommands,
    },

    /// 刪除條目 (Remove entry)
    #[command(visible_alias = "rm")]
    Remove {
        /// 條目類型 (Entry type): a/f/e/s
        entry_type: EntryTypeArg,
        /// 條目名稱 (Entry name)
        name: String,
    },

    /// 編輯條目 (Edit entry)
    /// 使用 "edit ." 直接在編輯器中打開配置檔 (Use "edit ." to open config file in editor)
    Edit {
        /// 條目類型 (Entry type): a/f/e/s，或使用 "." 直接編輯配置檔
        entry_type: Option<String>,
        /// 條目名稱 (Entry name)
        name: Option<String>,
    },

    /// 匯入條目 (Import entries)
    Import {
        /// 檔案路徑或 URL (File path or URL)
        source: String,
        /// 跳過預覽確認 (Skip preview confirmation)
        #[arg(short, long)]
        yes: bool,
    },

    /// 匯出條目 (Export entries)
    Export {
        /// 條目類型 (Entry type): a/f/e/s
        entry_type: Option<EntryTypeArg>,
        /// 輸出檔案 (Output file)
        #[arg(short, long)]
        output: PathBuf,
    },

    /// 格式化配置檔 (Format configuration file)
    Format {
        /// 僅顯示變更，不寫入 (Dry run - show changes without writing)
        #[arg(long)]
        dry_run: bool,
    },

    /// 備份管理 (Backup management)
    Backup {
        #[command(subcommand)]
        backup_command: BackupCommands,
    },

    /// 互動式終端介面 (Interactive TUI mode)
    Tui,
}

#[derive(Subcommand)]
pub enum AddCommands {
    /// 新增 alias (Add alias)
    #[command(visible_alias = "a")]
    Alias {
        /// NAME=VALUE 格式 (NAME=VALUE format)
        definition: String,
    },
    /// 新增 function (Add function)
    #[command(visible_alias = "f")]
    Func {
        /// 函數名稱 (Function name)
        name: String,
        /// 函數內容 (Function body)
        body: String,
    },
    /// 新增環境變數 (Add environment variable)
    #[command(visible_alias = "e")]
    Env {
        /// NAME=VALUE 格式 (NAME=VALUE format)
        definition: String,
    },
    /// 新增 source (Add source)
    #[command(visible_alias = "s")]
    Source {
        /// 檔案路徑 (File path)
        path: String,
    },
}

#[derive(Subcommand)]
pub enum BackupCommands {
    /// 列出備份 (List backups)
    List,
    /// 還原備份 (Restore backup)
    Restore {
        /// 備份 ID (Backup ID)
        id: String,
    },
    /// 清理舊備份 (Clean old backups)
    Clean {
        /// 保留份數 (Number to keep)
        #[arg(long, default_value = "20")]
        keep: usize,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellArg {
    Bash,
    Pwsh,
}

impl From<ShellArg> for crate::model::ShellType {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => crate::model::ShellType::Bash,
            ShellArg::Pwsh => crate::model::ShellType::PowerShell,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum EntryTypeArg {
    #[value(alias = "a")]
    Alias,
    #[value(alias = "f")]
    Func,
    #[value(alias = "e")]
    Env,
    #[value(alias = "s")]
    Source,
    #[value(alias = "c")]
    Code,
    #[value(alias = "cm")]
    Comment,
}

impl From<EntryTypeArg> for crate::model::EntryType {
    fn from(arg: EntryTypeArg) -> Self {
        match arg {
            EntryTypeArg::Alias => crate::model::EntryType::Alias,
            EntryTypeArg::Func => crate::model::EntryType::Function,
            EntryTypeArg::Env => crate::model::EntryType::EnvVar,
            EntryTypeArg::Source => crate::model::EntryType::Source,
            EntryTypeArg::Code => crate::model::EntryType::Code,
            EntryTypeArg::Comment => crate::model::EntryType::Comment,
        }
    }
}

impl std::str::FromStr for EntryTypeArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "alias" | "a" => Ok(EntryTypeArg::Alias),
            "func" | "function" | "f" => Ok(EntryTypeArg::Func),
            "env" | "envvar" | "e" => Ok(EntryTypeArg::Env),
            "source" | "s" => Ok(EntryTypeArg::Source),
            "code" | "c" => Ok(EntryTypeArg::Code),
            "comment" | "cm" => Ok(EntryTypeArg::Comment),
            _ => Err(format!(
                "Invalid entry type '{}'. Must be one of: alias (a), func (f), env (e), source (s), code (c), comment (cm)",
                s
            )),
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Default)]
pub enum ConflictStrategy {
    #[default]
    Ask,
    Skip,
    Overwrite,
}
