# Backlog

Status: In progress

The single living tracker for open follow-ups. Rows move to **Done** with the closing commit; never deleted.

## Open

| Item | Evidence | Verified | Priority | Effort | Acceptance |
|---|---|---|---|---|---|
| Rewrite `PENDING_STATE_MACHINE.md` in English against current code | `docs/reference/PENDING_STATE_MACHINE.md` §2 and §5 cite `CodeBlockBuilder`, which no longer exists (`src/parser/builders/` has `CommentBlockBuilder`, `QuotedValueBuilder`) | 2026-09-29 | Low | M | Every symbol named in the doc exists in `src/` |
| Verify `PARSER_FLOW.md` against current parsers | `docs/reference/PARSER_FLOW.md`, `PendingBlock`, `BoundaryType` | — | Low | S | Claims checked; doc covers PowerShell as well as Bash |
| Add missing `LICENSE` file | `README.md` links `LICENSE`; `Cargo.toml` declares `license = "MIT"`; no file in repo | 2026-09-29 | Medium | S | `LICENSE` (MIT) exists at repo root; README link resolves |

## Done

| Item | Commit |
|---|---|

## Blocked / not scheduled

None.
