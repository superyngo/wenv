# Backlog

Status: In progress

The single living tracker for open follow-ups. Rows move to **Done** with the closing commit; never deleted.

## Open

| Item | Evidence | Verified | Priority | Effort | Acceptance |
|---|---|---|---|---|---|
| Rewrite `PENDING_STATE_MACHINE.md` in English against current code | `docs/reference/PENDING_STATE_MACHINE.md` §2 and §5 cite `CodeBlockBuilder`, which no longer exists (`src/parser/builders/` has `CommentBlockBuilder`, `QuotedValueBuilder`) | 2026-09-29 | Low | M | Every symbol named in the doc exists in `src/` |
| Verify `PARSER_FLOW.md` against current parsers | `docs/reference/PARSER_FLOW.md`, `PendingBlock`, `BoundaryType` | — | Low | S | Claims checked; doc covers PowerShell as well as Bash |
| Add missing `LICENSE` file | `README.md` links `LICENSE`; `Cargo.toml` declares `license = "MIT"`; no file in repo | 2026-09-29 | Medium | S | `LICENSE` (MIT) exists at repo root; README link resolves |
| Parser drops unclosed block at EOF — save deletes those lines (audit C1) | `BashParser::parse`, `PowerShellParser::parse` EOF flush; [audit](../audit/2026-09-29-code-audit.md) probe | 2026-09-29 | Critical | S | Unclosed function/if/scriptblock at EOF round-trips byte-exact in both shells; test added |
| `strip_quotes` panics on a lone quote (audit C2) | `parser::builders::strip_quotes` via `try_parse_env`/`try_parse_source`; `export FOO="⏎…"` panics | 2026-09-29 | Critical | S | `export FOO="`, multi-line export, `source '`, `$env:X = "` parse without panic; tests added |
| Install terminal-restoring panic hook (audit C3) | No `set_hook` in `src/`; `TuiApp::run` raw mode + alt screen | 2026-09-29 | Critical | S | A forced panic in the TUI leaves the terminal usable |
| Atomic save with backup for rc files and `config.toml` (audit C4) | `operations::save_dirty_files`, `Config::save` use `std::fs::write` | 2026-09-29 | Critical | M | Temp+rename in same dir; symlink target preserved; `.bak` kept |
| Structured entries lose indentation and `\r` (audit H1, H2) | `Entry::new(.., trimmed)` in `bash/mod.rs`, `pwsh/mod.rs`; `save_dirty_files` writes `\n` | 2026-09-29 | High | M | Indented and CRLF files round-trip byte-exact |
| `run_edit_file` discards unsaved edits (audit H3) | `TuiApp::run_edit_file` has no `dirty` check | 2026-09-29 | High | S | Dirty file prompts before opening `$EDITOR` |
| Non-UTF-8 profile aborts startup (audit H4) | `append_resolved_pattern` `read_to_string(&path)?` | 2026-09-29 | High | M | UTF-16 BOM decoded; other invalid files skipped with a warning naming the path |
| File move reorders config patterns by resolved index (audit H5) | `TuiApp::execute_file_move` | 2026-09-29 | High | M | Moving a file inside a dir/glob group leaves `config.toml` patterns correct; tree consistent |
| Undo/redo stack corruption (audit H6) | `reload_profile` keeps `redo_stack`; delete-confirm pushes undo before confirm | 2026-09-29 | High | S | Cancelling delete keeps redo; file-set change clears both stacks |
| CI gate + permissions for release workflow (audit H7) | `.github/workflows/release.yml` | 2026-09-29 | High | S | test+clippy job required before build; no `actions: write`; `cross` pinned; version gate |
| `cargo audit` findings (audit H8, L3) | `time` unused (RUSTSEC-2026-0009); `anyhow` 1.0.100; `lru`/`paste` via ratatui 0.26; unused `colored`, `terminal_size`, `thiserror` | 2026-09-29 | High | S (L for ratatui) | `cargo audit` clean or remaining items listed as Blocked |
| Test suite repair (audit T1–T4) | `tests/integration/` never compiled; `tui_logic_tests` covers dead `cut_entries`/`paste_entries`; no CRLF/BOM/empty round-trip tests | 2026-09-29 | Medium | M | Round-trip fixture test over CRLF/BOM/empty/indented/unclosed inputs; dead tests removed or ported |
| Remaining audit Medium/Low items (M1–M7, L1–L8) | [audit](../audit/2026-09-29-code-audit.md) | 2026-09-29 | Medium | L | Each item fixed or moved to Blocked with a reason |

## Done

| Item | Commit |
|---|---|

## Blocked / not scheduled

None.
