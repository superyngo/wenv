# Code audit — architecture, quality, security, performance, testing

Status: Resolved (2026-09-29)

Standard-depth audit of the whole tree at commit `6aad69b` (v0.21.0 + docs reorg). Line numbers
are as of that commit; symbols are the durable reference. Open items are tracked as rows in
[../plan/BACKLOG.md](../plan/BACKLOG.md); this record is the evidence.

## Executive summary

- **Health: 5.5 / 10.** Clean baseline (clippy `-D warnings`, `fmt --check`, 380 tests all
  pass), a well-factored parser/model/TUI layering, and good docs. But the tool's core promise —
  editing a user's shell rc file without damaging it — is broken in several reproducible ways.
- **Critical: 4 · High: 8 · Medium: 10 · Low: 8.** Three scout claims were rejected after
  reproduction (see [Rejected](#rejected-claims)).
- **Top 3 priorities**
  1. **Round-trip fidelity**: parse → save must not delete or rewrite lines the user did not touch
     (C1, H1, H2). All three are reproduced with the real library.
  2. **Crash safety**: remove the `strip_quotes` panic and install a terminal-restoring panic hook
     (C2, C3). Right now a panic can wreck the terminal *and* lose every unsaved edit.
  3. **Durable writes**: atomic temp+rename with a backup for rc files and `config.toml` (C4).

## Method

- `cargo clippy --all-targets -- -D warnings`, `cargo fmt -- --check`, `cargo test`, `cargo audit`.
- Three read-only scouts (TUI, parser/formatter, I/O/CLI/CI), every Critical/High claim then
  re-verified by the auditor by reading code or by reproduction.
- Parser claims reproduced with a probe crate that calls `wenv::get_parser(..).parse()` and joins
  entries exactly as `operations::save_dirty_files` does (value + `\n`). Probe lives in the
  gitignored `docs/tmp/claude-scratch/audit-probe/` (not landed).

Probe output (abridged; `in` = original file, `out` = what save would write):

```
[DIFF]  bash unclosed function at EOF   in="alias a='b'\nfoo() {\n  echo 1\n"   out="alias a='b'\n"
[DIFF]  pwsh unclosed scriptblock       in="Set-Alias a b\nfunction f {\n  1\n" out="Set-Alias a b\n"
[PANIC] bash export FOO=" (1-char)      begin <= end (1 <= 0) when slicing `"`
[PANIC] bash multiline export "         in="export FOO=\"\nline2\"\n"
[PANIC] bash source ' (1-char)
[PANIC] pwsh $env:X = " (1-char)
[DIFF]  bash indented alias             in="    alias ll='ls -la'\n"   out="alias ll='ls -la'\n"
[DIFF]  bash CRLF                       in="echo x\r\nalias a='b'\r\n" out="echo x\r\nalias a='b'\n"
[DIFF]  pwsh CRLF mixed code+alias      ...}\r\nSet-Alias a b\r\n  ->  ...}\r\nSet-Alias a b\n
[DIFF]  bash empty file                 in=""  out="\n"
[OK]    bash echo done in loop / extra close brace / UTF-8 BOM / pwsh CRLF comment+scriptblock
```

## Findings by category

Severity: 🔴 Critical · 🟠 High · 🟡 Medium · ⚪ Low. Effort: S < 1 h, M < 1 day, L > 1 day.
"Principle" cites `wens-dev-principles <domain> <n>`.

### Correctness & data integrity

#### 🔴 C1 — Unclosed block at EOF is dropped; saving deletes those lines (reproduced)
- Where: `BashParser::parse` EOF flush (`src/parser/bash/mod.rs` ~673), `PowerShellParser::parse`
  EOF flush (`src/parser/pwsh/mod.rs` ~646).
- The remaining `active_block` only produces a `ParseWarning`; it is never pushed to `entries`.
  If the user edits any other entry in that file and saves, everything from the unclosed
  `foo() {` / `function f {` / `if` to EOF is erased from disk.
- Fix: at EOF convert the pending block into a `Code` entry (keep the warning). Add a round-trip
  test per shell. Effort S. Principle ui 9.

#### 🔴 C2 — `strip_quotes` panics on a lone quote, incl. valid multi-line exports (reproduced)
- Where: `parser::builders::strip_quotes`; reached from `try_parse_env` / `try_parse_source` in
  both `bash/parsers.rs` and `pwsh/parsers.rs`.
- `trimmed[1..len-1]` with `len == 1`. Triggers on `export FOO="` + newline (a legitimate
  multi-line value), `source '`, `$env:X = "`. At startup: wenv cannot open that rc file at all.
  Inside the TUI (reload, inline edit → `replace_entry_with_parsed`) it combines with C3.
- Fix: guard `trimmed.len() >= 2`. In `try_parse_env` the panicking call's result is discarded
  (`let _value = strip_quotes(..)`) — delete it. Add tests for all four inputs. Effort S.

#### 🟠 H1 — Indentation stripped from Alias/EnvVar/Source entries (reproduced)
- Where: `bash/mod.rs` and `pwsh/mod.rs` build `Entry::new(.., trimmed)` for structured lines.
- `    alias ll='ls -la'` is saved as `alias ll='ls -la'` as soon as anything else in that file is
  edited. Violates the ENTRY_MODEL rule "`value` stores complete raw syntax".
- Fix: match on `trimmed`, store `line`. Effort S.

#### 🟠 H2 — CRLF lost on structured entries → mixed line endings on save (reproduced)
- Where: same `trimmed` path as H1 (`trim()` eats `\r`) plus `save_dirty_files` always writing
  `\n`. Code/comment blocks keep `\r\n`; aliases/exports lose it — a Windows profile becomes mixed.
- Fix: detect line ending per `ProfileFile` at load, write with it. Related: the one `.lines()`
  call in `PowerShellParser::parse` (comment + scriptblock seed, ~568) violates the ENTRY_MODEL
  split rule; its CRLF impact did not reproduce with a `function` block — fix it anyway. Effort M.

#### 🟠 H3 — External "edit file" discards unsaved in-memory edits to that file
- Where: `TuiApp::run_edit_file`.
- No `dirty` check: `$EDITOR` opens the on-disk copy, then the re-parse replaces
  `file.entries`, silently dropping unsaved TUI edits. The `read_to_string(&path)?` after the
  editor returns propagates out of the handler if the file was removed/renamed in the editor.
- Fix: if dirty, confirm (save first / discard / cancel). Effort S. Principle ui 9.

#### 🟠 H4 — Non-UTF-8 profile aborts startup with no file name
- Where: `append_resolved_pattern` (`src/model/profile.rs`) `std::fs::read_to_string(&path)?`.
- Files saved as UTF-16 (e.g. Windows PowerShell 5.1 `Out-File`/ISE) or Latin-1 comments stop
  the program with `stream did not contain valid UTF-8`, naming no path.
- Fix: read bytes; decode UTF-8/UTF-16 BOM, else skip file with a warning; add
  `.with_context(path)`. Effort M.

#### 🟠 H5 — File move reorders `config.toml` patterns by resolved-file index
- Where: `TuiApp::execute_file_move`.
- `files_config.paths.remove(original_fi)` uses an index into `profile.files` (resolved files),
  but `paths` holds raw patterns (dirs/globs expand to many files). With any group, the wrong
  pattern is moved or the move silently does nothing. `profile.tree` indices are not remapped.
- Fix: map file → pattern via `find_matching_config_pattern`; rebuild tree after reorder.
  Effort M. Principle ui 1.

#### 🟠 H6 — Undo/redo history corrupted in two paths
- `reload_profile` clears `undo_stack` on file-set change but not `redo_stack` → a later redo
  restores snapshots of the old file set.
- Pressing `d` pushes a pre-emptive snapshot via `push_undo`, which clears `redo_stack` and may
  evict the oldest undo; cancelling pops the snapshot but cannot restore either.
- Fix: clear both stacks together; push the snapshot only on confirm. Effort S. Principle ui 9.

#### 🟡 M1 — Cursor after downward cut-drop lands on the wrong row; selection cleared
- Where: `TuiApp::execute_drop`. Entry index is corrected (`removed_before_target`), but
  `self.cursor = ms.insertion_cursor` is the pre-delete visible index — not corrected for rows
  removed above it — and `selection.clear()` drops the batch. Verified by reading, not reproduced.
- Fix: restore cursor by identity (target file + inserted index) after rebuild. Effort M.
  Principle ui 17, ui 8.

#### 🟡 M2 — Trash delete ignores dirty state; no fallback when trash is unavailable
- Where: `ConfirmDeleteFile` handling in `app.rs` (`trash::delete`). Headless Linux often has no
  trash; the user is left with only a status message. Effort S.

#### ⚪ L1 — Empty file saves as `"\n"`; missing final newline is added (reproduced)
- Where: both `parse()` entry points (`"".split('\n')` → one empty entry). Guard empty input.
  Effort S.

#### ⚪ L2 — UTF-8 BOM makes line 1 parse as `Code`
- Round-trips byte-exact (reproduced OK), but the first alias/comment is misclassified and the
  BOM travels with it if moved. Strip on load, remember `has_bom`, re-emit on save. Effort S.

### Crash safety & failure handling

#### 🔴 C3 — No panic hook: any panic leaves the terminal in raw mode + alt screen
- Where: `TuiApp::run` / `main`. No `std::panic::set_hook` anywhere in `src/`.
- With C2 reachable from inside the TUI, a panic both wrecks the terminal (needs `reset`) and
  loses every unsaved edit in every file.
- Fix: panic hook that restores the terminal, chained to the default hook; RAII guard for raw
  mode. Effort S. Principle ui 9, ui 21.

#### 🔴 C4 — rc files and `config.toml` written in place, non-atomically, no backup
- Where: `operations::save_dirty_files` (`std::fs::write`), `Config::save`.
- A crash, full disk, or kill mid-write truncates `~/.bashrc`/`$PROFILE`. With several dirty
  files, a failure on the second leaves the first saved and marked clean. The crate docs still
  advertise "Automatic backups" (feature was removed).
- Fix: `tempfile::NamedTempFile::new_in(parent)` + `persist`; resolve symlinks first (stow /
  chezmoi setups) and preserve permissions; keep one `.bak` per file. Effort M.

#### 🟡 M3 — Interactive prompts fail with a raw I/O error when not on a TTY
- Where: `cli::actions::source` (`Select::interact`), `main::startup_file_check`
  (`Confirm::interact_opt`). No `is_terminal()` guard. Fix: non-TTY → print choices/help, exit
  non-zero. Effort S. Principle cli 1.

#### 🟡 M4 — `$EDITOR` with arguments fails (`code --wait`, `vim -u NONE`)
- Where: `tui::editor::edit_file`, `cli::actions::source`, `main` — `Command::new(&editor)`.
  Fix: split with `shlex` (no shell). Effort S.

### Security & supply chain

#### 🟠 H7 — Release workflow publishes binaries without tests; over-privileged
- Where: `.github/workflows/release.yml`. No `cargo test`/clippy job before `build`; top-level
  `permissions: contents: write, actions: write` (`actions: write` unused); floating action tags;
  `cargo install cross --git` unpinned. No tag ↔ `Cargo.toml` ↔ `CHANGELOG.md` version gate
  either, although repo conduct assumes one.
- Fix: `check` job as `needs:`; per-job permissions; pin `cross` to a tag (SHAs optional);
  add the version gate. Effort S.

#### 🟠 H8 — `cargo audit`: 1 vulnerability + 4 warnings
- `time 0.3.45` RUSTSEC-2026-0009 (stack-exhaustion DoS). `time` is **not used** in `src/` —
  remove it. `anyhow 1.0.100` RUSTSEC-2026-0190 (unsound `downcast_mut`) → `cargo update -p anyhow`.
  `lru 0.12.5` (two unsound advisories) and `paste` (unmaintained) come from `ratatui 0.26` →
  upgrade ratatui/crossterm (breaking; L).
- Effort: S for `time`/`anyhow`; L for ratatui.

#### ⚪ L3 — Unused / redundant dependencies
- `colored`, `terminal_size`, `thiserror` have no uses in `src/`; `tempfile` is in both
  `[dependencies]` and `[dev-dependencies]`; `assert_cmd`/`predicates` are used only by the dead
  `tests/integration/` (T1). `lazy_static` could become `std::sync::LazyLock` (MSRV permitting).
  Effort S.

No shell-injection surface found: every `Command::new` passes paths as separate args;
`source` lines are not followed recursively (no include-cycle risk).

### Performance

No hotspots that matter at rc-file scale. One cheap fix:

#### ⚪ L4 — Regexes compiled per call
- `config::path_resolver` (`expand_env_vars`, `has_unresolved_vars`, `contains_var`) and
  `model::profile::derive_file_display_in_group` call `Regex::new` on every invocation. Hoist to
  statics. Effort S.

### Internationalization & UI principles

#### 🟡 M5 — i18n subsystem is bypassed; `ui.language` has no effect
- `TuiApp.messages` is stored but never read; `ui.rs`/`app.rs` hard-code English. Only
  `en.toml` is embedded; `languages/zh-TW.toml` is neither embedded nor packaged by
  `release.yml`; any missing key in an external file makes the whole file fail to load.
- Fix: route strings through `Messages`, embed zh-TW, per-key fallback. Effort M–L.
  Principle ui 22, ui 1.

#### 🟡 M6 — Text inputs lack the TUI editing contract
- `map_filter_input_key` / `map_text_input_key` / `map_inline_edit_key`: `Ctrl+A` inserts `a`
  (no CONTROL check); no `Ctrl+E/K/U/W/Y`, no word jumps, no field undo. Effort M. Principle ui 10.

#### 🟡 M7 — Width handling for CJK/emoji
- `draw_list` passes `entry.name.len()` (bytes) to `matched_char_indices` (chars) → search
  highlights shift for non-ASCII names. Inline editor and `{:<20}` columns count chars, not cells.
- Fix: `chars().count()` (S); `unicode-width` for layout (M). Principle ui 10, ui 22.

#### ⚪ L5 — No session-only `--lang` / `--no-color` flags
- `NO_COLOR` env is honored; no CLI override. Effort S. Principle cli 3.

### Testing

#### 🟡 T1 — `tests/integration/` is tracked but never compiled
- No crate root (`tests/integration/main.rs`) and no `mod integration`. The files call binary
  `srcman` and flags that no longer exist; `tests/fixtures/*` is only referenced from them.
- Fix: delete or rewrite as `tests/cli.rs` against the `wenv` binary. Effort S–M.

#### 🟡 T2 — TUI tests exercise dead code instead of the production path
- `tests/tui_logic_tests.rs` tests `operations::cut_entries` / `paste_entries`, which production
  never calls (`TuiApp::execute_drop` re-implements move/delete). The downward-move and cursor
  logic that actually runs (M1) has no test.
- Fix: test `execute_drop` through `TuiApp` (headless) and delete the dead functions. Effort M.

#### 🟡 T3 — No encoding / line-ending / edge-input parser tests; no round-trip property
- Zero tests for CRLF, BOM, UTF-16, empty input, missing final newline, indented structured
  lines, unclosed blocks. C1, C2, H1, H2 would all have been caught by one invariant:
  `join(parse(x)) == x` for unmodified input. Add it as a table test over fixtures. Effort M.

#### ⚪ T4 — Non-hermetic tests
- `test_expand_tilde`, `test_default_config_path` read the real `$HOME`; `config_tests.rs` and
  `path_resolver_patterns.rs` call `std::env::set_var` in the parallel test runner. Effort S.
  Principle cli 3.

Coverage was not measured (no `cargo-llvm-cov`/tarpaulin installed).

### Architecture & maintainability

#### ⚪ L6 — `TuiApp::handle_action` is ~1,190 lines; `app.rs` 3,010 lines
- Seams: navigation, modal confirm/cancel, inline edit, placement (`execute_drop`,
  `execute_file_move`), external editor (`run_edit_*`). Split incrementally, behind T2 tests.
  Effort L.

#### ⚪ L7 — ~300 lines duplicated between Bash and PowerShell parsers
- `build_entry_from_pending`, pending comment/code flush, and blank-line merging are
  near-verbatim in `bash/mod.rs` and `pwsh/mod.rs` (C1 and H1 had to be found twice). Move into
  `parser::pending` (`PendingBlock::into_entry`). Effort M.

#### ⚪ L8 — Stale crate docs and dead helpers
- `src/lib.rs` `//!` lists "Import from files and URLs", "Export entries", "Automatic backups" —
  all removed. `utils::path::{read_file, write_file, check_file_readable, normalize_path}` have
  no callers. ADR 0002 deviates from `wens-dev-principles cli 2` (MUST: XDG / per-platform dirs)
  but does not cite it, as `wens-dev-principles docs 15` requires. Effort S.

## Resolution (2026-09-29)

All findings were addressed the same day; commits are listed in [../plan/BACKLOG.md](../plan/BACKLOG.md)
under Done. Notes on deviations from the recommended fixes:

- **T4** (test hermeticity) is not scheduled: env vars used with `set_var` have unique names and
  no other reader (recorded under Blocked in the backlog).
- **M6**: kill/yank, word motion and caret shortcuts shipped; per-field undo did not.
- **M2**: no permanent-delete fallback was added (irreversible); the prompt now warns about unsaved
  edits and a trash failure says the file was left in place.
- **H5**: moves *inside* a glob/dir group are refused (config order cannot express them) instead of
  silently doing nothing.
- **Found while verifying**: Help/Detail popups panicked on terminals under 3 rows / 0 columns
  (`clamp(min > max)`); fixed and regression-tested. `cargo audit` is clean after the ratatui 0.30
  upgrade.

## Rejected claims

Reproduced and **not** confirmed; do not act on them without new evidence:

- `count_control_end` splitting a loop at `echo done` — the probe keeps the loop as one entry.
- Negative `brace_count` keeping a function open to EOF — extra `}` round-trips exactly.
- `app.rs` `unwrap()` risk — 87 of 88 are in `#[cfg(test)]`; the one in production is guarded.

## Prioritized action plan

1. **Quick wins (< 1 day total)** — C2 guard + tests; C3 panic hook; C1 EOF flush; H1 store raw
   line; H6 undo/redo; H3 dirty check; remove `time`, bump `anyhow` (H8); H7 CI gate +
   permissions; L3 unused deps; M3 TTY guard; M4 `$EDITOR` split; M7 byte/char fix; L8 docs.
2. **Medium term (1–5 days)** — C4 atomic save + backup; H2 line-ending preservation; H4
   encoding; H5 file move by pattern; T3 round-trip fixture suite; T1/T2 test repair; M1 cursor
   by identity; M6 input contract.
3. **Long term (> 5 days)** — ratatui/crossterm upgrade (H8); i18n wiring (M5); split
   `handle_action` (L6); shared parser core (L7).

## Metrics

| Metric | Value |
|---|---|
| Rust files analyzed | 46 in `src/`, 13 in `tests/` |
| Lines of code | 14,433 `src/` + 1,654 `tests/` |
| Tests | 380 passing, 0 failing (+16 ignored doctests); 3 test files never compiled |
| Clippy / fmt | Clean / clean |
| `cargo audit` | 1 vulnerability, 4 warnings |
| Test coverage | Not measured |
| Complexity hotspots | 4 — `TuiApp::handle_action`, `BashParser::parse`, `PowerShellParser::parse`, `TuiApp::execute_drop` |
