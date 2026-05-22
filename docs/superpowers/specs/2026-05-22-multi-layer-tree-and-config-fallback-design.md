# Multi-Layer TUI Tree, Config Fallback, and Release Bundling — Design Spec

**Date:** 2026-05-22
**Status:** Draft (pending review)
**Scope:** wenv v0.17 — breaking changes

---

## §1. Scope and Non-Goals

### In scope
1. **CLI** — remove `-c/--config` flag; add `wenv config` subcommand that opens the active config file in `$EDITOR`.
2. **Config loader** — drop the hardcoded `~/.config/wenv/config.toml` path; introduce an OS-conditional fallback search chain; when no file exists in any fallback, create a default config at the first writable fallback location.
3. **Cache file (new)** — introduce a new `cache.toml` sibling-to-`config.toml` to persist resolved PowerShell `$PROFILE` paths so subsequent runs skip the `pwsh -NoProfile` shell-out. Today this path is queried live every run via `query_powershell_profile()`; there is no existing `[cache]` field on `Config` to migrate. The new cache lives in the same directory as the resolved config.
4. **Resources/config.toml** — checked into repo root; serves both as the runtime first-search location (when binary is launched from a directory containing `Resources/config.toml`) and as the release tarball template.
5. **TUI three-layer tree** — `DirGroup → ProfileFile → Entry`, replacing the existing two-layer `ProfileFile → Entry` model.
6. **Startup collapsed** — all dirs and files start collapsed; no auto-expand based on file existence.
7. **Variable resolution display** — when a path pattern contains an env var, render it as `<resolved> (<original-pattern>)`.
8. **Release workflow** — bundle `Resources/config.toml` alongside the binary into a versioned archive.

### Out of scope (explicitly excluded)
- Migration from `~/.config/wenv/config.toml` to the new fallback chain.
- Migration from `.path_cache.toml` to `cache.toml`.
- Multiple `wenv config` subcommands (`show`/`path`/`edit`) — only the single bare `wenv config` is implemented.
- Relocating i18n language files.
- Signing release archives or producing additional checksums beyond what the current workflow does.

---

## §2. Data Model Changes

### 2.1 `ProfileFile` (unchanged structurally)
Same fields as today. The `expanded` field's default initialization changes (see §6.1).

### 2.2 New `DirGroup` (in `src/model/profile.rs`)

```rust
pub struct DirGroup {
    /// Original config pattern; used as the key when removing via TUI `d`.
    /// Examples: "~/.zshrc.d/*", "$ZDOTDIR/*", "/etc/profile.d/*.sh", "$ZDOTDIR"
    pub source_pattern: String,
    /// Display label. When the source contains `$VAR` or `%VAR%`,
    /// formatted as "<resolved-form> (<original-with-vars>)";
    /// otherwise the tilde-collapsed expanded form.
    pub display_label: String,
    /// Indices into `ShellProfile.files`, sorted by path alphabetically.
    pub file_indices: Vec<usize>,
    /// Default `false`.
    pub expanded: bool,
}
```

### 2.3 `ShellProfile` extension

```rust
pub struct ShellProfile {
    pub shell_type: ShellType,
    pub files: Vec<ProfileFile>,   // Linear; existing operations unchanged.
    pub tree: Vec<TreeNode>,        // Top-level topology.
}

pub enum TreeNode {
    Dir(DirGroup),
    File(usize), // index into ShellProfile.files
}
```

`files` remains a flat vector so existing cut/paste/undo/`file_index` semantics are preserved. `tree` only describes the top-level layout; `DirGroup` and single files coexist as siblings.

### 2.4 `ListItem` (replaces existing two-variant enum)

```rust
pub enum ListItem {
    DirHeader(usize),          // index into ShellProfile.tree (must be Dir)
    FileHeader(usize),         // index into ShellProfile.files
    Entry(usize, usize),       // (file_index, entry_index)
}
```

### 2.5 `build_visible_list` semantics

```text
for each tree node:
  Dir(group):
    push DirHeader(group_idx)
    if group.expanded:
      for fi in group.file_indices:
        push FileHeader(fi)
        if files[fi].expanded:
          for ei in 0..entries.len(): push Entry(fi, ei)
  File(fi):
    push FileHeader(fi)
    if files[fi].expanded:
      for ei in 0..entries.len(): push Entry(fi, ei)
```

### 2.6 `ShellProfile::toggle_all(expanded: bool)`
Sets `expanded` on every `DirGroup` and every `ProfileFile` to the given value. Called with `false` after `load_shell_profile` to enforce startup-collapsed regardless of file existence.

---

## §3. Path Resolver Refactor

### 3.1 New API (`src/config/path_resolver.rs`)

Replace `resolve_paths()` with:

```rust
pub enum ResolvedPattern {
    File {
        original: String,                // raw config string (key)
        display: String,                 // tilde-collapsed; with "(varname)" suffix if var-bearing
        path: PathBuf,
        exists: bool,
    },
    Dir {
        original: String,                // raw config string (key)
        display: String,                 // same rules as File.display
        files: Vec<(PathBuf, bool)>,     // alphabetized, binary-filtered
    },
}

pub fn resolve_patterns(patterns: &[String]) -> Vec<ResolvedPattern>;
```

### 3.2 Classification rules

For each `pattern`:

1. `expand_tilde` → `expand_env_vars` → `expanded`.
2. If `expanded` still matches `has_unresolved_vars` → `eprintln!` warning, skip.
3. If `expanded.trim().is_empty()` → warning, skip.
4. **If `expanded` contains `*` or `?`** (glob pattern):
   - Resolve with `glob::glob(&expanded)`, apply binary filter, sort by path.
   - Emit `Dir { files }`. Empty `files` is permitted (so the user still sees the pattern in the TUI).
5. **Else** try `std::fs::metadata(&expanded)`:
   - Directory → equivalent to `<expanded>/*`: `read_dir` → keep `is_file()` only → binary filter → sort by path → `Dir`.
   - File / not found → `File`.

### 3.3 Display label rules

- If original pattern matches `\$[A-Za-z_][A-Za-z0-9_]*` or `%[A-Za-z_][A-Za-z0-9_]*%` (var-bearing — same regex as `expand_env_vars` at `src/config/path_resolver.rs:18`):
  - `display = format!("{} ({})", expanded_form_with_tilde, original_with_vars)`
  - Example: `$ZDOTDIR/*` → `~/.zsh/* ($ZDOTDIR/*)`
- Else: `display = tilde_collapse(expanded)`. `~` collapse uses the same logic as `ProfileFile::display_name`.

### 3.4 Binary / non-text filter

New helper in `src/utils/path.rs`:

```rust
pub fn is_likely_text(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return true; };
    let mut buf = [0u8; 8192];
    let n = f.read(&mut buf).unwrap_or(0);
    !buf[..n].contains(&0)
}
```

Applied **only** during dir expansion (`ResolvedPattern::Dir.files`). Single-file patterns (`ResolvedPattern::File`) bypass the filter — if the user explicitly named the file, render it regardless. Non-existent paths return `true` so first-run "create this file?" flow remains intact.

**Accepted tradeoff:** the 8 KiB probe is sufficient for shell-config files in practice — binaries that might end up in `profile.d/` (ELF, Mach-O, PE) all contain null bytes in their first block. Files with a text header followed by binary content past 8 KiB will not be filtered; this is acceptable given the cost of a full scan per dir entry.

### 3.5 Old API removal

- `resolve_paths()` is deleted; all callers move to `resolve_patterns()`.
- `expand_tilde`, `expand_env_vars`, `has_unresolved_vars` remain `pub` — used elsewhere.

---

## §4. Config Loading / Saving

### 4.1 Fallback paths (`src/model/config.rs`)

```rust
impl Config {
    fn fallback_paths() -> Vec<PathBuf> {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        let home = dirs::home_dir();

        #[cfg(not(target_os = "windows"))]
        {
            let mut v = Vec::new();
            if let Some(d) = &exe_dir { v.push(d.join("Resources/config.toml")); }
            if let Some(h) = &home {
                v.push(h.join(".wenget/apps/wenv/Resources/config.toml"));
                v.push(h.join(".local/bin/Resources/config.toml"));
            }
            v.push(PathBuf::from("/opt/wenget/apps/wenv/Resources/config.toml"));
            v.push(PathBuf::from("/usr/local/bin/config/config.toml"));
            v
        }

        #[cfg(target_os = "windows")]
        {
            let env = |k: &str| std::env::var(k).ok().map(PathBuf::from);
            let mut v = Vec::new();
            if let Some(d) = &exe_dir { v.push(d.join("Resources").join("config.toml")); }
            if let Some(p) = env("USERPROFILE")  { v.push(p.join(".wenget/apps/wenv/Resources/config.toml")); }
            if let Some(p) = env("LOCALAPPDATA") { v.push(p.join("Programs/wenv/Resources/config.toml")); }
            if let Some(p) = env("ProgramW6432") { v.push(p.join("wenget/apps/wenv/Resources/config.toml")); }
            if let Some(p) = env("ProgramFiles") { v.push(p.join("gpinstall/Resources/config.toml")); }
            v
        }
    }
}
```

### 4.2 `Config` runtime field

```rust
#[derive(Serialize, Deserialize, ...)]
pub struct Config {
    // existing: ui, files, snippets, template_paths, ...
    #[serde(skip)]
    pub source_path: PathBuf,
}
```

### 4.3 `resolve_or_create()` flow

```rust
impl Config {
    pub fn resolve_or_create(shell_key: &str) -> anyhow::Result<Self> {
        // Phase 1: existing file
        for p in Self::fallback_paths() {
            if p.exists() {
                let content = std::fs::read_to_string(&p)?;
                let mut cfg: Config = toml::from_str(&content)?;
                cfg.source_path = p;
                return Ok(cfg);
            }
        }
        // Phase 2: create default at first writable location.
        // Construct Config directly (skip parse round-trip) and serialize once for disk write.
        for p in Self::fallback_paths() {
            let Some(parent) = p.parent() else { continue };
            if std::fs::create_dir_all(parent).is_err() { continue; }
            let mut cfg = Config::default();
            if let Some(paths) = templates::default_paths(shell_key) {
                cfg.files.insert(shell_key.to_string(), FilesConfig { paths });
            }
            let serialized = toml::to_string_pretty(&cfg)?;
            if std::fs::write(&p, &serialized).is_ok() {
                cfg.source_path = p.clone();
                eprintln!("✓ Created default config at: {}", p.display());
                return Ok(cfg);
            }
        }
        anyhow::bail!("No writable config location among fallback paths")
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.source_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.source_path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
```

### 4.4 Cache file (`src/config/cache.rs`, new)

`cache.toml` lives **next to the resolved `config.toml`** — i.e., in `cfg.source_path.parent()`. This guarantees config and cache always sit together; the cache is not an independent search.

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct Cache {
    pub pwsh_profile: Option<String>,
    pub powershell_profile: Option<String>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl Cache {
    /// Derive cache path from the resolved config path.
    pub fn cache_path_for(config: &Config) -> PathBuf {
        config.source_path
            .parent()
            .map(|p| p.join("cache.toml"))
            .unwrap_or_else(|| PathBuf::from("cache.toml"))
    }
    pub fn load_or_default(config: &Config) -> Self {
        let p = Self::cache_path_for(config);
        let mut cache: Cache = if p.exists() {
            std::fs::read_to_string(&p).ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Cache::default()
        };
        cache.source_path = p;
        cache
    }
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.source_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.source_path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
```

If `config.source_path.parent()` is read-only, `Cache::save()` returns `Err`; callers log a warning and continue (PowerShell `$PROFILE` falls back to live query).

### 4.5 Removals / redirections

- `Config::config_dir()` — deleted.
- `Config::config_path()` — deleted (callers use `cfg.source_path`).
- `config::ensure_config_dir()` — deleted (`resolve_or_create` handles it).
- `config::load_or_create_config()` — replaced by `Config::resolve_or_create(shell_key)`.
- (No existing `[cache]` field on `Config` to remove — feature is new.)
- (No legacy `.path_cache.toml` migration code in the current codebase — nothing to delete.)

### 4.6 Accepted risk — shadowing pre-existing user configs

Putting `<exe_dir>/Resources/config.toml` first in the fallback chain means a user with an existing `~/.config/wenv/config.toml` who installs the new tarball will have their old config silently shadowed by the bundled default (since the release tarball ships `Resources/config.toml`). The old file is **not** read, **not** migrated, and **not** deleted.

This is the intended behavior:
- The user explicitly chose no migration (see decisions captured in §1 Out of scope).
- The bundled `Resources/config.toml` is the source of truth for tarball installations.
- Users who want to preserve prior customization must manually copy fields from their old `~/.config/wenv/config.toml` into the new fallback location, then delete the old file.

This is called out in CHANGELOG as a breaking change.

### 4.7 Failure modes

| Situation | Behavior |
|---|---|
| All fallbacks absent and all unwritable | `anyhow::bail!`, stderr lists attempted paths |
| Config exists but `toml::from_str` fails | Propagate `anyhow::Error`; do not overwrite |
| Cache unwritable | `eprintln!` warning; PowerShell `$PROFILE` falls back to live query every run (non-fatal) |

---

## §5. CLI Refactor

### 5.1 `src/cli/args.rs`

```rust
#[derive(Parser)]
#[command(name = "wenv")]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub shell: Option<ShellArg>,

    /// Open source file in $EDITOR (same as "wenv .")
    #[arg(long)]
    pub source: bool,

    /// "." to open editor
    #[arg(value_name = "COMMAND")]
    pub command: Option<String>,

    #[command(subcommand)]
    pub subcommand: Option<SubCmd>,
}

#[derive(clap::Subcommand)]
pub enum SubCmd {
    /// Open wenv config file in $EDITOR
    Config,
}
```

- `-c, --config` flag is removed entirely.
- `--shell` becomes `global = true` so `wenv --shell zsh config` is accepted.

### 5.2 `src/main.rs`

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();

    if matches!(cli.subcommand, Some(SubCmd::Config)) {
        let shell_type = get_shell_type(cli.shell.map(Into::into), None);
        let cfg = Config::resolve_or_create(shell_type.config_key())?;
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
            if cfg!(windows) { "notepad".into() } else { "vi".into() }
        });
        std::process::Command::new(&editor).arg(&cfg.source_path).status()?;
        return Ok(());
    }

    // Otherwise: existing flow, using Config::resolve_or_create in place of load_or_create_config
}
```

### 5.3 Breaking change

| Old | New |
|---|---|
| `wenv -c` / `wenv --config` | `wenv config` |

No deprecation alias. CHANGELOG records this as breaking.

---

## §6. TUI Changes

### 6.1 Startup-collapsed default
`load_shell_profile` sets `pf.expanded = false` (replacing the `expanded = exists` line at the current `src/model/profile.rs:169`). `DirGroup.expanded` initializes to `false`. No code path auto-expands at startup.

### 6.2 Key semantics (additions / clarifications)

| Key | On DirHeader | On FileHeader | On Entry |
|---|---|---|---|
| `Enter`/`Space` | toggle `dir.expanded` | toggle `file.expanded` (existing) | no-op |
| `9` | `toggle_all(true)` | same | same |
| `0` | `toggle_all(false)` | same | same |
| `d` | confirm → remove `source_pattern` from `config.files.{shell}.paths` → save → reload | existing (remove single path) | existing (delete entry) |
| `a` | existing (prompt for arbitrary string; accepts file/dir/glob/`$VAR`) | same | same |
| `m` | not supported — show message "Move mode not supported on DirHeader" | existing (top-level file move only; see §6.7) | existing |
| `e` | no-op + status hint | existing | existing |
| `s` (select toggle) | no-op | existing | existing |
| `x`/`c`/`v` | no-op | existing | existing |
| `r` (remark) | no-op | no-op | existing |
| `w`, `q`, `?`, `Esc`, `/`, `n`, `z` | unchanged behavior |

### 6.3 `a` key prompt text
Update the dialoguer prompt to:
```
Add path to config (file, directory, glob, or $VAR):
```
After accepting input: append raw string to `config.files.{shell_key}.paths`, `config.save()`, reload profile (rebuild `tree` + `files`). Best-effort `expanded` state migration: match files by path; new files default to collapsed.

### 6.4 `d` on DirHeader
```text
1. dialoguer Confirm:
   "Remove pattern '{source_pattern}' from config? ({N} files will be hidden)"
2. If confirmed:
   a. config.files.{shell_key}.paths.retain(|p| p != source_pattern);
   b. config.save();
   c. reload profile (rebuild tree).
3. If cancelled: no-op.
```
The removal key is the *raw* pattern string, not the expanded path.

### 6.5 List rendering (`src/tui/list.rs`, `src/tui/ui.rs`)

Indentation:
- Top-level `DirHeader` / `FileHeader`: column 0.
- `FileHeader` inside a `DirGroup`: indent 2.
- `Entry`: indent 2 (top-level file) or 4 (inside dir).

Symbols:
- Expanded: `▼`. Collapsed: `▶`. (Same glyphs as existing FileHeader.)

DirHeader format:
```
▶ ~/.zshrc.d/*                                    [3 files]
▼ ~/.zsh/* ($ZDOTDIR/*)                           [5 files]
```

FileHeader inside dir:
```
  ▶ ~/.zshrc.d/01-aliases.sh                      [12 entries]
  ▼ ~/.zsh/path.sh ($ZDOTDIR/path.sh)             [3 entries]
```

Top-level FileHeader:
```
▶ ~/.zshrc                                        [42 entries]
```

`[N files]` and `[N entries]` are right-aligned and truncated if terminal width is insufficient.

### 6.6 Filter / search interaction
- When filter is active, a file matching the query forces both that file and its parent `DirGroup` (if any) to render as expanded.
- A `DirGroup` with zero matching descendants is hidden from the filtered list.
- Existing `saved_expanded` mechanism is extended to record both `DirGroup.expanded` and `ProfileFile.expanded`, restored on filter exit.

### 6.7 Move file mode
- `m` on a top-level `File` node: existing behavior.
- `m` on a `FileHeader` inside a `DirGroup`: not supported. Show status message: "Files inside a directory group are sorted alphabetically; move is not supported".
- Files within a `DirGroup` are always sorted alphabetically by path; order cannot be customized.
- `tree` is rebuilt on reload using the same alphabetical rule, so move operations on top-level files do not affect dir-internal ordering.

---

## §7. Release Workflow Changes

### 7.1 Tarball layout

```
wenv-vX.Y.Z-<target>/
├── wenv (or wenv.exe)
└── Resources/
    └── config.toml
```

- Linux / macOS: `.tar.gz`
- Windows: `.zip`

### 7.2 Workflow modification (sketch — exact YAML deferred to plan)

In each matrix build job, between `cargo build --release` and the archive/upload step, insert a "stage" step:

```yaml
- name: Stage artifacts
  shell: bash
  run: |
    STAGE="wenv-${{ github.ref_name }}-${{ matrix.target }}"
    mkdir -p "$STAGE/Resources"
    if [[ "${{ matrix.os }}" == "windows-latest" ]]; then
      cp "target/${{ matrix.target }}/release/wenv.exe" "$STAGE/"
    else
      cp "target/${{ matrix.target }}/release/wenv" "$STAGE/"
    fi
    cp Resources/config.toml "$STAGE/Resources/config.toml"
    echo "STAGE=$STAGE" >> "$GITHUB_ENV"

- name: Archive (Unix)
  if: matrix.os != 'windows-latest'
  run: tar -czf "${STAGE}.tar.gz" "$STAGE"

- name: Archive (Windows)
  if: matrix.os == 'windows-latest'
  shell: pwsh
  run: Compress-Archive -Path $env:STAGE -DestinationPath "$env:STAGE.zip"
```

Plan phase: reconcile against `/Volumes/Home/Users/wen/repos/agd/.github/workflows/release.yml` patterns and existing wenv strip/upload steps.

### 7.3 Repo addition — `Resources/config.toml`

- Content: output of `templates::generate_default_config("zsh")` (POSIX-shell defaults; `[ui] language="en"`; zsh path list).
- README gets a short paragraph explaining the file's dual role (runtime first-search location + release template).
- Not gitignored; committed.

### 7.4 Not in this change
- No signing.
- No additional checksums beyond existing workflow output (if any).
- No separate `<binary>-config.tar.gz` archive.

---

## §8. Testing Strategy

### 8.1 Unit tests (new / rewritten)

**`src/config/path_resolver.rs`**
- Single-file pattern → `File` variant.
- Glob pattern with matches → `Dir` variant.
- Glob pattern with no matches → `Dir` with empty files (header still appears).
- Pattern resolving to existing directory → `Dir` enumerating contents.
- Binary-filter behavior: temp file with leading `\0` is skipped; pure-text file kept.
- Var-bearing pattern → `display` contains `(varname)` suffix.
- Non-var pattern → `display` is tilde-collapsed.
- Unresolved variable → warning + skip; absent from output.

**`src/model/config.rs`**
- `fallback_paths()` content on Unix and Windows (use `temp_env` or env var overrides).
- `resolve_or_create` under tempdir HOME:
  - Existing file → loads, `source_path` correct.
  - No existing file → creates default at first writable fallback.
  - All fallbacks unwritable → `Err`.
- `save()` writes to `source_path`.

**`src/model/profile.rs`**
- `build_visible_list` ordering across dir-collapsed / dir-expanded / fully-expanded.
- Top-level `File` and `Dir` interleaved.
- `toggle_all(true)` / `(false)` flips both dir and file `expanded`.

**`src/utils/path.rs`**
- `is_likely_text` on: file with null byte, pure text, empty file, non-existent path.

### 8.2 Integration tests

`tests/three_layer_tree.rs`
- Tempdir with `<td>/.zshrc`, `<td>/zshrc.d/a.sh`, `<td>/zshrc.d/b.sh`, `<td>/zshrc.d/bin.dat` (contains `\0`).
- Config `files.zsh = ["<td>/.zshrc", "<td>/zshrc.d/*"]`.
- After `load_shell_profile`: `tree = [File, Dir(a.sh, b.sh)]`; `bin.dat` absent.

`tests/config_fallback.rs`
- Mock HOME + env vars + exe path; assert `resolve_or_create` chooses expected file.

### 8.3 Manual checklist (run after Task 4 / Task 5 / Task 6)

- macOS: `cargo run` → TUI fully collapsed; `9` expands all; `0` collapses all.
- `~/.zshrc.d/*` pattern: DirHeader shows raw pattern; expansion lists matched files.
- `$PROFILE` (pwsh): display shows `<resolved> ($PROFILE)`.
- `wenv config` opens `$EDITOR` on the active config; edits round-trip on next `wenv` run.
- Wipe all fallback paths + make them unwritable → `wenv` reports "No writable config location" and exits.
- `m` on dir-internal file → shows unsupported message; `m` on top-level file works.
- `d` on DirHeader → confirm dialog → pattern removed → tree updates.
- Release: local build + manual archive command produces correct layout.

### 8.4 Out of scope for tests
- TUI interaction end-to-end (no framework; covered by logic tests).
- CI dry-run of release workflow (requires tag push).

---

## §9. Implementation Order and Milestones

Six tasks; each maintains a green `cargo build` + `cargo test`.

### Task 1 — Path resolver refactor (foundation)
- Add `ResolvedPattern` and `resolve_patterns()`.
- Add `utils::path::is_likely_text`.
- Keep `resolve_paths()` as a thin shim wrapping `resolve_patterns()` and flattening so `load_shell_profile` still compiles.
- Unit tests per §8.1 path_resolver.

### Task 2 — Config fallback chain
- Add `source_path` to `Config`.
- Add `Config::fallback_paths()` and `Config::resolve_or_create()`.
- Rewrite `Config::save()` to use `source_path`.
- Remove `config_dir()` / `config_path()` (no existing cache or legacy migration code to delete).
- Add `src/config/cache.rs` with `Cache::cache_path_for(config)` / `load_or_default(config)` / `save`.
- Update PowerShell `$PROFILE` caching to use the new `Cache`.
- Add `Resources/config.toml` to repo.
- Integration tests per §8.2 config_fallback.

### Task 3 — Three-layer profile model
- Add `DirGroup`, `TreeNode`; extend `ShellProfile` with `tree`.
- Update `ListItem` to three variants.
- Rewrite `build_visible_list` / `toggle_all`.
- Rewrite `load_shell_profile` to consume `resolve_patterns()` and build `tree`, with `expanded = false` everywhere.
- Delete the path_resolver shim from Task 1.
- Tests per §8.1 profile + §8.2 three_layer_tree.

### Task 4 — TUI key bindings and rendering
- Extend `app.rs` event handlers for `DirHeader` (toggle, `d`, `a` text, `m` reject).
- Extend `list.rs` / `ui.rs` for three-layer indentation and `[N files]` / `[N entries]` rendering.
- Extend filter behavior + `saved_expanded` to track dir state.
- Run manual checklist subset (TUI items).

### Task 5 — CLI subcommand
- Update `args.rs` to `Subcommand` form.
- Update `main.rs` early branch for `wenv config`.
- Remove `cli.config` bool.
- Update `--help` output and CHANGELOG.

### Task 6 — Release workflow + Resources
- Edit `.github/workflows/release.yml`: insert staging steps and switch archive command.
- Update README with tarball layout note.
- CHANGELOG breaking-change entry.

### Dependency graph

```
T1 ──► T3 ──► T4
        │
T2 ─────┤
        │
T5 ─────┘   (T5 may run in parallel with T3/T4)

T6 — independent; sequence last
```

### Done criteria
- `cargo test` green.
- `cargo clippy` clean.
- Manual checklist (§8.3) completed.
- `CHANGELOG.md` Unreleased section lists three breaking changes:
  1. `-c/--config` removed → use `wenv config`.
  2. Config location moved from `~/.config/wenv/config.toml` to OS-conditional fallback chain (no migration).
  3. TUI startup default: all dirs and files collapsed.

---

## Appendix A — Reference paths (resolved)

**Linux / macOS fallback order:**
1. `<binary_dir>/Resources/config.toml`
2. `$HOME/.wenget/apps/wenv/Resources/config.toml`
3. `$HOME/.local/bin/Resources/config.toml`
4. `/opt/wenget/apps/wenv/Resources/config.toml`
5. `/usr/local/bin/config/config.toml`

**Windows fallback order:**
1. `<binary_dir>\Resources\config.toml`
2. `%USERPROFILE%\.wenget\apps\wenv\Resources\config.toml`
3. `%LOCALAPPDATA%\Programs\wenv\Resources\config.toml`
4. `%ProgramW6432%\wenget\apps\wenv\Resources\config.toml`
5. `%ProgramFiles%\gpinstall\Resources\config.toml`

Cache file (`cache.toml`) lives in the *parent directory* of the first writable entry in the same fallback chain.
