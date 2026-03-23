# Multi-File Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor wenv from a single-file shell config manager to a multi-file TUI with $EDITOR integration, fuzzy search, and cross-file operations.

**Architecture:** Partial rewrite preserving parser/formatter/model core (~7,700 LOC). Rewrite TUI from scratch with multi-file architecture. Remove import/export, checker, backup, format, sort. Add config-based file lists, $EDITOR integration, fuzzy filter, cross-file cut/paste/move.

**Tech Stack:** Rust, ratatui 0.26, crossterm 0.27, clap 4, fuzzy-matcher, glob (crate), tempfile, dialoguer

**Spec:** `docs/superpowers/specs/2026-03-23-multi-file-refactoring-design.md`

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src/model/profile.rs` | `ShellProfile`, `ProfileFile`, `ListItem` structs |
| `src/config/templates.rs` | Built-in shell file list templates (bash/zsh/powershell) |
| `src/config/path_resolver.rs` | `~` expansion, `$VAR` expansion, glob matching |
| `src/tui/state.rs` | `AppMode`, `UndoSnapshot`, `ClipboardState` |
| `src/tui/list.rs` | Flat list building from `ShellProfile`, navigation helpers |
| `src/tui/selection.rs` | Single, multi, range selection logic |
| `src/tui/editor.rs` | `$EDITOR` integration (suspend TUI, launch editor, resume) |
| `src/tui/operations.rs` | Cut/paste, move, delete, add, save, undo |
| `src/tui/search.rs` | Fuzzy filter search state and matching |
| `src/tui/keys.rs` | Key event dispatch (maps keys → operations per mode) |
| `tests/config_tests.rs` | Config loading, templates, path resolution tests |
| `tests/profile_tests.rs` | ShellProfile/ProfileFile/ListItem tests |
| `tests/tui_logic_tests.rs` | TUI operations logic tests (no rendering) |

### Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Remove `ureq`/`url`, add `fuzzy-matcher`/`glob`/`tempfile`, promote `tempfile` to main deps |
| `src/lib.rs` | Remove `backup`, `cache`, `checker` modules; update exports |
| `src/main.rs` | Rewrite flow: config check → shell detect → build ShellProfile → TUI or source |
| `src/model/mod.rs` | Add `pub mod profile;` export |
| `src/model/config.rs` | Replace `FormatConfig`/`BackupConfig`/`CacheConfig` with `FilesConfig`; unify macOS path to `~/.config/wenv/` |
| `src/model/entry.rs` | Add `file_index: usize` field to `Entry` |
| `src/model/shell.rs` | Remove `default_config_path()` PowerShell cache logic; simplify |
| `src/config/mod.rs` | Add first-run flow, template generation, file list loading |
| `src/cli/mod.rs` | Remove `ConflictStrategy`, `EntryTypeArg` exports |
| `src/cli/args.rs` | Remove all deleted flags; keep `-s`/`--shell`, `--source`, `-c`/`--config`, positional `.` |
| `src/cli/context.rs` | Replace single-file `Context` with multi-file `ShellProfile` builder |
| `src/cli/actions/source.rs` | Rewrite to show file selection menu via `dialoguer::Select` |
| `src/utils/mod.rs` | Remove `http`, `path_merge`, `dependency` module declarations |
| `src/utils/shell_detect.rs` | Simplify; remove file-based detection (no longer used) |
| `src/tui/mod.rs` | Declare new submodules |
| `src/tui/app.rs` | Complete rewrite: multi-file TuiApp, main event loop |
| `src/tui/ui.rs` | Complete rewrite: multi-file rendering |
| `src/i18n/mod.rs` | Update/remove messages for deleted features; add new messages |
| `src/formatter/mod.rs` | Remove `find_attached_comments()` (format-only) |
| `src/formatter/bash.rs` | Keep `format_entry()`; remove `format()` group-by-type logic |
| `src/formatter/pwsh.rs` | Same: keep `format_entry()`; remove `format()` |

### Deleted Files

| File | Reason |
|------|--------|
| `src/checker/mod.rs` | Feature removed |
| `src/checker/duplicate.rs` | Feature removed |
| `src/backup/mod.rs` | Feature removed |
| `src/cli/actions/import.rs` | Feature removed |
| `src/cli/actions/export.rs` | Feature removed |
| `src/utils/http.rs` | Only used by import |
| `src/utils/path_merge.rs` | Only used by format |
| `src/utils/dependency.rs` | Only used by format |

---

## Task 1: Remove Deleted Modules and Dependencies

**Goal:** Strip out checker, backup, cache, import/export, and unused utils. Project must compile and existing parser tests must pass.

**Files:**
- Delete: `src/checker/mod.rs`, `src/checker/duplicate.rs`, `src/backup/mod.rs`, `src/cli/actions/import.rs`, `src/cli/actions/export.rs`, `src/utils/http.rs`, `src/utils/path_merge.rs`, `src/utils/dependency.rs`
- Modify: `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/cli/mod.rs`, `src/cli/actions/mod.rs`, `src/utils/mod.rs`, `src/tui/app.rs` (remove checker/backup imports and usage)

- [ ] **Step 1: Remove `ureq` and `url` from Cargo.toml**

In `Cargo.toml`, delete the `ureq` and `url` dependency lines. Add `tempfile = "3"` to `[dependencies]` (promote from dev-only). Add `glob = "0.3"` and `fuzzy-matcher = "0.3"` to `[dependencies]`.

- [ ] **Step 2: Delete feature module files**

```bash
rm src/checker/mod.rs src/checker/duplicate.rs
rmdir src/checker
rm src/backup/mod.rs
rmdir src/backup
rm src/cli/actions/import.rs src/cli/actions/export.rs
rm src/utils/http.rs src/utils/path_merge.rs src/utils/dependency.rs
```

- [ ] **Step 3: Update `src/lib.rs`**

Remove module declarations: `pub mod backup;`, `pub mod cache;`, `pub mod checker;`. Remove from exports: `pub use checker::check_all`, and any `Formatter` trait re-export that references removed types. Keep: `pub use model::{Config, Entry, EntryType, ParseResult, ShellType}`, `pub use parser::{get_parser, Parser}`, `pub use formatter::{get_formatter, Formatter}`.

- [ ] **Step 4: Update `src/utils/mod.rs`**

Remove: `pub mod http;`, `pub mod path_merge;`, `pub mod dependency;`. Keep: `pub mod path;`, `pub mod shell_detect;`, `pub mod strings;`.

- [ ] **Step 5: Update `src/cli/actions/mod.rs`**

Remove: `pub mod import;`, `pub mod export;`. Keep: `pub mod source;`.

- [ ] **Step 6: Update `src/cli/mod.rs`**

Remove re-exports of `ConflictStrategy`, `EntryTypeArg` if present.

- [ ] **Step 7: Update `src/main.rs`**

Remove all references to `--clear-cache`, `--import`, `--export` handling. Remove `use wenv::cache::PathCache;`. Remove the early-exit block for `clear_cache`. Remove the import/export action branches. Keep: `--config` handling, `--source`/`.` handling, TUI launch.

- [ ] **Step 8: Stub out removed references in `src/tui/app.rs`**

The current TUI imports `check_all`, `BackupManager`, and uses format/validation features. For now, comment out or remove these imports and any code blocks that reference them. The TUI will be rewritten later, so this is temporary — just make it compile. If large sections depend on removed features, stub them with `todo!()` or simply remove the code paths.

- [ ] **Step 9: Remove unused i18n messages**

In `src/i18n/mod.rs`, remove or comment out message fields related to import, export, checker, backup, and format features. This is cleanup — the struct must still compile.

- [ ] **Step 10: Build and test**

```bash
cargo build 2>&1 | head -50
cargo test --lib 2>&1 | tail -20
cargo test bash_tests 2>&1 | tail -10
cargo test pwsh 2>&1 | tail -10
```

Expected: compile succeeds, parser tests pass. TUI may have reduced functionality but compiles.

- [ ] **Step 11: Commit**

```bash
git add -A && git commit -m "refactor: remove checker, backup, cache, import/export, unused utils

Remove features per multi-file refactoring spec:
- checker/ module (buggy, platform-dependent)
- backup/ module (replaced by external tools)
- import/export actions and related CLI flags
- utils: http.rs, path_merge.rs, dependency.rs
- ureq/url dependencies

Add new dependencies: fuzzy-matcher, glob, tempfile (promoted)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 2: Simplify CLI Args

**Goal:** Remove all deleted flags from clap definitions. Keep only `-s`/`--shell`, `--source`, `-c`/`--config`, and positional `.`.

**Files:**
- Modify: `src/cli/args.rs`, `src/cli/context.rs`, `src/cli/mod.rs`

- [ ] **Step 1: Rewrite `src/cli/args.rs`**

Replace the entire `Cli` struct with:

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "wenv", about = "Shell configuration file manager")]
pub struct Cli {
    /// Specify shell type
    #[arg(short, long)]
    pub shell: Option<ShellArg>,

    /// Open config file in $EDITOR
    #[arg(long)]
    pub source: bool,

    /// Open wenv config in $EDITOR
    #[arg(short, long)]
    pub config: bool,

    /// "." to open editor, or ignored
    pub command: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ShellArg {
    Bash,
    Zsh,
    Pwsh,
}
```

Remove: `ConflictStrategy`, `EntryTypeArg` enums entirely.

- [ ] **Step 2: Simplify `src/cli/context.rs`**

Remove `on_conflict` field from `Context`. Remove `config_file` field (will be replaced by ShellProfile in Task 5). For now, keep a minimal `Context` that holds `config`, `shell_type`, and `messages`. Remove `parse_config_file()` method (will be in ShellProfile).

- [ ] **Step 3: Update `src/cli/mod.rs`**

Simplify exports to only `Cli`, `ShellArg`, `Context`.

- [ ] **Step 4: Update `src/main.rs`**

Update to use the simplified `Cli` struct. Remove references to deleted fields (`cli.import`, `cli.export`, `cli.yes`, `cli.on_conflict`, `cli.file`, `cli.r#type`, `cli.clear_cache`).

- [ ] **Step 5: Build and test**

```bash
cargo build 2>&1 | head -50
cargo run -- --help
```

Expected: `--help` shows only `-s`, `--source`, `-c`, `--config`. No deleted flags.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "refactor: simplify CLI to shell/source/config flags only

Remove -f/--file, -i/--import, -e/--export, --on-conflict,
-y/--yes, --clear-cache, -t/--type flags per spec.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 3: Restructure Config for File Lists

**Goal:** Replace old config sections (format, backup, cache) with `[files.<shell>]` sections. Unify macOS config path to `~/.config/wenv/`.

**Files:**
- Modify: `src/model/config.rs`
- Create: `src/config/templates.rs`, `src/config/path_resolver.rs`
- Modify: `src/config/mod.rs`
- Test: `tests/config_tests.rs`

- [ ] **Step 1: Write config test file**

Create `tests/config_tests.rs`:

```rust
use std::fs;
use tempfile::TempDir;

#[test]
fn test_default_config_has_no_file_lists() {
    // Default config should have empty files map
    let config = wenv::Config::default();
    assert!(config.files.is_empty());
}

#[test]
fn test_config_with_bash_files() {
    let toml_str = r#"
[ui]
language = "en"

[files.bash]
paths = ["~/.bashrc", "~/.profile"]
"#;
    let config: wenv::Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.files.get("bash").unwrap().paths.len(), 2);
}

#[test]
fn test_config_roundtrip() {
    let mut config = wenv::Config::default();
    config.files.insert("bash".to_string(), wenv::model::config::FilesConfig {
        paths: vec!["~/.bashrc".to_string(), "/etc/profile".to_string()],
    });
    let serialized = toml::to_string_pretty(&config).unwrap();
    let deserialized: wenv::Config = toml::from_str(&serialized).unwrap();
    assert_eq!(deserialized.files.get("bash").unwrap().paths.len(), 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test config_tests 2>&1 | tail -20
```

Expected: FAIL — `Config` doesn't have `files` field yet.

- [ ] **Step 3: Rewrite `src/model/config.rs`**

Replace the Config struct. Remove `FormatConfig`, `BackupConfig`, `CacheConfig`, `TypeOrder`. Add `FilesConfig`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub files: HashMap<String, FilesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_language")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesConfig {
    pub paths: Vec<String>,
}

fn default_language() -> String { "en".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig { language: default_language() },
            files: HashMap::new(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { language: default_language() }
    }
}
```

Keep `Config::config_dir()`, `Config::config_path()`, `Config::load()`, `Config::save()` methods. **Update `config_dir()`** to always return `~/.config/wenv/` on all platforms (remove the macOS `Library/Application Support` path). Remove the `.path_cache.toml` migration logic from `Config::load()`. Remove `Config::backups_dir()`.

- [ ] **Step 4a: Add `config_key()` to ShellType**

In `src/model/shell.rs`, add a method that returns the config section key (user-facing, readable):

```rust
impl ShellType {
    /// Key used in config.toml [files.<key>] sections.
    /// Distinct from name() which returns CLI-style short names.
    pub fn config_key(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::PowerShell => "powershell",
        }
    }
}
```

**Important:** All config/template lookups must use `config_key()` (returns `"powershell"`), not `name()` (returns `"pwsh"`). This keeps config files readable for users while maintaining the short CLI name.

- [ ] **Step 4: Update `src/model/mod.rs`**

Update exports: remove `BackupConfig`, `CacheConfig`, `FormatConfig`, `TypeOrder`. Add `FilesConfig`.

- [ ] **Step 5: Fix compilation errors**

Other files may reference removed config types. Update `src/formatter/bash.rs` and `src/formatter/pwsh.rs`: their `format()` method takes `config: &Config` — if it references `FormatConfig` fields, update or simplify. Since the format *command* is removed, the `format()` trait method can be simplified or have its body return a basic concatenation of `format_entry()` calls. Keep `format_entry()` intact.

Update `src/tui/app.rs` references to config fields if any.

- [ ] **Step 6: Run tests**

```bash
cargo test --test config_tests 2>&1 | tail -20
cargo test --lib 2>&1 | tail -20
```

Expected: config_tests pass, lib tests pass.

- [ ] **Step 7: Create `src/config/templates.rs`**

Built-in templates for each shell:

```rust
pub fn default_paths(shell_key: &str) -> Option<Vec<String>> {
    // shell_key is from ShellType::config_key(): "bash", "zsh", "powershell"
    match shell_key {
        "bash" => Some(vec![
            "/etc/profile".into(),
            "/etc/profile.d/*.sh".into(),
            "~/.profile".into(),
            "~/.bashrc".into(),
            "~/.bash_aliases".into(),
        ]),
        "zsh" => Some(vec![
            "/etc/zshenv".into(),
            "/etc/zprofile".into(),
            "/etc/zshrc".into(),
            "~/.zshenv".into(),
            "~/.zprofile".into(),
            "~/.zshrc".into(),
            "~/.zsh_aliases".into(),
        ]),
        "powershell" => Some(vec![
            "$PROFILE".into(),
        ]),
        _ => None,
    }
}

pub fn generate_default_config(shell_key: &str) -> String {
    let mut config = crate::model::config::Config::default();
    if let Some(paths) = default_paths(shell_key) {
        config.files.insert(shell_key.to_string(),
            crate::model::config::FilesConfig { paths });
    }
    toml::to_string_pretty(&config).unwrap()
}
```

- [ ] **Step 8: Create `src/config/path_resolver.rs`**

```rust
use std::path::PathBuf;

/// Expand ~ to home directory
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

/// Expand $VAR references in path
pub fn expand_env_vars(path: &str) -> String {
    let mut result = path.to_string();
    // Match $WORD patterns (not inside single quotes)
    let re = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in re.captures_iter(path) {
        let var_name = &cap[1];
        if let Ok(val) = std::env::var(var_name) {
            result = result.replace(&cap[0], &val);
        }
    }
    result
}

/// Resolve a config path pattern to concrete file paths.
/// Handles ~, $VAR, and glob patterns.
/// Returns (resolved_path, exists) pairs.
pub fn resolve_paths(patterns: &[String]) -> Vec<(PathBuf, bool)> {
    let mut results = Vec::new();
    for pattern in patterns {
        let expanded = expand_env_vars(&expand_tilde(pattern));
        if expanded.contains('*') || expanded.contains('?') {
            // Glob expansion
            if let Ok(paths) = glob::glob(&expanded) {
                for entry in paths.flatten() {
                    let exists = entry.exists();
                    results.push((entry, exists));
                }
            }
        } else {
            let path = PathBuf::from(&expanded);
            let exists = path.exists();
            results.push((path, exists));
        }
    }
    results
}
```

- [ ] **Step 9: Write path resolver tests**

Add to `tests/config_tests.rs`:

```rust
use wenv::config::path_resolver;

#[test]
fn test_expand_tilde() {
    let expanded = path_resolver::expand_tilde("~/test");
    assert!(!expanded.starts_with("~"));
    assert!(expanded.ends_with("/test"));
}

#[test]
fn test_expand_env_vars() {
    std::env::set_var("WENV_TEST_VAR", "/tmp/test");
    let expanded = path_resolver::expand_env_vars("$WENV_TEST_VAR/config");
    assert_eq!(expanded, "/tmp/test/config");
    std::env::remove_var("WENV_TEST_VAR");
}

#[test]
fn test_resolve_nonexistent_path() {
    let results = path_resolver::resolve_paths(&[
        "/nonexistent/path/file.txt".to_string()
    ]);
    assert_eq!(results.len(), 1);
    assert!(!results[0].1); // exists == false
}
```

- [ ] **Step 10: Update `src/config/mod.rs`**

Add module declarations and update public API:

```rust
pub mod path_resolver;
pub mod templates;

// Keep existing: ensure_config_dir, load_or_create_config, save_config
// Add: first-run flow function
pub fn first_run_setup(shell_name: &str) -> anyhow::Result<Config> { ... }
pub fn ensure_shell_files(config: &mut Config, shell_name: &str) -> anyhow::Result<bool> { ... }
```

`first_run_setup()`: prompt user with dialoguer, generate config from template, save, return.
`ensure_shell_files()`: check if `config.files` contains the shell; if not, prompt and add from template. Returns `true` if added.

- [ ] **Step 11: Build and test**

```bash
cargo test --test config_tests 2>&1 | tail -20
cargo build 2>&1 | head -50
```

Expected: all config tests pass, project compiles.

- [ ] **Step 12: Commit**

```bash
git add -A && git commit -m "feat: restructure config for multi-file support

- Replace FormatConfig/BackupConfig/CacheConfig with files HashMap
- Add FilesConfig with paths per shell type
- Add built-in templates for bash/zsh/powershell
- Add path resolver (tilde, env vars, glob expansion)
- Unify macOS config path to ~/.config/wenv/
- Add first-run setup flow

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 4: Create Multi-File Data Model

**Goal:** Add `ShellProfile`, `ProfileFile`, `ListItem` types and the logic to build a profile from config.

**Files:**
- Create: `src/model/profile.rs`
- Modify: `src/model/mod.rs`, `src/model/entry.rs`
- Test: `tests/profile_tests.rs`

- [ ] **Step 1: Write profile tests**

Create `tests/profile_tests.rs`:

```rust
use wenv::model::profile::{ShellProfile, ProfileFile, ListItem};
use wenv::model::{Entry, EntryType, ShellType};
use std::path::PathBuf;

#[test]
fn test_build_visible_list_collapsed() {
    let profile = ShellProfile {
        shell_type: ShellType::Bash,
        files: vec![
            ProfileFile::new_with_entries(
                PathBuf::from("/etc/profile"),
                vec![Entry::new(EntryType::Alias, "ll".into(), "alias ll='ls -la'".into())],
                false, // collapsed
            ),
        ],
    };
    let list = profile.build_visible_list();
    assert_eq!(list.len(), 1); // only file header, entries hidden
    assert!(matches!(list[0], ListItem::FileHeader(0)));
}

#[test]
fn test_build_visible_list_expanded() {
    let profile = ShellProfile {
        shell_type: ShellType::Bash,
        files: vec![
            ProfileFile::new_with_entries(
                PathBuf::from("~/.bashrc"),
                vec![
                    Entry::new(EntryType::Alias, "ll".into(), "alias ll='ls -la'".into()),
                    Entry::new(EntryType::Function, "greet".into(), "greet() { echo hi; }".into()),
                ],
                true, // expanded
            ),
        ],
    };
    let list = profile.build_visible_list();
    assert_eq!(list.len(), 3); // header + 2 entries
    assert!(matches!(list[0], ListItem::FileHeader(0)));
    assert!(matches!(list[1], ListItem::Entry(0, 0)));
    assert!(matches!(list[2], ListItem::Entry(0, 1)));
}

#[test]
fn test_build_visible_list_multiple_files() {
    let profile = ShellProfile {
        shell_type: ShellType::Bash,
        files: vec![
            ProfileFile::new_with_entries(
                PathBuf::from("/etc/profile"),
                vec![Entry::new(EntryType::EnvVar, "PATH".into(), "export PATH=/usr/bin".into())],
                true,
            ),
            ProfileFile::new_with_entries(
                PathBuf::from("~/.bashrc"),
                vec![Entry::new(EntryType::Alias, "ll".into(), "alias ll='ls -la'".into())],
                false, // collapsed
            ),
        ],
    };
    let list = profile.build_visible_list();
    // File 0 header + 1 entry + File 1 header (collapsed, no entries)
    assert_eq!(list.len(), 3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test profile_tests 2>&1 | tail -20
```

Expected: FAIL — module doesn't exist yet.

- [ ] **Step 3: Add `file_index` to Entry**

In `src/model/entry.rs`, add field to `Entry` struct:

```rust
pub struct Entry {
    pub entry_type: EntryType,
    pub name: String,
    pub value: String,
    pub line_number: Option<usize>,
    pub end_line: Option<usize>,
    pub file_index: usize,  // NEW: index into ShellProfile.files
}
```

Update `Entry::new()` to set `file_index: 0` by default. Add `Entry::with_file_index(mut self, idx: usize) -> Self` builder method.

Fix all existing code that constructs `Entry` (parsers, tests) to include the new field.

- [ ] **Step 4: Create `src/model/profile.rs`**

```rust
use std::path::PathBuf;
use crate::model::{Entry, ShellType};

#[derive(Debug, Clone, PartialEq)]
pub enum ListItem {
    FileHeader(usize),
    Entry(usize, usize),
}

pub struct ProfileFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub content: String,
    pub expanded: bool,
    pub dirty: bool,
    pub exists: bool,
}

pub struct ShellProfile {
    pub shell_type: ShellType,
    pub files: Vec<ProfileFile>,
}

impl ProfileFile {
    pub fn new(path: PathBuf, exists: bool) -> Self { ... }
    pub fn new_with_entries(path: PathBuf, entries: Vec<Entry>, expanded: bool) -> Self { ... }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn display_name(&self) -> String {
        // Show ~ for home dir paths
        ...
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
```

- [ ] **Step 5: Update `src/model/mod.rs`**

Add `pub mod profile;` and export key types.

- [ ] **Step 6: Build and test**

```bash
cargo test --test profile_tests 2>&1 | tail -20
cargo test --lib 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 7: Add ShellProfile loading**

Add a function (in `src/model/profile.rs` or `src/config/mod.rs`) that builds a `ShellProfile` from a `Config` and `ShellType`:

```rust
pub fn load_shell_profile(
    config: &Config,
    shell_type: ShellType,
) -> anyhow::Result<ShellProfile> {
    let shell_key = shell_type.config_key();
    let file_configs = config.files.get(shell_key)
        .ok_or_else(|| anyhow::anyhow!("No file list for {}", shell_key))?;

    let resolved = path_resolver::resolve_paths(&file_configs.paths);
    let parser = get_parser(shell_type);

    let mut files = Vec::new();
    for (path, exists) in resolved {
        let mut pf = ProfileFile::new(path.clone(), exists);
        if exists {
            let content = std::fs::read_to_string(&path)?;
            let result = parser.parse(&content);
            // Set file_index on each entry
            for (i, mut entry) in result.entries.into_iter().enumerate() {
                entry.file_index = files.len();
                pf.entries.push(entry);
            }
            pf.content = content;
        }
        pf.expanded = exists; // auto-expand files that exist
        files.push(pf);
    }

    Ok(ShellProfile { shell_type, files })
}
```

- [ ] **Step 8: Build and test**

```bash
cargo build 2>&1 | head -50
cargo test 2>&1 | tail -20
```

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: add multi-file data model (ShellProfile, ProfileFile, ListItem)

- ShellProfile holds Vec<ProfileFile> per shell session
- ProfileFile wraps entries with path, dirty flag, expand state
- ListItem enum for flat-list TUI navigation
- Entry gains file_index field
- load_shell_profile() builds from config + parser

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 5: Refactor Main Entry Point and Context

**Goal:** Wire the new config/profile system into main.rs. First-run flow works.

**Files:**
- Modify: `src/main.rs`, `src/cli/context.rs`, `src/cli/actions/source.rs`

- [ ] **Step 1: Rewrite `src/main.rs`**

New flow:

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Early exit: open wenv config
    if cli.config {
        // open $EDITOR on Config::config_path()
        return Ok(());
    }

    // Determine shell type (runtime, no config dependency)
    let shell_type = determine_shell_type(cli.shell);

    // Load or create config (first-run flow)
    let mut config = config::load_or_create_config()?;
    let shell_key = shell_type.config_key();

    // Ensure file list exists for this shell
    if !config.files.contains_key(shell_key) {
        config::ensure_shell_files(&mut config, shell_key)?;
    }

    let messages = i18n::init_messages(&config.ui.language);

    // Source mode: file selection menu
    let is_source = cli.source || cli.command.as_deref() == Some(".");
    if is_source {
        return actions::source::execute(&config, shell_type, messages);
    }

    // Default: load profile and launch TUI
    let profile = model::profile::load_shell_profile(&config, shell_type)?;
    tui::TuiApp::new(profile, messages)?.run()
}
```

- [ ] **Step 2: Simplify `src/cli/context.rs`**

The `Context` struct can be slimmed down or removed entirely since `ShellProfile` + `Messages` carry all needed state. If any utility methods remain useful (like `print_success`, `print_warning`), move them to a utility module. Otherwise remove `context.rs`.

- [ ] **Step 3: Rewrite `src/cli/actions/source.rs`**

New behavior: show file selection menu using `dialoguer::Select`:

```rust
use crate::config::path_resolver;
use crate::model::{Config, ShellType};
use dialoguer::Select;

pub fn execute(config: &Config, shell_type: ShellType, messages: &'static Messages) -> anyhow::Result<()> {
    let shell_key = shell_type.config_key();
    let file_configs = config.files.get(shell_key)
        .ok_or_else(|| anyhow::anyhow!("No file list for {}", shell_key))?;

    let resolved = path_resolver::resolve_paths(&file_configs.paths);
    if resolved.is_empty() {
        println!("No files configured for {}", shell_key);
        return Ok(());
    }

    let items: Vec<String> = resolved.iter().map(|(path, exists)| {
        let display = path.display();
        if *exists { format!("{} ✓", display) }
        else { format!("{} (not found)", display) }
    }).collect();

    let selection = Select::new()
        .with_prompt(format!("Shell configuration files ({})", shell_key))
        .items(&items)
        .default(0)
        .interact()?;

    let (path, exists) = &resolved[selection];
    if !exists {
        println!("File does not exist: {}", path.display());
        return Ok(());
    }

    // Open in $EDITOR
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) { "notepad".into() } else { "vi".into() }
    });
    std::process::Command::new(&editor).arg(path).status()?;
    Ok(())
}
```

- [ ] **Step 4: Build and smoke test**

```bash
cargo build 2>&1 | head -50
cargo run -- --help
cargo run -- -c  # should open config in editor (Ctrl+C to cancel)
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "refactor: wire new config/profile system into main entry point

- First-run flow prompts to create config from template
- Shell detection at runtime (CLI flag, $SHELL, platform)
- wenv . shows file selection menu via dialoguer
- Remove old Context struct

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 6: TUI Scaffold — New Multi-File App Shell

**Goal:** Replace old TUI with a minimal working shell: renders file headers and entries, navigates up/down, quits with `q`. No operations yet.

**Files:**
- Rewrite: `src/tui/app.rs`, `src/tui/ui.rs`, `src/tui/mod.rs`
- Create: `src/tui/state.rs`, `src/tui/list.rs`, `src/tui/keys.rs`

- [ ] **Step 1: Back up old TUI files, then clear them**

```bash
# Old files will be completely rewritten
> src/tui/app.rs
> src/tui/ui.rs
```

- [ ] **Step 2: Create `src/tui/state.rs`**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Searching,
    ShowingDetail,
    ShowingHelp,
    ConfirmDelete,
    ConfirmQuit,
    Moving,
}

pub struct ClipboardState {
    pub entries: Vec<crate::model::Entry>,
}

pub struct UndoSnapshot {
    pub file_states: Vec<(std::path::PathBuf, String, Vec<crate::model::Entry>)>,
}
```

- [ ] **Step 3: Create `src/tui/list.rs`**

Re-export `ListItem` from model::profile, plus navigation helpers:

```rust
use crate::model::profile::{ListItem, ShellProfile};

pub fn navigate_up(items: &[ListItem], current: usize) -> usize {
    if current > 0 { current - 1 } else { current }
}

pub fn navigate_down(items: &[ListItem], current: usize) -> usize {
    if current + 1 < items.len() { current + 1 } else { current }
}

pub fn navigate_home() -> usize { 0 }

pub fn navigate_end(items: &[ListItem]) -> usize {
    if items.is_empty() { 0 } else { items.len() - 1 }
}

/// Given a ListItem, determine which file_index it belongs to
pub fn file_index_of(item: &ListItem) -> usize {
    match item {
        ListItem::FileHeader(fi) => *fi,
        ListItem::Entry(fi, _) => *fi,
    }
}
```

- [ ] **Step 4: Create `src/tui/keys.rs`**

Key event dispatch stub — maps crossterm KeyEvents to actions:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::tui::state::AppMode;

pub enum Action {
    NavigateUp,
    NavigateDown,
    Home,
    End,
    ToggleExpand,
    CollapseAll,
    ExpandAll,
    Edit,
    Add,
    Delete,
    ToggleSelect,
    RangeSelectUp,
    RangeSelectDown,
    Cut,
    Paste,
    StartMove,
    Search,
    Undo,
    Help,
    Save,
    Quit,
    Confirm,
    Cancel,
    // Search-mode specific
    SearchInput(char),
    SearchBackspace,
    Noop,
}

pub fn map_key(mode: &AppMode, key: KeyEvent) -> Action {
    match mode {
        AppMode::Normal => map_normal_key(key),
        AppMode::Moving => map_moving_key(key),
        AppMode::Searching => map_search_key(key),
        _ => map_popup_key(key),
    }
}

fn map_normal_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::SHIFT) => Action::RangeSelectUp,
        KeyCode::Up | KeyCode::Char('k') => Action::NavigateUp,
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::SHIFT) => Action::RangeSelectDown,
        KeyCode::Down | KeyCode::Char('j') => Action::NavigateDown,
        KeyCode::Home => Action::Home,
        KeyCode::End => Action::End,
        KeyCode::Enter | KeyCode::Char(' ') => Action::ToggleExpand,
        KeyCode::Char('0') => Action::CollapseAll,
        KeyCode::Char('9') => Action::ExpandAll,
        KeyCode::Char('e') => Action::Edit,
        KeyCode::Char('a') => Action::Add,
        KeyCode::Char('d') => Action::Delete,
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Save,
        KeyCode::Char('s') => Action::ToggleSelect,
        KeyCode::Char('x') => Action::Cut,
        KeyCode::Char('p') => Action::Paste,
        KeyCode::Char('m') => Action::StartMove,
        KeyCode::Char('/') => Action::Search,
        KeyCode::Char('u') => Action::Undo,
        KeyCode::Char('?') => Action::Help,
        KeyCode::Char('w') => Action::Save,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Esc => Action::Cancel,
        _ => Action::Noop,
    }
}
// ... implement map_moving_key, map_search_key, map_popup_key similarly
```

- [ ] **Step 5: Write `src/tui/app.rs` — minimal TuiApp**

Core state struct and main event loop. Start minimal: navigation + toggle + quit.

```rust
use crate::i18n::Messages;
use crate::model::profile::{ShellProfile, ListItem};
use crate::tui::state::AppMode;
use crate::tui::keys::{self, Action};
use crossterm::event::{self, Event};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub struct TuiApp {
    pub profile: ShellProfile,
    pub visible_items: Vec<ListItem>,
    pub cursor: usize,
    pub mode: AppMode,
    pub should_quit: bool,
    pub message: Option<String>,
    pub messages: &'static Messages,
}

impl TuiApp {
    pub fn new(profile: ShellProfile, messages: &'static Messages) -> anyhow::Result<Self> {
        let visible_items = profile.build_visible_list();
        Ok(Self {
            profile,
            visible_items,
            cursor: 0,
            mode: AppMode::Normal,
            should_quit: false,
            message: None,
            messages,
        })
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        // Setup terminal
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Main loop
        while !self.should_quit {
            terminal.draw(|f| crate::tui::ui::draw(f, self))?;

            if let Event::Key(key) = event::read()? {
                let action = keys::map_key(&self.mode, key);
                self.handle_action(action)?;
            }
        }

        // Cleanup terminal
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::NavigateUp => { /* update cursor */ },
            Action::NavigateDown => { /* update cursor */ },
            Action::Home => { self.cursor = 0; },
            Action::End => { self.cursor = self.visible_items.len().saturating_sub(1); },
            Action::ToggleExpand => { self.toggle_current_file(); },
            Action::CollapseAll => { self.profile.toggle_all(false); self.rebuild_list(); },
            Action::ExpandAll => { self.profile.toggle_all(true); self.rebuild_list(); },
            Action::Quit => { self.should_quit = true; },
            _ => {} // Not yet implemented
        }
        Ok(())
    }

    fn toggle_current_file(&mut self) { ... }
    fn rebuild_list(&mut self) {
        self.visible_items = self.profile.build_visible_list();
        // Clamp cursor
        if self.cursor >= self.visible_items.len() {
            self.cursor = self.visible_items.len().saturating_sub(1);
        }
    }
}
```

- [ ] **Step 6: Write `src/tui/ui.rs` — minimal rendering**

Draw the multi-file list with file headers and entries:

```rust
use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::tui::app::TuiApp;
use crate::model::profile::ListItem;

pub fn draw(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),     // title
            Constraint::Min(1),        // main list
            Constraint::Length(2),     // status bar
        ])
        .split(f.area());

    draw_title(f, chunks[0]);
    draw_list(f, chunks[1], app);
    draw_status(f, chunks[2], app);
}

fn draw_list(f: &mut Frame, area: Rect, app: &TuiApp) {
    // Note: use `crate::model::profile::ListItem` (aliased as `ProfileListItem` if needed)
    // to avoid collision with `ratatui::widgets::ListItem`.
    let items: Vec<ratatui::widgets::ListItem<'_>> = app.visible_items.iter().enumerate().map(|(i, item)| {
        match item {
            crate::model::profile::ListItem::FileHeader(fi) => {
                let file = &app.profile.files[*fi];
                let icon = if file.expanded { "📜 ▼" } else { "📜 ▶" };
                let dirty = if file.dirty { " ●" } else { "" };
                let text = format!("{} {} [{} entries]{}", icon, file.display_name(), file.entry_count(), dirty);
                // style based on selection
                ...
            }
            crate::model::profile::ListItem::Entry(fi, ei) => {
                let entry = &app.profile.files[*fi].entries[*ei];
                let text = format!("   {} {:10} L{}", entry.name, entry.entry_type, entry.line_number.unwrap_or(0));
                ...
            }
        }
    }).collect();
    // Render with ratatui List widget, highlight current cursor position
    ...
}
```

- [ ] **Step 7: Update `src/tui/mod.rs`**

```rust
pub mod app;
pub mod keys;
pub mod list;
pub mod state;
pub mod ui;
pub use app::TuiApp;
```

- [ ] **Step 8: Build and test**

```bash
cargo build 2>&1 | head -50
```

Expected: compiles. Manual test: `cargo run` should launch TUI showing file list, navigate with arrows, toggle with Enter, quit with q.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: new multi-file TUI scaffold

Minimal working TUI with:
- Multi-file list display (📜 headers with expand/collapse)
- Arrow key navigation across files and entries
- Toggle expand (Enter/Space), collapse all (0), expand all (9)
- Quit with q
- New modular structure: app, state, list, keys, ui

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 7: TUI Selection System

**Goal:** Implement single select (cursor), multi-select (`s`), range select (`Shift+↑/↓`).

**Files:**
- Create: `src/tui/selection.rs`
- Modify: `src/tui/app.rs`, `src/tui/ui.rs`, `src/tui/keys.rs`

- [ ] **Step 1: Create `src/tui/selection.rs`**

```rust
use std::collections::HashSet;

pub struct SelectionState {
    pub multi_select_mode: bool,
    pub selected_indices: HashSet<usize>,
    pub anchor: Option<usize>,
}

impl SelectionState {
    pub fn new() -> Self { ... }
    pub fn clear(&mut self) { ... }
    pub fn toggle(&mut self, index: usize) { ... }
    pub fn set_range(&mut self, from: usize, to: usize) { ... }
    pub fn is_selected(&self, index: usize) -> bool { ... }
    pub fn selected_count(&self) -> usize { ... }
    pub fn sorted_indices(&self) -> Vec<usize> { ... }
}
```

- [ ] **Step 2: Wire into TuiApp**

Add `selection: SelectionState` field to `TuiApp`. Handle `ToggleSelect`, `RangeSelectUp`, `RangeSelectDown` actions. Update UI rendering to highlight selected items with distinct style.

- [ ] **Step 3: Build and test**

Manual test: launch TUI, press `s` to toggle select, Shift+arrows for range selection.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: TUI selection system (single, multi, range)

- s to toggle multi-select
- Shift+Up/Down for range selection
- Visual feedback with distinct highlight color

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 8: $EDITOR Integration

**Goal:** Edit entries and files via external editor.

**Files:**
- Create: `src/tui/editor.rs`
- Modify: `src/tui/app.rs`, `src/tui/keys.rs`

- [ ] **Step 1: Create `src/tui/editor.rs`**

```rust
use std::path::Path;
use std::io::Write;

/// Get the user's preferred editor
pub fn get_editor() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) { "notepad".into() } else { "vi".into() }
    })
}

/// Suspend TUI, launch editor on a file, resume TUI.
/// Returns Ok(true) if the file was modified.
pub fn edit_file(path: &Path) -> anyhow::Result<bool> {
    let before = std::fs::metadata(path).ok().map(|m| m.modified().ok()).flatten();

    let editor = get_editor();
    let status = std::process::Command::new(&editor).arg(path).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    let after = std::fs::metadata(path).ok().map(|m| m.modified().ok()).flatten();
    Ok(before != after)
}

/// Write content to a temp file, edit it, read back.
/// Returns the new content, or None if user didn't save.
pub fn edit_temp_content(content: &str, suffix: &str) -> anyhow::Result<Option<String>> {
    let mut tmp = tempfile::Builder::new()
        .prefix(".wenv_edit_")
        .suffix(suffix)
        .tempfile()?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;

    let path = tmp.path().to_path_buf();
    let modified = edit_file(&path)?;

    if modified {
        let new_content = std::fs::read_to_string(&path)?;
        Ok(Some(new_content))
    } else {
        Ok(None)
    }
}
```

- [ ] **Step 2: Add TUI suspend/resume**

In `src/tui/app.rs`, add methods to suspend and resume the terminal:

```rust
fn suspend_tui(terminal: &mut Terminal<...>) -> anyhow::Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    Ok(())
}

fn resume_tui(terminal: &mut Terminal<...>) -> anyhow::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    terminal.clear()?;
    Ok(())
}
```

- [ ] **Step 3: Wire Edit action**

In `handle_action()`, when `Action::Edit`:
- If cursor is on `FileHeader(fi)`: suspend TUI → call `edit_file(&profile.files[fi].path)` → re-parse file → resume
- If cursor is on `Entry(fi, ei)`: get `entry.value` → call `edit_temp_content(value, ".sh")` → parse result → update entry → mark dirty → resume

- [ ] **Step 4: Wire Add action**

When `Action::Add`:
- Determine target file from cursor position
- Call `edit_temp_content("", ".sh")` (or with template comment)
- Parse result into entries
- Insert after cursor position in target file
- Mark dirty

- [ ] **Step 5: Build and test**

```bash
cargo build 2>&1 | head -50
```

Manual test: launch TUI, press `e` on a file header (should open $EDITOR), press `e` on an entry (should edit in temp file), press `a` to add.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: $EDITOR integration for entry and file editing

- e on file header: open file in $EDITOR, re-parse on return
- e on entry: edit entry.value in temp file via $EDITOR
- a to add: create entry via $EDITOR at cursor position
- TUI suspend/resume around editor invocations

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 9: Delete, Cut/Paste, and Undo

**Goal:** Entry deletion with confirmation, cut/paste across files, single-step undo.

**Files:**
- Create: `src/tui/operations.rs`
- Modify: `src/tui/app.rs`, `src/tui/ui.rs`

- [ ] **Step 1: Create `src/tui/operations.rs`**

Logic for entry manipulation operations:

```rust
use crate::model::profile::{ShellProfile, ListItem};
use crate::model::Entry;
use crate::tui::state::UndoSnapshot;

/// Delete entries at given visible-list indices from the profile.
pub fn delete_entries(profile: &mut ShellProfile, items: &[ListItem], indices: &[usize]) {
    // Group by file, remove in reverse order to preserve indices
    ...
}

/// Cut entries: remove from profile, return as clipboard.
pub fn cut_entries(profile: &mut ShellProfile, items: &[ListItem], indices: &[usize]) -> Vec<Entry> {
    // Similar to delete but returns the removed entries
    ...
}

/// Paste entries after a given position.
pub fn paste_entries(profile: &mut ShellProfile, items: &[ListItem], at: usize, entries: Vec<Entry>) {
    // Determine target file_index and entry position from ListItem at 'at'
    // Insert entries, update file_index on each entry
    // Mark target file as dirty
    ...
}

/// Take an undo snapshot of all files.
pub fn take_snapshot(profile: &ShellProfile) -> UndoSnapshot { ... }

/// Restore from an undo snapshot.
pub fn restore_snapshot(profile: &mut ShellProfile, snapshot: UndoSnapshot) { ... }
```

- [ ] **Step 2: Wire Delete action**

When `Action::Delete`:
- Take undo snapshot
- Switch to `ConfirmDelete` mode
- UI shows confirmation popup listing entries to delete
- On confirm: call `delete_entries()`
- On cancel: return to Normal

- [ ] **Step 3: Wire Cut/Paste actions**

`Action::Cut`: take snapshot → `cut_entries()` → store in `ClipboardState`
`Action::Paste`: take snapshot → `paste_entries()` from clipboard

- [ ] **Step 4: Wire Undo action**

`Action::Undo`: restore from `undo_snapshot` if present. Clear snapshot after restore.

- [ ] **Step 5: Wire Save action**

`Action::Save`: for each dirty file, reconstruct content from entries (join `entry.value` with `\n`, add trailing newline), write to disk, clear dirty flag. Recalculate line numbers by re-parsing.

- [ ] **Step 6: Wire Quit action**

`Action::Quit`: if `profile.any_dirty()`, switch to `ConfirmQuit` mode showing dirty file list. On confirm: quit without saving. On cancel: return to Normal.

- [ ] **Step 7: Build and test**

```bash
cargo build 2>&1 | head -50
```

Manual test: delete entries, cut/paste between files, undo, save, quit with unsaved changes.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: delete, cut/paste, undo, save, quit operations

- d: delete with confirmation popup
- x: cut entries to clipboard
- p: paste entries (cross-file support)
- u: single-step undo
- w/Ctrl+s: save all dirty files
- q: quit with unsaved-changes confirmation

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 10: Drag-Style Move

**Goal:** Press `m` to enter move mode, navigate to target, press Enter to drop.

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/ui.rs`, `src/tui/keys.rs`, `src/tui/operations.rs`

- [ ] **Step 1: Add move state to TuiApp**

```rust
pub struct MoveState {
    pub source_items: Vec<(usize, usize)>,  // (file_index, entry_index) of moved entries
    pub insertion_cursor: usize,             // visible-list index for drop target
}
```

- [ ] **Step 2: Handle `Action::StartMove`**

Take undo snapshot. Capture selected entries. Switch to `Moving` mode. Initialize `insertion_cursor` at current position.

- [ ] **Step 3: Handle Moving mode keys**

In `map_moving_key()`:
- `↑`/`↓`: move `insertion_cursor`
- `Enter` (`Action::Confirm`): execute move — remove entries from source, insert at target position, mark files dirty, return to Normal
- `Esc` (`Action::Cancel`): restore from snapshot, return to Normal

- [ ] **Step 4: Render move indicator**

In `ui.rs`, when mode is `Moving`, draw a horizontal line or highlight bar at the `insertion_cursor` position to show where entries will be dropped.

- [ ] **Step 5: Build and test**

Manual test: select entries, press `m`, navigate across files, press Enter to drop.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: drag-style move across files

- m: enter move mode with selected entries
- Up/Down: move insertion indicator across files
- Enter: confirm drop at target position
- Esc: cancel and restore original positions

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 11: Fuzzy Filter Search

**Goal:** Press `/` for fzf-style fuzzy search filtering entries in real-time.

**Files:**
- Create: `src/tui/search.rs`
- Modify: `src/tui/app.rs`, `src/tui/ui.rs`, `src/tui/keys.rs`

- [ ] **Step 1: Create `src/tui/search.rs`**

```rust
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use crate::model::profile::ShellProfile;

pub struct SearchState {
    pub query: String,
    pub cursor: usize,    // cursor position in query string
    pub matcher: SkimMatcherV2,
    pub matches: Vec<SearchMatch>,
}

pub struct SearchMatch {
    pub file_index: usize,
    pub entry_index: usize,
    pub score: i64,
    pub matched_indices: Vec<usize>,  // char positions that matched
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            matcher: SkimMatcherV2::default(),
            matches: Vec::new(),
        }
    }

    pub fn update_matches(&mut self, profile: &ShellProfile) {
        self.matches.clear();
        if self.query.is_empty() { return; }

        for (fi, file) in profile.files.iter().enumerate() {
            for (ei, entry) in file.entries.iter().enumerate() {
                let haystack = format!("{} {}", entry.name, entry.value);
                if let Some((score, indices)) = self.matcher.fuzzy_indices(&haystack, &self.query) {
                    self.matches.push(SearchMatch {
                        file_index: fi,
                        entry_index: ei,
                        score,
                        matched_indices: indices,
                    });
                }
            }
        }
        self.matches.sort_by(|a, b| b.score.cmp(&a.score));
    }

    pub fn input_char(&mut self, c: char) {
        self.query.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.query[..self.cursor].chars().last().map(|c| c.len_utf8()).unwrap_or(0);
            self.query.drain(self.cursor - prev..self.cursor);
            self.cursor -= prev;
        }
    }
}
```

- [ ] **Step 2: Wire search mode into TuiApp**

Add `search: Option<SearchState>` to TuiApp.

`Action::Search`: create `SearchState`, switch to `Searching` mode.
`Action::SearchInput(c)`: update query, recalculate matches.
`Action::SearchBackspace`: remove char, recalculate.
`Action::Confirm` (in Searching): exit search, navigate to selected match.
`Action::Cancel` (in Searching): clear search, restore full list.

- [ ] **Step 3: Filtered visible list**

When searching, `build_visible_list()` should only include entries that match. File headers are included if they have any matching entries.

- [ ] **Step 4: Search UI rendering**

In `ui.rs`:
- Show search bar at bottom (or top) with query text
- Highlight matched characters in entry names/values
- Show match count

- [ ] **Step 5: Build and test**

```bash
cargo build 2>&1 | head -50
```

Manual test: press `/`, type query, see filtered results, Enter to select, Esc to clear.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: fuzzy filter search with real-time matching

- / to enter search mode
- Real-time fuzzy filtering using fuzzy-matcher (skim algorithm)
- Matched characters highlighted in results
- Enter to jump to selected match
- Esc to clear filter and restore full list

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 12: Detail Popup and Help

**Goal:** Entry detail view and help popup.

**Files:**
- Modify: `src/tui/app.rs`, `src/tui/ui.rs`

- [ ] **Step 1: ShowingDetail mode**

Refine Enter/Space behavior from Task 6: dispatch based on cursor target.
- On `FileHeader`: toggle expand/collapse (existing behavior)
- On `Entry`: switch to `ShowingDetail` mode
- Show popup with: file path, entry type, name, full value (scrollable), line numbers

Update `handle_action(Action::ToggleExpand)` to check `visible_items[cursor]` — if `FileHeader`, toggle expand; if `Entry`, show detail.

- [ ] **Step 2: Help popup**

When user presses `?`:
- Switch to `ShowingHelp`
- Show popup listing all key bindings from the spec

- [ ] **Step 3: Build and test**

Manual test: view entry details, view help.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: entry detail popup and help screen

- Enter on entry: show detail popup with full content
- ? to show help with all key bindings

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 13: Update i18n and Cleanup

**Goal:** Update i18n messages for new features, remove dead messages, clean up formatter.

**Files:**
- Modify: `src/i18n/mod.rs`, `src/formatter/mod.rs`, `src/formatter/bash.rs`, `src/formatter/pwsh.rs`

- [ ] **Step 1: Clean up i18n**

Remove messages for: import, export, checker, backup, format preview, sort, edit mode fields. Add messages for: multi-file operations, search, move mode, file headers.

- [ ] **Step 2: Simplify formatters**

In `src/formatter/mod.rs`: remove `find_attached_comments()`.

In `src/formatter/bash.rs` and `src/formatter/pwsh.rs`: simplify `format()` method to just concatenate `format_entry()` calls (no group-by-type, no sorting). Or mark `format()` as a simple pass-through. Keep `format_entry()` intact.

- [ ] **Step 3: Clean up formatter trait**

Consider simplifying the `Formatter` trait: if `format()` is now trivial (just joining entries), it could be a default method. `format_entry()` remains the important method.

- [ ] **Step 4: Build and test**

```bash
cargo build 2>&1 | head -50
cargo test 2>&1 | tail -20
cargo clippy 2>&1 | tail -20
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "chore: update i18n messages, simplify formatters

- Remove dead i18n messages for deleted features
- Add messages for multi-file TUI features
- Simplify Formatter trait (remove group-by-type logic)

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Task 14: Integration Testing and Polish

**Goal:** End-to-end tests, update documentation, final cleanup.

**Files:**
- Modify: `CLAUDE.md`, `README.md`, `CHANGELOG.md`
- Modify: various test files
- Create: `tests/tui_logic_tests.rs`

- [ ] **Step 1: Write TUI logic tests**

Create `tests/tui_logic_tests.rs` testing operations without UI rendering:

```rust
// Test: build_visible_list with mixed expand states
// Test: delete_entries removes correct entries and marks files dirty
// Test: cut_entries + paste_entries moves entries between files
// Test: take_snapshot + restore_snapshot round-trips correctly
// Test: selection toggle, range, clear
```

- [ ] **Step 2: Update existing tests**

Fix any broken tests from `tests/integration/`. Update parser tests if Entry struct changed.

- [ ] **Step 3: Run full test suite**

```bash
cargo test 2>&1 | tail -30
cargo clippy 2>&1 | tail -20
cargo fmt --check
```

- [ ] **Step 4: Update `CLAUDE.md`**

Replace documentation to match new architecture:
- Remove references to checker, backup, import/export, format, sort
- Document new multi-file model (ShellProfile, ProfileFile, ListItem)
- Document new config structure ([files.*] sections)
- Document new TUI key bindings
- Document $EDITOR integration
- Update module descriptions

- [ ] **Step 5: Update `README.md`**

Update user-facing documentation:
- New usage examples (`wenv`, `wenv .`, `wenv -s bash`)
- Config file format with `[files.*]` sections
- TUI key bindings reference
- Remove references to import/export/check/backup/format

- [ ] **Step 6: Update `CHANGELOG.md`**

Add entry for this release documenting all breaking changes and new features.

- [ ] **Step 7: Final build verification**

```bash
cargo build --release 2>&1 | tail -10
cargo test 2>&1 | tail -20
```

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "docs: update documentation for multi-file refactoring

- CLAUDE.md: new architecture, config, TUI, operations
- README.md: updated usage, config format, key bindings
- CHANGELOG.md: document breaking changes and new features
- Add TUI logic tests

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```
