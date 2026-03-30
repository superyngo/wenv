# Snippet Template Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a snippet template selection popup to the TUI 'n' (new entry) flow, with shell-specific defaults stored in config.toml.

**Architecture:** New `AppMode::SelectingSnippet` popup intercepts the 'n' keypress. Config gains `snippets` and `template_paths` fields with serde support. `run_add_entry` accepts an optional template string to pre-fill the editor temp file.

**Tech Stack:** Rust, ratatui (TUI), serde/toml (config), crossterm (terminal)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/model/config.rs` | Modify | Add `Snippet`, `TemplatePathsConfig` structs; extend `Config` with `snippets` and `template_paths` fields |
| `src/config/templates.rs` | Modify | Add `default_snippets()` function returning shell-specific default snippets |
| `src/config/mod.rs` | Modify | Add `ensure_shell_snippets()` and `load_snippets_for_shell()` functions |
| `src/model/mod.rs` | Modify | Export new `Snippet` and `TemplatePathsConfig` types |
| `src/tui/state.rs` | Modify | Add `AppMode::SelectingSnippet` variant |
| `src/tui/keys.rs` | Modify | Add `SnippetNavigateUp`, `SnippetNavigateDown`, `SnippetSelect`, `SnippetCancel` actions; add `map_snippet_key()` function |
| `src/tui/ui.rs` | Modify | Add `draw_snippet_popup()` function; wire into `draw()` match |
| `src/tui/app.rs` | Modify | Add snippet state fields; change `Action::Add` handler to enter snippet mode; handle snippet actions; modify `run_add_entry` signature |
| `src/main.rs` | Modify | Call `ensure_shell_snippets` during init; pass snippets to `TuiApp` |

---

### Task 1: Data Model — Snippet Struct and Config Extension

**Files:**
- Modify: `src/model/config.rs`
- Modify: `src/model/mod.rs`

- [ ] **Step 1: Write the failing test**

Add test at the bottom of `src/model/config.rs` in the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_snippet_serialization() {
    use super::{Snippet, TemplatePathsConfig};

    let snippet = Snippet {
        name: "alias".into(),
        description: "Define an alias".into(),
        template: Some("alias NAME='VALUE'".into()),
    };
    let toml_str = toml::to_string(&snippet).unwrap();
    let parsed: Snippet = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.name, "alias");
    assert_eq!(parsed.template, Some("alias NAME='VALUE'".into()));
}

#[test]
fn test_config_with_snippets() {
    use super::{Snippet, TemplatePathsConfig};
    use std::collections::HashMap;

    let mut config = Config::default();
    let snippets = vec![
        Snippet {
            name: "Empty".into(),
            description: "Blank entry".into(),
            template: None,
        },
        Snippet {
            name: "alias".into(),
            description: "Define an alias".into(),
            template: Some("alias NAME='VALUE'".into()),
        },
    ];
    config.snippets.insert("zsh".into(), snippets);
    config.template_paths = TemplatePathsConfig {
        paths: vec!["~/.config/wenv/snippets/extra.toml".into()],
    };

    let toml_str = toml::to_string_pretty(&config).unwrap();
    assert!(toml_str.contains("[snippets.zsh]"));
    assert!(toml_str.contains("[template_paths]"));

    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.snippets["zsh"].len(), 2);
    assert!(parsed.snippets["zsh"][0].template.is_none());
    assert_eq!(parsed.template_paths.paths.len(), 1);
}

#[test]
fn test_config_without_snippets_parses() {
    // Existing config without snippets section should parse fine
    let toml_str = "[ui]\nlanguage = \"en\"\n";
    let parsed: Config = toml::from_str(toml_str).unwrap();
    assert!(parsed.snippets.is_empty());
    assert!(parsed.template_paths.paths.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib model::config`
Expected: FAIL — `Snippet` and `TemplatePathsConfig` types not found

- [ ] **Step 3: Write implementation**

Add to `src/model/config.rs` after the `FilesConfig` struct:

```rust
/// A snippet template for new entry creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub template: Option<String>,
}

/// External template file paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePathsConfig {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Default for TemplatePathsConfig {
    fn default() -> Self {
        Self { paths: Vec::new() }
    }
}
```

Modify the `Config` struct to add the two new fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub files: HashMap<String, FilesConfig>,
    #[serde(default)]
    pub snippets: HashMap<String, Vec<Snippet>>,
    #[serde(default)]
    pub template_paths: TemplatePathsConfig,
}
```

Update `Config::default()`:

```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig {
                language: default_language(),
            },
            files: HashMap::new(),
            snippets: HashMap::new(),
            template_paths: TemplatePathsConfig::default(),
        }
    }
}
```

Update `src/model/mod.rs` to export the new types:

```rust
pub use config::{Config, FilesConfig, Snippet, TemplatePathsConfig, UiConfig};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib model::config`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/model/config.rs src/model/mod.rs
git commit -m "feat(model): add Snippet and TemplatePathsConfig to Config"
```

---

### Task 2: Default Snippets Templates

**Files:**
- Modify: `src/config/templates.rs`
- Modify: `src/config/mod.rs`

- [ ] **Step 1: Write the failing test**

Add tests at the bottom of `src/config/templates.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_snippets_bash() {
        let snippets = default_snippets("bash").unwrap();
        assert!(snippets.len() >= 5);
        assert_eq!(snippets[0].name, "Empty");
        assert!(snippets[0].template.is_none());
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"source"));
        assert!(names.contains(&"alias"));
        assert!(names.contains(&"export"));
        assert!(names.contains(&"function"));
        assert!(!names.contains(&"bindkey"));
    }

    #[test]
    fn test_default_snippets_zsh() {
        let snippets = default_snippets("zsh").unwrap();
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"bindkey"));
        // zsh includes all bash snippets plus bindkey
        assert!(names.contains(&"source"));
        assert!(names.contains(&"alias"));
    }

    #[test]
    fn test_default_snippets_pwsh() {
        let snippets = default_snippets("powershell").unwrap();
        let names: Vec<&str> = snippets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Empty"));
        assert!(names.contains(&"source"));
        assert!(names.contains(&"env"));
        assert!(names.contains(&"alias"));
        assert!(names.contains(&"function"));
        assert!(names.contains(&"enum"));
        assert!(names.contains(&"class"));
        assert!(names.contains(&"scriptblock"));
    }

    #[test]
    fn test_default_snippets_unknown_shell() {
        assert!(default_snippets("fish").is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::templates`
Expected: FAIL — `default_snippets` not found

- [ ] **Step 3: Write implementation**

Add to `src/config/templates.rs` after the existing `generate_default_config` function:

```rust
use crate::model::Snippet;

pub fn default_snippets(shell_key: &str) -> Option<Vec<Snippet>> {
    match shell_key {
        "bash" => Some(vec![
            Snippet { name: "Empty".into(), description: "Blank entry".into(), template: None },
            Snippet { name: "source".into(), description: "Source a file".into(),
                template: Some("# Source a shell file\nsource PATH".into()) },
            Snippet { name: "export".into(), description: "Set environment variable".into(),
                template: Some("# Set variable name and value\nexport NAME='VALUE'".into()) },
            Snippet { name: "alias".into(), description: "Define an alias".into(),
                template: Some("# Set alias name and value\nalias NAME='VALUE'".into()) },
            Snippet { name: "function".into(), description: "Define a function".into(),
                template: Some("# Define function name and body\nNAME() {\n    # body\n}".into()) },
        ]),
        "zsh" => Some(vec![
            Snippet { name: "Empty".into(), description: "Blank entry".into(), template: None },
            Snippet { name: "source".into(), description: "Source a file".into(),
                template: Some("# Source a shell file\nsource PATH".into()) },
            Snippet { name: "export".into(), description: "Set environment variable".into(),
                template: Some("# Set variable name and value\nexport NAME='VALUE'".into()) },
            Snippet { name: "alias".into(), description: "Define an alias".into(),
                template: Some("# Set alias name and value\nalias NAME='VALUE'".into()) },
            Snippet { name: "function".into(), description: "Define a function".into(),
                template: Some("# Define function name and body\nNAME() {\n    # body\n}".into()) },
            Snippet { name: "bindkey".into(), description: "Bind a key".into(),
                template: Some("# Bind key to widget\nbindkey KEY WIDGET".into()) },
        ]),
        "powershell" => Some(vec![
            Snippet { name: "Empty".into(), description: "Blank entry".into(), template: None },
            Snippet { name: "source".into(), description: "Source a file".into(),
                template: Some("# Source a PowerShell file\n. PATH".into()) },
            Snippet { name: "env".into(), description: "Set environment variable".into(),
                template: Some("# Set environment variable\n$env:NAME = \"VALUE\"".into()) },
            Snippet { name: "alias".into(), description: "Define an alias".into(),
                template: Some("# Set alias name and command\nSet-Alias -Name NAME -Value COMMAND".into()) },
            Snippet { name: "function".into(), description: "Define a function".into(),
                template: Some("# Define function name and body\nfunction NAME {\n    # body\n}".into()) },
            Snippet { name: "enum".into(), description: "Define an enum".into(),
                template: Some("# Define enum type\nenum NAME {\n    VALUE1\n    VALUE2\n}".into()) },
            Snippet { name: "class".into(), description: "Define a class".into(),
                template: Some("# Define class\nclass NAME {\n    # properties and methods\n}".into()) },
            Snippet { name: "scriptblock".into(), description: "Script block".into(),
                template: Some("# Script block\n{\n    # code\n}".into()) },
        ]),
        _ => None,
    }
}
```

Add `ensure_shell_snippets` and `load_snippets_for_shell` to `src/config/mod.rs`:

```rust
use crate::model::{Config, FilesConfig, Snippet};

/// Ensure config has snippets for the given shell. Returns true if added.
pub fn ensure_shell_snippets(config: &mut Config, shell_key: &str) -> anyhow::Result<bool> {
    if config.snippets.contains_key(shell_key) {
        return Ok(false);
    }
    if let Some(snippets) = templates::default_snippets(shell_key) {
        config.snippets.insert(shell_key.to_string(), snippets);
        config.save()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Load merged snippets for a shell: inline from config + external files, deduped by name.
pub fn load_snippets_for_shell(config: &Config, shell_key: &str) -> Vec<Snippet> {
    let mut result: Vec<Snippet> = config
        .snippets
        .get(shell_key)
        .cloned()
        .unwrap_or_default();

    let mut seen_names: std::collections::HashSet<String> =
        result.iter().map(|s| s.name.clone()).collect();

    for path_str in &config.template_paths.paths {
        let resolved = crate::config::path_resolver::resolve_tilde(path_str);
        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let external: toml::Value = match toml::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(shell_snippets) = external.get("snippets").and_then(|s| s.get(shell_key)) {
            if let Some(array) = shell_snippets.as_array() {
                for item in array {
                    if let Ok(snippet) = item.clone().try_into::<Snippet>() {
                        if !seen_names.contains(&snippet.name) {
                            seen_names.insert(snippet.name.clone());
                            result.push(snippet);
                        }
                    }
                }
            }
        }
    }

    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::templates && cargo test --lib config`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config/templates.rs src/config/mod.rs
git commit -m "feat(config): add default snippets per shell and snippet loading"
```

---

### Task 3: AppMode and Key Bindings

**Files:**
- Modify: `src/tui/state.rs`
- Modify: `src/tui/keys.rs`

- [ ] **Step 1: Add `AppMode::SelectingSnippet` to state.rs**

In `src/tui/state.rs`, add the variant to the `AppMode` enum:

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
    TextInput,
    ConfirmRemoveFile,
    ConfirmCreateFile,
    MovingFile,
    SelectingSnippet,
}
```

- [ ] **Step 2: Add snippet key mapping to keys.rs**

Add new actions to the `Action` enum in `src/tui/keys.rs`:

```rust
pub enum Action {
    // ... existing variants ...
    SnippetUp,
    SnippetDown,
    SnippetSelect,
    Noop,
}
```

Update `map_key` in `src/tui/keys.rs` to handle the new mode:

```rust
pub fn map_key(mode: &AppMode, key: KeyEvent) -> Action {
    match mode {
        AppMode::Normal => map_normal_key(key),
        AppMode::Moving => map_moving_key(key),
        AppMode::MovingFile => map_moving_key(key),
        AppMode::Searching => map_search_key(key),
        AppMode::ShowingDetail => map_detail_key(key),
        AppMode::TextInput => map_text_input_key(key),
        AppMode::SelectingSnippet => map_snippet_key(key),
        _ => map_popup_key(key),
    }
}
```

Add the `map_snippet_key` function after `map_text_input_key`:

```rust
fn map_snippet_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::SnippetUp,
        KeyCode::Down | KeyCode::Char('j') => Action::SnippetDown,
        KeyCode::Enter => Action::SnippetSelect,
        KeyCode::Esc => Action::SnippetCancel,
        _ => Action::Noop,
    }
}
```

Note: `SnippetCancel` is a new action — add it to the enum:

```rust
pub enum Action {
    // ... existing variants ...
    SnippetUp,
    SnippetDown,
    SnippetSelect,
    SnippetCancel,
    Noop,
}
```

And map `SnippetCancel`:

```rust
fn map_snippet_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::SnippetUp,
        KeyCode::Down | KeyCode::Char('j') => Action::SnippetDown,
        KeyCode::Enter => Action::SnippetSelect,
        KeyCode::Esc => Action::SnippetCancel,
        _ => Action::Noop,
    }
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check`
Expected: PASS (compiles, but snippet actions are not yet handled in app.rs — that's Task 5)

- [ ] **Step 4: Commit**

```bash
git add src/tui/state.rs src/tui/keys.rs
git commit -m "feat(tui): add SelectingSnippet mode and key bindings"
```

---

### Task 4: Snippet Popup Rendering

**Files:**
- Modify: `src/tui/ui.rs`

- [ ] **Step 1: Add `draw_snippet_popup` function**

Add after `draw_help_popup` in `src/tui/ui.rs` (after line 629):

```rust
fn draw_snippet_popup(f: &mut Frame, area: Rect, app: &TuiApp) {
    let snippets = &app.snippets;
    if snippets.is_empty() {
        return;
    }

    let hint_line = " \u{2191}\u{2193} navigate  Enter select  Esc";
    let mut lines: Vec<Line> = Vec::new();

    let name_width = snippets
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(0);

    for (i, snippet) in snippets.iter().enumerate() {
        let is_selected = i == app.snippet_cursor;
        let name_part = format!("  {:<width$}", snippet.name, width = name_width);
        let desc_part = if snippet.description.is_empty() {
            String::new()
        } else {
            format!(" \u{2014} {}", snippet.description)
        };

        let style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{}{}", name_part, desc_part),
            style,
        )));
    }

    // Blank line + hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        hint_line,
        Style::default().fg(Color::DarkGray),
    )));

    // Calculate popup size
    let max_line_width = snippets
        .iter()
        .map(|s| {
            let w = name_width + 3 + s.description.len();
            hint_line.len().max(w)
        })
        .max()
        .unwrap_or(20);
    let content_lines = lines.len() as u16;
    let popup_width = ((max_line_width as u16) + 4)
        .min((area.width * 4) / 5)
        .min(area.width - 4);
    let max_height = (area.height * 4) / 5;
    let popup_height = (content_lines + 2).min(max_height);

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" New Entry ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_height = popup_height.saturating_sub(2);
    let snippet_count = snippets.len() as u16;
    // snippet_scroll_offset tracks the first visible snippet index
    let max_scroll = snippet_count.saturating_sub(inner_height.saturating_sub(2)); // -2 for blank+hint
    let scroll_offset = app.snippet_scroll_offset.min(max_scroll as usize) as u16;

    let mut paragraph = Paragraph::new(lines).block(block);
    if max_scroll > 0 {
        paragraph = paragraph.scroll((scroll_offset, 0));
    }
    f.render_widget(paragraph, popup_area);
}
```

- [ ] **Step 2: Wire into the `draw` function**

In `src/tui/ui.rs`, update the popup match in `draw()` (around line 124-139) to include `SelectingSnippet`:

```rust
    // Draw popups on top
    match &app.mode {
        AppMode::ConfirmDelete
        | AppMode::ConfirmQuit
        | AppMode::ConfirmRemoveFile
        | AppMode::ConfirmCreateFile => {
            draw_confirm_popup(f, f.size(), app);
        }
        AppMode::ShowingDetail => {
            draw_detail_popup(f, f.size(), app);
        }
        AppMode::ShowingHelp => {
            draw_help_popup(f, f.size(), app);
        }
        AppMode::SelectingSnippet => {
            draw_snippet_popup(f, f.size(), app);
        }
        _ => {}
    }
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check`
Expected: FAIL — `app.snippets` and `app.snippet_cursor` and `app.snippet_scroll_offset` fields don't exist yet on `TuiApp`. That's expected, will be added in Task 5.

- [ ] **Step 4: Commit**

```bash
git add src/tui/ui.rs
git commit -m "feat(tui): add snippet popup rendering"
```

---

### Task 5: App State and Action Handling

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add snippet state fields to TuiApp**

In `src/tui/app.rs`, add fields to the `TuiApp` struct (after `detail_page_size: u16`):

```rust
pub struct TuiApp {
    // ... existing fields ...
    pub detail_scroll_offset: u16,
    pub detail_page_size: u16,
    pub snippet_cursor: usize,
    pub snippet_scroll_offset: usize,
    pub snippets: Vec<crate::model::Snippet>,
}
```

Update `TuiApp::new()` constructor to initialize them (after `detail_page_size: 10,`):

```rust
snippet_cursor: 0,
snippet_scroll_offset: 0,
snippets: crate::config::load_snippets_for_shell(&config, &shell_key),
```

- [ ] **Step 2: Handle snippet actions in `handle_action`**

In `src/tui/app.rs`, add to the `handle_action` match block. Find the `Action::Add` handler (around line 332) and change it to enter snippet selection instead of directly adding:

```rust
Action::Add => {
    if !self.is_current_file_writable() {
        self.message = Some("File is read-only".into());
        return Ok(EditorRequest::None);
    }
    if self.snippets.is_empty() {
        // No snippets configured — fall back to empty flow
        let fi = self.current_file_index();
        return Ok(EditorRequest::AddEntry(fi));
    }
    self.snippet_cursor = 0;
    self.snippet_scroll_offset = 0;
    self.mode = AppMode::SelectingSnippet;
    return Ok(EditorRequest::None);
}
Action::SnippetUp => {
    if self.snippet_cursor > 0 {
        self.snippet_cursor -= 1;
        if self.snippet_cursor < self.snippet_scroll_offset {
            self.snippet_scroll_offset = self.snippet_cursor;
        }
    }
    return Ok(EditorRequest::None);
}
Action::SnippetDown => {
    if self.snippet_cursor < self.snippets.len() - 1 {
        self.snippet_cursor += 1;
    }
    return Ok(EditorRequest::None);
}
Action::SnippetSelect => {
    let fi = self.current_file_index();
    let template = self
        .snippets
        .get(self.snippet_cursor)
        .and_then(|s| s.template.clone());
    self.mode = AppMode::Normal;
    return Ok(EditorRequest::AddEntryWithTemplate(fi, template));
}
Action::SnippetCancel => {
    self.mode = AppMode::Normal;
    return Ok(EditorRequest::None);
}
```

- [ ] **Step 3: Add `AddEntryWithTemplate` to `EditorRequest`**

In `src/tui/app.rs`, update the `EditorRequest` enum:

```rust
enum EditorRequest {
    None,
    EditFile(usize),
    EditEntry(usize, usize),
    AddEntry(usize),
    AddEntryWithTemplate(usize, Option<String>),
}
```

Update the event loop match (around line 139-144) to handle the new variant:

```rust
EditorRequest::AddEntry(fi) => {
    self.run_add_entry(terminal, fi, None)?;
    if self.mode == AppMode::Searching {
        self.update_search_and_navigate();
    }
}
EditorRequest::AddEntryWithTemplate(fi, template) => {
    self.run_add_entry(terminal, fi, template.as_deref())?;
    if self.mode == AppMode::Searching {
        self.update_search_and_navigate();
    }
}
```

- [ ] **Step 4: Modify `run_add_entry` signature and body**

Change the signature:

```rust
fn run_add_entry(
    &mut self,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    fi: usize,
    template: Option<&str>,
) -> Result<()> {
```

Change the `edit_temp_content` call to use the template:

```rust
Self::suspend_tui(terminal)?;
let initial_content = template.unwrap_or("");
let result = crate::tui::editor::edit_temp_content(initial_content, suffix);
Self::resume_tui(terminal)?;
```

- [ ] **Step 5: Add `ensure_shell_snippets` call in main.rs**

In `src/main.rs`, after the `ensure_shell_files` call (line 113-114), add:

```rust
wenv::config::ensure_shell_snippets(&mut config, shell_key)?;
```

The full block becomes:

```rust
if !config.files.contains_key(shell_key) {
    wenv::config::ensure_shell_files(&mut config, shell_key)?;
}
wenv::config::ensure_shell_snippets(&mut config, shell_key)?;
```

- [ ] **Step 6: Run cargo check**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Run full tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/tui/app.rs src/main.rs
git commit -m "feat(tui): wire snippet selection into new entry flow"
```

---

### Task 6: Manual Smoke Test

**Files:** None (manual testing)

- [ ] **Step 1: Build and run**

Run: `cargo run`

- [ ] **Step 2: Verify popup appears**

Press 'n' on an expanded file. Expect a centered popup titled "New Entry" listing snippets for the current shell. "Empty" should be highlighted by default.

- [ ] **Step 3: Verify navigation**

Press j/k or arrow keys. Highlight moves. Press Esc — popup closes, back to normal mode.

- [ ] **Step 4: Verify Empty selection**

Select "Empty" and press Enter. Editor opens with blank file. Quit editor without saving — expect "Cancelled" message.

- [ ] **Step 5: Verify template selection**

Select "alias" and press Enter. Editor opens with pre-filled content including comment and `alias NAME='VALUE'`. Quit editor without saving.

- [ ] **Step 6: Verify config.toml**

Check `~/.config/wenv/config.toml` now contains `[snippets.zsh]` (or the active shell) entries. Verify they can be manually edited.

---

### Task 7: CHANGELOG and Docs Update

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md` (if architecture section needs update)

- [ ] **Step 1: Update CHANGELOG**

Add unreleased entry:

```
## Unreleased

### Added
- Snippet template selection menu for new entry creation (press 'n' to choose from shell-specific templates)
- Configurable snippet templates in config.toml with external file support via [template_paths]
```

- [ ] **Step 2: Update CLAUDE.md architecture section**

Add snippet-related documentation to the Architecture > Configuration System section in `CLAUDE.md`, noting the new `[snippets.<shell>]` and `[template_paths]` config sections.

Add `SelectingSnippet` to the TUI Key Bindings Reference table.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md CLAUDE.md
git commit -m "docs: update CHANGELOG and CLAUDE.md for snippet template feature"
```
