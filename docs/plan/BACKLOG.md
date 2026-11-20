# Backlog
Status: In progress

The single living tracker for open follow-ups. Rows move to **Done** with the closing commit; never deleted.

## Open

| ID | Item | Evidence | Verified | Priority | Effort | Acceptance |
|---|---|---|---|---|---|---|
| B1 | Rewrite `PENDING_STATE_MACHINE.md` in English against current code; fix stale `//!` docs | `docs/reference/PENDING_STATE_MACHINE.md` §2 and §5 cite `CodeBlockBuilder`, which no longer exists (`src/parser/builders/` has `CommentBlockBuilder`, `QuotedValueBuilder`); `src/parser/mod.rs` module docs and `src/parser/bash/control.rs` still name it | 2026-11-19 | Low | M | Every symbol named in the doc and in `src/parser/**` `//!` docs exists in `src/` |
| B2 | Verify `PARSER_FLOW.md` against current parsers | `docs/reference/PARSER_FLOW.md`; `src/parser/pending.rs` `PendingBlock`, `BoundaryType` | 2026-11-20 (symbols exist; claims unchecked) | Low | S | Claims checked; doc covers PowerShell as well as Bash |
| B3 | Add missing `LICENSE` file | `README.md` links `LICENSE`; `Cargo.toml` `license = "MIT"`; no file in repo | 2026-11-20 | Medium | S | `LICENSE` (MIT) exists at repo root; README link resolves |
| B4 | Decide the round-trip audit probe: integration test in `tests/` or discard | `docs/audit/2026-09-29-code-audit.md` §Method cites gitignored `docs/tmp/claude-scratch/audit-probe/` (`roundtrip`), a cargo crate importing `wenv` | 2026-11-20 | Low | S | Either a round-trip test exists in `tests/` and is named in `docs/reference/README.md`, or the user accepts the evidence is gone |
| B5 | Decide on tracked root binary `rust_out` | `rust_out` (Mach-O arm64) tracked at repo root | 2026-11-20 | Low | S | User decision recorded; file untracked or kept intentionally |

## Pending verification

None.

## Awaiting external

None.

## Watching

None.

## Done

| ID | Item | Commit |
|---|---|---|
| D1 | Unclosed block at EOF kept as `Code`; `strip_quotes` lone-quote panic; indentation kept on structured entries; empty/BOM/CRLF/final-newline preserved; encoding decode with read-only fallback (audit C1, C2, H1, H2, H4, L1, L2, T3) | `4660a1b` |
| D2 | Atomic save with `.bak` for rc files and `config.toml` (audit C4) | `4660a1b` |
| D3 | Panic hook, `$EDITOR` arguments, non-TTY guards (audit C3, M3, M4) | `3c4e64f` |
| D4 | `run_edit_file` dirty guard, file move by pattern, undo/redo integrity (audit H3, H5, H6) | `4e1a955` |
| D5 | Cursor after drop, trash prompt, CJK-safe columns (audit M1, M2, M7) | `8ebd8b5` |
| D6 | Text input contract: Emacs shortcuts, Ctrl chords never insert (audit M6) | `ba0e884` |
| D7 | i18n per-key fallback, embedded zh-TW, `--lang`/`--no-color` (audit M5 partial, L5) | `63f9f6f` |
| D8 | `cargo audit` clean: ratatui 0.30, dropped unused deps, anyhow bump (audit H8, L3) | `4f834f1` |
| D9 | Regex statics, dead helpers/tests removed, CLI smoke tests, crate docs (audit L4, L8, T1, T2) | `ce377c0` |
| D10 | Release CI gate + least-privilege permissions (audit H7) | `7b492a2` |
| D11 | Shared Bash/PowerShell pending-block core (audit L7) | `ba5f02b` |
| D12 | Split `tui/app.rs`; `handle_action` is a ~190-line dispatcher (audit L6) | `a5e82a1` |
| D13 | Every user-visible TUI string in the i18n catalog, complete zh-TW (audit M5) | `bbaa628` |
| D14 | Popup panics on tiny terminals (found during audit verification) | `bbaa628` |
| D15 | Per-field `Ctrl+Z` undo in text inputs (audit M6 remainder) | `1c7672a` |
| D16 | Hermetic tests: `set_var` removed from integration tests (audit T4; one unique-name `set_var` remains in `add_env_var_pattern_resolves`, single reader) | `1c7672a` |
