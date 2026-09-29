# Entry Model

Current data model for parsed Entries. See [glossary.md](glossary.md) for terms,
[PARSER_FLOW.md](PARSER_FLOW.md) for how Entries are produced, and
[PENDING_STATE_MACHINE.md](PENDING_STATE_MACHINE.md) for comment/code merging.

## Fields (`Entry`)

- `entry_type` (`EntryType`: `Alias`, `Function`, `EnvVar`, `Source`, `Code`, `Comment`,
  `ScriptBlock`) — classification for UI filtering/grouping only.
- `name` — extracted identifier for display/search only (alias name, function name, `L10-L12` for Code).
- `value` — **complete raw syntax**, including keywords, options, quotes, and merged comments/blank lines.
- `line_number` / `end_line` — `Option<usize>` start and end line in the source File.
- `file_index` — index into `ShellProfile.files`.

## Value semantics

`value` is never stripped of keywords or quotes:

| Type | Example `value` |
|---|---|
| Alias | `alias -g ll='ls -la'` |
| EnvVar | `export PATH="/usr/bin"` |
| Function | `foo() { echo hi; }` |
| Source | `source ~/.profile` |
| Comment | `# This is a comment` |
| Code | `if true; then\n  echo hi\nfi` |

Comments/blank lines that precede a structured Entry merge into it: `value = "# comment\n\nalias foo='bar'"`,
`entry_type = Alias`, `name = "foo"`.

## Newline formats

Two newline conventions coexist; confusing them causes off-by-one bugs.

| Format | Rule | Example (3 lines) | Used by |
|---|---|---|---|
| Separator | N lines = N-1 `\n` | `"line1\n\n"` | `value` |
| Terminator | N lines = N `\n` | `"line1\n\n\n"` | File contents |

- Split `value` with `value.split('\n')`, never `.lines()` or `split_lines_preserve_trailing()`.
- Writing separator-format content to a File appends a `\n` terminator.
- When checking "ends with newline", distinguish content from terminator.

## Parsing rules

- **Lenient:** unparseable lines are skipped with a warning; parsing continues.
- **Control-structure aware:** the Bash parser tracks `if`/`while`/`for`/`case` depth and only
  extracts top-level definitions.
- **No regex backreferences:** single-, double-, and unquoted aliases use separate patterns.
