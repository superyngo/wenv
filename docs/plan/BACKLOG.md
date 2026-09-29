# Backlog

Status: In progress

The single living tracker for open follow-ups. Rows move to **Done** with the closing commit; never deleted.

## Open

| Item | Evidence | Verified | Priority | Effort | Acceptance |
|---|---|---|---|---|---|
| Rewrite `PENDING_STATE_MACHINE.md` in English against current code | `docs/reference/PENDING_STATE_MACHINE.md` §2 and §5 cite `CodeBlockBuilder`, which no longer exists (`src/parser/builders/` has `CommentBlockBuilder`, `QuotedValueBuilder`) | 2026-09-29 | Low | M | Every symbol named in the doc exists in `src/` |
| Verify `PARSER_FLOW.md` against current parsers | `docs/reference/PARSER_FLOW.md`, `PendingBlock`, `BoundaryType` | — | Low | S | Claims checked; doc covers PowerShell as well as Bash |
| Add missing `LICENSE` file | `README.md` links `LICENSE`; `Cargo.toml` declares `license = "MIT"`; no file in repo | 2026-09-29 | Medium | S | `LICENSE` (MIT) exists at repo root; README link resolves |
| Route the remaining hard-coded English TUI strings through `i18n::Messages` (audit M5 remainder) | Only 5 catalog keys are wired (`tui_confirm_delete_title`, `tui_entry_details_title`, `tui_msg_nothing_to_undo`, …); help popup, status messages, prompts are literals in `src/tui/` | 2026-09-29 | Medium | L | Every user-visible string in `src/tui/` comes from the catalog; `zh-TW` covers all keys |

## Done

| Item | Commit |
|---|---|
| Unclosed block at EOF kept as `Code`; `strip_quotes` lone-quote panic; indentation kept on structured entries; empty/BOM/CRLF/final-newline preserved; encoding decode with read-only fallback (audit C1, C2, H1, H2, H4, L1, L2, T3) | `4660a1b` |
| Atomic save with `.bak` for rc files and `config.toml` (audit C4) | `4660a1b` |
| Panic hook, `$EDITOR` arguments, non-TTY guards (audit C3, M3, M4) | `3c4e64f` |
| `run_edit_file` dirty guard, file move by pattern, undo/redo integrity (audit H3, H5, H6) | `4e1a955` |
| Cursor after drop, trash prompt, CJK-safe columns (audit M1, M2, M7) | `8ebd8b5` |
| Text input contract: Emacs shortcuts, Ctrl chords never insert (audit M6) | `ba0e884` |
| i18n per-key fallback, embedded zh-TW, `--lang`/`--no-color` (audit M5 partial, L5) | `63f9f6f` |
| `cargo audit` clean: ratatui 0.30, dropped unused deps, anyhow bump (audit H8, L3) | `4f834f1` |
| Regex statics, dead helpers/tests removed, CLI smoke tests, crate docs (audit L4, L8, T1, T2) | `ce377c0` |
| Release CI gate + least-privilege permissions (audit H7) | `7b492a2` |
| Shared Bash/PowerShell pending-block core (audit L7) | see `git log` "share pending/blank/comment/code" |

## Blocked / not scheduled

| Item | Reason |
|---|---|
| Hermetic env in tests (audit T4) | `set_var` calls use unique variable names (`WENV_TEST_VAR*`) and nothing else reads them, so parallel runs cannot collide; revisit if the tests move to edition 2024 (`set_var` becomes `unsafe`). |
| Field-level undo in text inputs (audit M6 remainder) | Kill/yank, word motion and caret editing are done; per-field undo has no user demand yet. |
