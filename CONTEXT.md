# CONTEXT

Glossary for the wenv codebase. Resolved terminology only — no implementation details.

## Glossary

### Group
A top-level TUI tree node that bundles one or more files. A Group is created when a config path is:
- a glob pattern (e.g. `~/.zshrc.d/*`, `/etc/profile.d/*.sh`)
- a literal filesystem directory (e.g. `$ZDOTDIR` resolving to `/home/wen/.zsh`)
- a variable that resolves to either of the above

Distinct from a **filesystem directory** (the OS concept). A Group's identity is its originating config-pattern string, not the resolved path.

Code symbol: `DirGroup` (in `src/model/profile.rs`). The struct name predates this glossary; user-facing copy says "group", not "directory".

### File
A single shell-config file on disk (e.g. `~/.zshrc`). Owned by zero or one Group; a top-level File has no Group parent.

Code symbol: `ProfileFile`.

### Entry
A single parsed item within a File — alias, function, env var, source line, comment, or raw code block. Entries are the smallest unit operated on by cut/paste/edit/delete.

Code symbol: `Entry`.

### Pattern (config pattern)
The raw string the user wrote in `config.toml` under `[files.<shell>] paths`. Pattern is the key used when the TUI's `d` removes a Group: the user removes the *pattern*, not the resolved files.

### Fallback chain
The OS-conditional ordered list of locations where wenv searches for `config.toml` at startup. The first existing match wins; if none exist, the first writable location receives a freshly-generated default. See spec §4.1.
