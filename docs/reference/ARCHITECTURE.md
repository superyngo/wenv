# Architecture

Module map and runtime behavior. Terms: [glossary.md](glossary.md). Entry data:
[ENTRY_MODEL.md](ENTRY_MODEL.md). Keys: [KEYMAP.md](KEYMAP.md).

## Data model (`src/model/profile.rs`)

- `ShellProfile { shell_type, files: Vec<ProfileFile>, tree: Vec<TreeNode> }` — `tree` orders
  top-level Files and Groups.
- `ProfileFile { path, entries, content, expanded, dirty, exists, writable, display_label }`.
- `DirGroup { source_pattern, display_label, file_indices, expanded }` — a Group.
- `ListItem::{DirHeader, FileHeader, Entry(file_index, entry_index)}` — TUI rows.

## Modules

- `parser/` — `Parser` trait; `BashParser`, `PwshParser`; shared `PendingBlock` in `pending.rs`.
- `formatter/` — `Formatter` trait; shell-specific file reconstruction.
- `config/` — path resolution (`path_resolver.rs`), PowerShell `cache.rs`, `templates.rs`.
- `model/` — `Entry`, `ShellType`, `ShellProfile`, `Config`, `Snippets`.
- `tui/` — `app.rs` (state/events), `ui.rs`/`list.rs` (rendering), `keys.rs` (bindings, help),
  `operations.rs` (delete/placement/undo), `selection.rs`, `state.rs` (modes, `MoveState`,
  `InlineEditState`), `editor.rs` (`$EDITOR`), `search.rs` (fuzzy filter).
- `utils/` — path expansion, shell detection, string helpers.
- `i18n/` — English embedded; external `~/.config/wenv/i18n/{lang}.toml`, chosen by `[ui] language`.

## Editing

- **Inline editor** (`AppMode::InlineEdit`): single-line Entries edit their whole raw `value`;
  commit re-parses via `apply_edited_value` → `replace_entry_with_parsed`. Horizontal overflow
  scrolling (`clamp_inline_scroll`) with a status-bar `col {caret}/{len}` hint.
- **`$EDITOR`** (`get_editor`): `$EDITOR`, else `vi` (`notepad` on Windows); temp file carries a
  shell extension; output is parsed back into Entries.
- **Placement** (`c`/`x`): see [glossary.md](glossary.md); switching to cut is rejected when a
  source File is read-only.
- **Undo/redo**: snapshots restore all Files; **save** writes all dirty Files.

## Shell detection

`--shell` flag, else `$SHELL`/`$0`. No config-based preference.

## Configuration

**`config.toml`** — single fixed location `~/.config/wenv/config.toml` on all platforms, created
from a template when missing, overridable with `-c, --config <PATH>`. Loaded by
`Config::resolve_or_create`. Sections: `[ui]` and `[files.<shell>] paths = [...]` (Patterns).

**`Resources/snippets.toml`** — mandatory bundled resource for the `a` snippet menu; never
auto-generated. `Snippets::resolve()` searches the in-repo copy (debug builds),
`<exe_dir>/Resources/`, then platform install locations; if none exists wenv prints the searched
paths and exits non-zero. Format: `[[snippets.<shell>]]` with `name`, `description`, `template?`.

**`cache.toml`** — next to the resolved `config.toml`; stores detected `pwsh_profile` /
`powershell_profile` paths (`Cache::load_or_default`, `Cache::save`). Entries whose path no
longer exists are dropped on load.
