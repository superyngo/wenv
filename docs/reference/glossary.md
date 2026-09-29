# Glossary

Resolved terminology for wenv. Code, UI strings, commits, and docs use these terms.

**Group**:
A top-level TUI tree node that bundles one or more Files. A Group is created when a Pattern is a
glob (`~/.zshrc.d/*`), a literal filesystem directory (`$ZDOTDIR` resolving to `/home/wen/.zsh`),
or a variable that resolves to either. A Group's identity is its originating Pattern, not the
resolved path. Code symbol: `DirGroup` (name predates this glossary; user-facing copy says "group").
_Avoid_: directory, folder (those mean the OS concept).

**File**:
A single shell-config file on disk (e.g. `~/.zshrc`). Owned by zero or one Group; a top-level File
has no Group parent. Code symbol: `ProfileFile`.
_Avoid_: profile, config (ambiguous with `config.toml`).

**Entry**:
A single parsed item within a File — alias, function, env var, source line, comment, script block,
or raw code block. The smallest unit operated on by copy/cut/edit/delete. Code symbol: `Entry`.
See [ENTRY_MODEL.md](ENTRY_MODEL.md).
_Avoid_: line, item, record.

**Pattern**:
The raw string the user wrote in `config.toml` under `[files.<shell>] paths`. Removing a Group or
standalone File with `d` removes the Pattern, not the resolved files.
_Avoid_: path (a Pattern may be a glob or variable, not a path).

**Placement**:
The shared copy/cut flow (`c`/`x`): sources are marked, a target box is steered, and a single drop
inserts clones (copy) or clones-and-removes (cut). Code symbols: `AppMode::Moving`, `MoveState`.
_Avoid_: clipboard, paste (there is no persistent clipboard).
