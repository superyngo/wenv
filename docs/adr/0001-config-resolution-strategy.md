# ADR 0001 — Config Resolution Strategy

**Status:** Accepted
**Date:** 2026-05-22
**Context spec:** `docs/superpowers/specs/2026-05-22-multi-layer-tree-and-config-fallback-design.md`

## Context

In v0.16 wenv read its config from a single hardcoded path: `~/.config/wenv/config.toml`. v0.17 ships tarball releases that bundle a default `Resources/config.toml` next to the binary, and we need a strategy that:

- Lets a tarball install work out-of-the-box without any prior setup.
- Lets a user with multiple wenv installs (system-wide + per-user) get a predictable result.
- Lets `cargo run` during development not contaminate the user's installed config.
- Keeps `a`/`d`/save operations always-functional, even when the resolved config is on a read-only filesystem.

## Decision

Config is located via an OS-conditional **fallback chain**, searched in order:

**Unix:**
1. `$WENV_CONFIG_DIR/config.toml` (when env var set; dev override)
2. `<exe_dir>/Resources/config.toml`
3. `$HOME/.wenget/apps/wenv/Resources/config.toml`
4. `$HOME/.local/bin/Resources/config.toml`
5. `/opt/wenget/apps/wenv/Resources/config.toml`
6. `/usr/local/bin/config/config.toml`

**Windows:** symmetric chain, see spec §4.1.

**First existing file wins** for read. If none exist, the **first writable location** receives a freshly generated default and becomes `source_path`.

**Copy-up on read-only save:** if `source_path` is read-only when `save()` is called, in-memory config is written to the *next* writable fallback, `source_path` is updated to that location for the rest of the session, and subsequent runs find the new copy first (because it sits earlier in the chain or above the read-only original).

**`WENV_CONFIG_DIR`** is the only env-var surface. Documented in README "Development" section; production users may discover it but the behavior is well-defined (same precedence as any chain entry).

## Alternatives considered

| Alternative | Rejected because |
|---|---|
| Use XDG Base Directory spec only (`$XDG_CONFIG_HOME/wenv/config.toml`) | Doesn't accommodate tarball installs that ship a sibling `Resources/config.toml`; loses zero-setup tarball UX. |
| Migrate `~/.config/wenv/config.toml` from v0.16 location | Explicitly out of scope; user accepted breaking change. |
| Build-time `dev-config` cargo feature | Adds a second build mode; conflates dev vs release in a way `cargo run` users can't toggle without a rebuild. |
| Propagate save errors instead of copy-up | Tarball install at `/opt/...` becomes effectively read-only forever; `a`/`d` always fail; degrades core TUI UX. |
| Probe writability at load time and skip read-only entries | A system-wide read-only config gets silently ignored even when it's the only one present; surprising. |

## Consequences

**Positive**
- Tarball install works zero-config: `Resources/config.toml` ships with the binary; first read finds it; first write copies up.
- Per-user installs override system installs naturally (`$HOME/.wenget/...` precedes `/opt/...`).
- Devs can isolate via `WENV_CONFIG_DIR=$(pwd)/Resources cargo run`.
- A user who runs `wenv config` and edits never has to think about *which* file got opened — it's the one currently resolved.

**Negative**
- An existing `~/.config/wenv/config.toml` from v0.16 is invisible to v0.17. Documented as a breaking change in CHANGELOG.
- Copy-up means `source_path` can shift mid-session; the spec notes this is logged to stderr.
- `WENV_CONFIG_DIR` is a permanent compatibility surface once shipped.

**Verification**
- Integration tests cover all four phases: (1) existing file in chain → load it; (2) no existing file → create at first writable; (3) all read-only → bail; (4) read-only source_path + save → copy-up.
