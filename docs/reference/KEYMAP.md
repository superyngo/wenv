# TUI Keymap

Normal-mode bindings (source of truth: `map_key` family in `src/tui/keys.rs`). See
[ARCHITECTURE.md](ARCHITECTURE.md) for the modes behind them.

| Key | Action |
|-----|--------|
| `j`/`k`, `↑`/`↓` | Navigate entries |
| `Enter`/`Space` | Toggle expand/collapse |
| `s` | Toggle selection |
| `Shift+↑`/`↓` | Extend selection range |
| `e` | Edit — single-line Entries edit inline; multi-line Entries and File headers open `$EDITOR` |
| `E` | Edit in `$EDITOR` (force external) |
| `a` | Insert Entry via snippet menu, then `$EDITOR`. On a Group header: prompt for a filename, create it in the Group's directory, open `$EDITOR` |
| `n` | Add a Pattern to config (file, `~`, `$VAR`/`%VAR%`, glob, or directory; a single missing file offers to create it) |
| `d` / `Delete` | Delete Entries / remove Pattern / remove Group. On a File header inside a Group: move the real file to the system trash after `y/n` |
| `c` | Copy — enter Placement; `v`/`Enter` drops clones, `Esc` cancels, `c`/`x` switch copy↔move |
| `x` | Cut — enter Placement; drop removes the sources (net move) |
| `m` | Reorder Files (File header only) |
| `r` | Toggle remark (comment/uncomment) |
| `0` / `9` | Collapse / expand all |
| `z` / `y` | Undo / redo (20-step history) |
| `/` | Filter (fuzzy); `Enter` commits, `Esc` clears |
| `Ctrl+A` / `Ctrl+E` | In any text input: caret to line start / end |
| `Ctrl+U` / `Ctrl+K` / `Ctrl+W` / `Ctrl+Y` | In any text input: kill to start / kill to end / delete word / yank |
| `Alt+B` / `Alt+F`, `Ctrl+←` / `Ctrl+→` | In any text input: word left / right |
| `Ctrl+Z` | In any text input: undo the last edit in that field |
| `Esc` | Clear selection / exit mode |
| `w` | Save all changes |
| `?` | Help popup (scrollable, with About section from `Cargo.toml`) |
| `q` | Quit (confirms if unsaved) |
