# Parser Flow Overview

## Core Concept

**First delimit the boundary of each entry, then produce the entry.**

All lines pass through a unified `PendingBlock` state machine before becoming `Entry` objects.

## State Machine Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Main Parse Loop                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  for each line:                                             │
│    ┌─────────────────┐                                      │
│    │  active_block?  │──yes──► accumulate + check boundary  │
│    └────────┬────────┘              │                       │
│             no                      ▼                       │
│             │              ┌────────────────┐               │
│             │              │   complete?    │               │
│             │              └───────┬────────┘               │
│             │                 yes  │  no                    │
│             │                  ▼   │   │                    │
│             │         build Entry  │   └──► continue        │
│             │                      │                        │
│             ▼                      │                        │
│    ┌─────────────────┐             │                        │
│    │ detect new block│◄────────────┘                        │
│    └────────┬────────┘                                      │
│             │                                               │
│             ▼                                               │
│    ┌─────────────────┐                                      │
│    │ pending_entry?  │──► merge or flush Comment/Code       │
│    └─────────────────┘                                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Two-Level Pending State

| State | Purpose | Block Types |
|-------|---------|-------------|
| `active_block` | Multi-line structures with explicit boundaries | Function, Control, Alias, EnvVar |
| `pending_entry` | Adjacent line merging | Comment, BlankLines, CodeWithBlanks |

## Boundary Types

| BoundaryType | Detection | Completion |
|--------------|-----------|------------|
| `Complete` | Single-line entry | Immediate |
| `BraceCounting` | `func() {` | `brace_count == 0` |
| `QuoteCounting` | Odd single quotes | Even quote count |
| `KeywordTracking` | `if`/`while`/`for`/`case` | `fi`/`done`/`esac` (depth=0) |
| `AdjacentMerging` | Comment/blank line | Non-matching line encountered |

## Parse Priority (Bash)

```
1. Active block accumulation (if active_block exists)
2. Control structure detection (if/while/for/case)
3. Empty line handling (merge into pending_entry)
4. Comment line handling (merge into pending_entry)
5. Structured entries:
   - Alias (single-line or multi-line start)
   - Export/EnvVar (single-line or multi-line start)
   - Source statement
   - Function definition
6. Fallback: Code entry
```

## Entry Type Flow

```
┌────────────────────────────────────────────────────────────┐
│                     Line Classification                     │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  alias name='value'  ──────────────────────► Alias         │
│  alias name='start   ──► QuoteCounting ────► Alias         │
│                                                            │
│  export VAR=value    ──────────────────────► EnvVar        │
│  export VAR='start   ──► QuoteCounting ────► EnvVar        │
│                                                            │
│  source ~/.file      ──────────────────────► Source        │
│  . ~/.file           ──────────────────────► Source        │
│                                                            │
│  func() {            ──► BraceCounting ────► Function      │
│  function name {     ──► BraceCounting ────► Function      │
│                                                            │
│  if/while/for/case   ──► KeywordTracking ──► Code          │
│                                                            │
│  # comment           ──► AdjacentMerging ──► Comment/Code  │
│  (blank line)        ──► AdjacentMerging ──► Code          │
│  other code          ──► AdjacentMerging ──► Code          │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

## Comment/Code Merge Rules

| Scenario | Result |
|----------|--------|
| Comment + Comment | Comment (merged) |
| Comment + blank | Comment (absorbs blank) |
| Comment + non-blank Code | **Code** (type upgrade) |
| Comment + Code + blank | Code (absorbs trailing blank) |
| blank + blank | Code (empty, merged) |
| non-blank Code + blank | Code (absorbs trailing blank) |
| blank + non-blank Code | **Separate entries** |

## Key Data Structures

### PendingBlock

```rust
pub struct PendingBlock {
    pub lines: Vec<String>,      // Accumulated raw lines
    pub start_line: usize,       // Starting line number (1-indexed)
    pub end_line: usize,         // Current ending line number
    pub boundary: BoundaryType,  // How to detect completion
    pub entry_hint: Option<EntryType>,  // Expected entry type
    pub name: Option<String>,    // Entry name (for Alias/Function/EnvVar)
    pub value: Option<String>,   // Extracted value
}
```

### BoundaryType

```rust
pub enum BoundaryType {
    Complete,
    BraceCounting { brace_count: i32 },
    QuoteCounting { quote_count: usize },
    KeywordTracking { depth: usize },
    AdjacentMerging { merge_type: MergeType },
}
```

## Files

| File | Purpose |
|------|---------|
| `src/parser/pending.rs` | PendingBlock state machine |
| `src/parser/bash/mod.rs` | Bash parser implementation |
| `src/parser/pwsh/mod.rs` | PowerShell parser implementation |
| `src/parser/bash/parsers.rs` | Entry-specific parsing (alias, export, etc.) |
| `src/parser/bash/control.rs` | Control structure keyword detection |
| `src/parser/bash/patterns.rs` | Regex patterns |
