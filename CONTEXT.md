# CONTEXT

Documentation entry point for wenv. Start with the [glossary](docs/reference/glossary.md).

| Folder | Holds | Canonical? | Lifecycle |
|---|---|---|---|
| [docs/reference/](docs/reference/README.md) | Current behavior: glossary, architecture, entry model, keymap, parser | Yes | Living |
| [docs/adr/](docs/adr/README.md) | Architecture decisions | Yes (for *why*) | Superseded, never edited |
| [docs/spec/](docs/spec/README.md) | Design specs | No | Frozen once approved |
| [docs/plan/](docs/plan/README.md) | Implementation plans; [BACKLOG.md](docs/plan/BACKLOG.md) is the one live tracker | No | Frozen once shipped (backlog living) |
| [docs/debug/](docs/debug/README.md) | Defect investigations | No | Frozen once resolved |
| [docs/audit/](docs/audit/README.md) | Sweeps and verification runs | No | Frozen once addressed |
| `docs/tmp/` | Scratch (gitignored) | No | Disposable |

Root files: [README.md](README.md) (users), [CHANGELOG.md](CHANGELOG.md), [AGENTS.md](AGENTS.md) (agent conduct).

## Reading order

1. [glossary.md](docs/reference/glossary.md)
2. [ARCHITECTURE.md](docs/reference/ARCHITECTURE.md)
3. [ENTRY_MODEL.md](docs/reference/ENTRY_MODEL.md) — read before touching parsers or `value`
4. [KEYMAP.md](docs/reference/KEYMAP.md)
5. [PARSER_FLOW.md](docs/reference/PARSER_FLOW.md), then [PENDING_STATE_MACHINE.md](docs/reference/PENDING_STATE_MACHINE.md)
6. [docs/adr/](docs/adr/README.md) for why config works the way it does
