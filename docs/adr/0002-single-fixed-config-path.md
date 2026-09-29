# ADR 0002 — Single Fixed Config Path

**Status:** Implemented (2026-05-25)
**Date:** 2026-05-25 (recorded retroactively 2026-09-29 from commit `350ec70` and CHANGELOG v0.17.0)
**Supersedes:** [0001-config-resolution-strategy.md](0001-config-resolution-strategy.md)

## Context

ADR 0001 introduced an OS-conditional fallback chain for `config.toml`, with copy-up on read-only
save and a `WENV_CONFIG_DIR` override, largely so tarball installs could ship a default
`Resources/config.toml`. Before v0.17.0 shipped, snippets were split out of `config.toml` into a
mandatory bundled `Resources/snippets.toml`, leaving nothing in `config.toml` that needed to ship
with the binary.

## Decision

`config.toml` lives at the single fixed path `~/.config/wenv/config.toml` on all platforms, created
from a template when missing. The only override is the `-c, --config <PATH>` flag. The fallback
chain, copy-up save, and `WENV_CONFIG_DIR` are removed. Bundled, read-only data lives in
`Resources/snippets.toml`, resolved by its own search chain.

## Principle deviation

`wens-dev-principles cli 2` (MUST) asks for XDG / per-platform config directories. This ADR
deliberately deviates: one predictable `~/.config/wenv/config.toml` on every platform (including
Windows and macOS) was chosen over per-OS directories to keep docs, support and dotfile-manager
setups uniform. The deviation is recorded here rather than fixed (audit L8, 2026-09-29).

## Alternatives considered

| Alternative | Rejected because |
|---|---|
| Keep ADR 0001's fallback chain | Its main driver (shipping a default config in the tarball) disappeared once snippets moved out; the chain, copy-up, and env var remained cost without benefit. |

## Consequences

- One predictable config location; `source_path` never shifts mid-session.
- `WENV_CONFIG_DIR` is not a compatibility surface (removed before it was relied on).
- Alternate locations require passing `-c` explicitly.
