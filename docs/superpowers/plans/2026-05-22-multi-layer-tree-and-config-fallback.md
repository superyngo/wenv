# Multi-Layer Tree, Config Fallback, and Release Bundling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship wenv v0.17 with a three-layer TUI tree (group → file → entry), an OS-conditional config fallback chain with copy-up save, a `wenv config` subcommand, and release tarballs bundling `Resources/config.toml`.

**Architecture:** Path resolver is extended to emit structured `ResolvedPattern { File | Dir }` records preserving the originating config string. `ShellProfile` gains a top-level `tree: Vec<TreeNode>` describing the new group layer while keeping `files: Vec<ProfileFile>` flat so existing cut/paste/undo continue to work. Config loading walks a fallback chain (env-var override → exe-dir → user dirs → system dirs), creating a default at the first writable location when no file exists; save copies up to a writable fallback when `source_path` becomes read-only. PowerShell `$PROFILE` caching moves to a new sibling `cache.toml`. Release workflow stages each platform binary into `wenv-vX.Y.Z-<target>/{wenv,Resources/config.toml}` before archiving.

**Tech Stack:** Rust (edition 2021), ratatui, dialoguer, anyhow, toml, serde, regex, glob, dirs. Tests use `tempfile`. CI uses GitHub Actions with cross-rs for musl targets.

**Spec:** `docs/superpowers/specs/2026-05-22-multi-layer-tree-and-config-fallback-design.md`
**ADR:** `docs/adr/0001-config-resolution-strategy.md`
**Glossary:** `CONTEXT.md`

---

## File Structure

### New files
- `src/utils/path.rs` — extended with `is_likely_text()` and `is_dir_writable()` helpers (file exists; add to it).
- `src/config/cache.rs` — `Cache` struct, `cache_path_for(&Config)`, `load_or_default(&Config)`, `save()`.
- `Resources/config.toml` — committed default config (zsh defaults).
- `tests/path_resolver_patterns.rs` — `resolve_patterns()` unit-ish integration tests.
- `tests/config_fallback.rs` — fallback chain, copy-up, WENV_CONFIG_DIR coverage.
- `tests/three_layer_tree.rs` — `load_shell_profile` produces correct tree.
- `tests/dedup.rs` — duplicate pattern handling.
- `tests/targeted_reload.rs` — add/remove pattern preserves other dirty files.

### Modified files
- `src/config/path_resolver.rs` — add `ResolvedPattern`, `resolve_patterns()`; keep helpers `pub`.
- `src/model/config.rs` — `source_path: PathBuf`, `fallback_paths()`, `resolve_or_create()`, copy-up `save()`.
- `src/config/mod.rs` — remove `ensure_config_dir`, `load_or_create_config`; re-export `Cache`.
- `src/config/templates.rs` — unchanged (`default_paths` / `default_snippets` still used by Phase 2 build).
- `src/model/profile.rs` — add `DirGroup`, `TreeNode`; extend `ShellProfile` with `tree`; rewrite `build_visible_list`, `toggle_all`, `load_shell_profile`.
- `src/tui/app.rs` — three-layer event handling (DirHeader toggle/d/a/m); targeted patch reload; filter behavior; saved_expanded extension.
- `src/tui/list.rs` — render DirHeader rows.
- `src/tui/ui.rs` — three-layer indentation.
- `src/tui/state.rs` — extend `saved_expanded` to record `(dir_idx_expanded, file_idx_expanded)`.
- `src/cli/args.rs` — `Subcommand` form; remove `-c/--config` bool.
- `src/main.rs` — early `wenv config` branch; use `Config::resolve_or_create`; emit shadow warning.
- `.github/workflows/release.yml` — stage Resources/ + switch Windows to zip.
- `Cargo.toml` — add `tempfile` to `[dev-dependencies]` if absent.
- `CHANGELOG.md` — Unreleased section, three breaking changes.
- `README.md` — Development section noting `WENV_CONFIG_DIR`; tarball layout.

---

## Task 1: Path Resolver Refactor (Foundation)

**Files:**
- Create: `tests/path_resolver_patterns.rs`
- Modify: `src/utils/path.rs` (add helpers at end of file)
- Modify: `src/config/path_resolver.rs` (add `ResolvedPattern` + `resolve_patterns`; keep `resolve_paths` as transitional shim that calls the new function and flattens)

### Setup

- [ ] **Step 1.0: Verify cargo dev deps include tempfile**

Run: `grep -A2 'dev-dependencies' Cargo.toml`

If `tempfile` is absent, add it:

```toml
[dev-dependencies]
tempfile = "3"
```

Commit:
```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add tempfile to dev-dependencies"
```

### `is_likely_text` and `is_dir_writable`

- [ ] **Step 1.1: Write tests for is_likely_text**

Append to `src/utils/path.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn is_likely_text_handles_pure_text() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a.sh");
        std::fs::write(&p, b"#!/bin/sh\necho hi\n").unwrap();
        assert!(is_likely_text(&p));
    }

    #[test]
    fn is_likely_text_rejects_null_byte() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bin.dat");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(&[0x7f, 0x45, 0x4c, 0x46, 0x00, 0x01]).unwrap();
        assert!(!is_likely_text(&p));
    }

    #[test]
    fn is_likely_text_handles_empty_file() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("empty");
        std::fs::write(&p, b"").unwrap();
        assert!(is_likely_text(&p));
    }

    #[test]
    fn is_likely_text_handles_missing_file() {
        let p = std::path::PathBuf::from("/definitely/not/here/x");
        assert!(is_likely_text(&p));
    }

    #[test]
    fn is_dir_writable_true_for_tempdir() {
        let dir = tempdir().unwrap();
        assert!(is_dir_writable(dir.path()));
    }
}
```

- [ ] **Step 1.2: Run tests to verify they fail**

Run: `cargo test --lib utils::path::tests -- --nocapture`
Expected: FAIL — `is_likely_text` / `is_dir_writable` not defined.

- [ ] **Step 1.3: Implement helpers**

Append (before the `#[cfg(test)]` block) to `src/utils/path.rs`:

```rust
/// Returns true if the file at `path` appears to be text (no null bytes
/// in the first 8 KiB). Missing files or unreadable files return true so
/// the dir-expansion filter doesn't accidentally hide pending or transient
/// entries. See spec §3.4 for the accepted tradeoff.
pub fn is_likely_text(path: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return true; };
    let mut buf = [0u8; 8192];
    let n = f.read(&mut buf).unwrap_or(0);
    !buf[..n].contains(&0)
}

/// Probe whether `dir` is writable by attempting to create and delete a
/// unique temporary file. Used by Cache::cache_path_for fallback logic.
pub fn is_dir_writable(dir: &std::path::Path) -> bool {
    if !dir.exists() { return false; }
    let probe = dir.join(format!(".wenv-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true).create_new(true).open(&probe)
    {
        Ok(_) => { let _ = std::fs::remove_file(&probe); true }
        Err(_) => false,
    }
}
```

- [ ] **Step 1.4: Run tests to verify pass**

Run: `cargo test --lib utils::path::tests`
Expected: PASS, 5 tests.

- [ ] **Step 1.5: Commit**

```bash
git add src/utils/path.rs
git commit -m "feat(utils): add is_likely_text and is_dir_writable helpers"
```

### `ResolvedPattern` and `resolve_patterns`

- [ ] **Step 1.6: Write integration test for resolve_patterns — single file**

Create `tests/path_resolver_patterns.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use wenv::config::path_resolver::{resolve_patterns, ResolvedPattern};

#[test]
fn single_existing_file_resolves_to_file_variant() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("a.sh");
    fs::write(&p, b"echo hi\n").unwrap();
    let patterns = vec![p.to_string_lossy().to_string()];
    let out = resolve_patterns(&patterns);
    assert_eq!(out.len(), 1);
    match &out[0] {
        ResolvedPattern::File { path, exists, .. } => {
            assert_eq!(path, &p);
            assert!(*exists);
        }
        _ => panic!("expected File, got {:?}", out[0]),
    }
}
```

- [ ] **Step 1.7: Run test — should fail (resolve_patterns missing)**

Run: `cargo test --test path_resolver_patterns single_existing_file_resolves_to_file_variant`
Expected: FAIL — compilation error, `resolve_patterns` not found.

- [ ] **Step 1.8: Add the enum + skeleton function in `src/config/path_resolver.rs`**

At the bottom of `src/config/path_resolver.rs`, append:

```rust
use std::fmt;

/// Result of resolving a single config pattern. Preserves the original
/// pattern string so the TUI can render meaningful labels and so removal
/// via `d` can match against the *raw* string the user wrote.
#[derive(Debug, Clone)]
pub enum ResolvedPattern {
    File {
        original: String,
        display: String,
        path: std::path::PathBuf,
        exists: bool,
    },
    Dir {
        original: String,
        display: String,
        /// Already alphabetized and binary-filtered.
        files: Vec<(std::path::PathBuf, bool)>,
    },
}

impl fmt::Display for ResolvedPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolvedPattern::File { display, .. } => write!(f, "{}", display),
            ResolvedPattern::Dir { display, .. } => write!(f, "{}", display),
        }
    }
}

fn contains_var(pat: &str) -> bool {
    let re_unix = regex::Regex::new(r"\$[A-Za-z_][A-Za-z0-9_]*").unwrap();
    let re_win = regex::Regex::new(r"%[A-Za-z_][A-Za-z0-9_]*%").unwrap();
    re_unix.is_match(pat) || re_win.is_match(pat)
}

fn tilde_collapse(expanded: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let h = home.to_string_lossy();
        if expanded.starts_with(h.as_ref()) {
            return format!("~{}", &expanded[h.len()..]);
        }
    }
    expanded.to_string()
}

fn build_display(original: &str, expanded: &str) -> String {
    if contains_var(original) {
        format!("{} ({})", tilde_collapse(expanded), original)
    } else {
        tilde_collapse(expanded)
    }
}

/// Resolve a list of config patterns, preserving the originating string
/// for each. Replaces `resolve_paths()` callers should now use this.
/// Dedup of overlapping paths is performed across the full result set:
/// first occurrence (in input order) wins; later duplicates are dropped
/// silently from `Dir.files`, or the entire `File` is dropped, with an
/// eprintln warning naming both patterns.
pub fn resolve_patterns(patterns: &[String]) -> Vec<ResolvedPattern> {
    let mut out: Vec<ResolvedPattern> = Vec::new();
    let mut seen: std::collections::HashMap<std::path::PathBuf, String> =
        std::collections::HashMap::new();

    for original in patterns {
        let expanded = expand_env_vars(&expand_tilde(original));
        if expanded.trim().is_empty() {
            eprintln!("⚠ Skipping config path (empty after expansion): {:?}", original);
            continue;
        }
        if has_unresolved_vars(&expanded) {
            eprintln!(
                "⚠ Skipping config path (unresolved variables): {:?} → {:?}",
                original, expanded
            );
            continue;
        }

        let display = build_display(original, &expanded);

        // Glob
        if expanded.contains('*') || expanded.contains('?') {
            let mut files: Vec<(std::path::PathBuf, bool)> = Vec::new();
            if let Ok(paths) = glob::glob(&expanded) {
                for entry in paths.flatten() {
                    if entry.is_file() && !crate::utils::path::is_likely_text(&entry) {
                        continue;
                    }
                    let exists = entry.exists();
                    if let Some(prev) = seen.get(&entry) {
                        eprintln!(
                            "⚠ Path {} already loaded from pattern {:?}; skipping duplicate from pattern {:?}",
                            entry.display(), prev, original
                        );
                        continue;
                    }
                    seen.insert(entry.clone(), original.clone());
                    files.push((entry, exists));
                }
            }
            files.sort_by(|a, b| a.0.cmp(&b.0));
            out.push(ResolvedPattern::Dir { original: original.clone(), display, files });
            continue;
        }

        // Try metadata to classify file vs directory
        let path = std::path::PathBuf::from(&expanded);
        match std::fs::metadata(&path) {
            Ok(m) if m.is_dir() => {
                let mut files: Vec<(std::path::PathBuf, bool)> = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&path) {
                    for entry in rd.flatten() {
                        let ep = entry.path();
                        if !ep.is_file() { continue; }
                        if !crate::utils::path::is_likely_text(&ep) { continue; }
                        if let Some(prev) = seen.get(&ep) {
                            eprintln!(
                                "⚠ Path {} already loaded from pattern {:?}; skipping duplicate from pattern {:?}",
                                ep.display(), prev, original
                            );
                            continue;
                        }
                        seen.insert(ep.clone(), original.clone());
                        files.push((ep, true));
                    }
                }
                files.sort_by(|a, b| a.0.cmp(&b.0));
                out.push(ResolvedPattern::Dir { original: original.clone(), display, files });
            }
            _ => {
                // File (existing or not)
                let exists = path.exists();
                if let Some(prev) = seen.get(&path) {
                    eprintln!(
                        "⚠ Path {} already loaded from pattern {:?}; skipping duplicate from pattern {:?}",
                        path.display(), prev, original
                    );
                    continue;
                }
                seen.insert(path.clone(), original.clone());
                out.push(ResolvedPattern::File { original: original.clone(), display, path, exists });
            }
        }
    }
    out
}
```

Make `has_unresolved_vars` `pub(crate)` (or `pub`) so the new function can see it. Locate `fn has_unresolved_vars` (currently `fn`); change to `pub(crate) fn has_unresolved_vars`.

- [ ] **Step 1.9: Keep `resolve_paths` as a transitional shim**

Replace the body of the existing `pub fn resolve_paths(...) -> Vec<(PathBuf, bool)>` to delegate:

```rust
/// DEPRECATED transitional shim — flattens resolve_patterns output to the
/// old (path, exists) tuple list. Task 3 removes this entirely.
pub fn resolve_paths(patterns: &[String]) -> Vec<(std::path::PathBuf, bool)> {
    let mut out = Vec::new();
    for p in resolve_patterns(patterns) {
        match p {
            ResolvedPattern::File { path, exists, .. } => out.push((path, exists)),
            ResolvedPattern::Dir { files, .. } => out.extend(files),
        }
    }
    out
}
```

- [ ] **Step 1.10: Run the original test + add more cases**

Run: `cargo test --test path_resolver_patterns single_existing_file_resolves_to_file_variant`
Expected: PASS.

Now append to `tests/path_resolver_patterns.rs`:

```rust
#[test]
fn glob_pattern_resolves_to_dir_with_sorted_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("b.sh"), b"x").unwrap();
    fs::write(dir.path().join("a.sh"), b"x").unwrap();
    let glob = format!("{}/*", dir.path().display());
    let out = resolve_patterns(&[glob.clone()]);
    assert_eq!(out.len(), 1);
    match &out[0] {
        ResolvedPattern::Dir { original, files, .. } => {
            assert_eq!(original, &glob);
            assert_eq!(files.len(), 2);
            assert!(files[0].0.ends_with("a.sh"));
            assert!(files[1].0.ends_with("b.sh"));
        }
        _ => panic!("expected Dir"),
    }
}

#[test]
fn directory_pattern_without_glob_resolves_to_dir() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("x.sh"), b"echo\n").unwrap();
    let p = dir.path().to_string_lossy().to_string();
    let out = resolve_patterns(&[p]);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], ResolvedPattern::Dir { .. }));
}

#[test]
fn binary_files_filtered_from_dir_expansion() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("ok.sh"), b"echo\n").unwrap();
    fs::write(dir.path().join("bad.dat"), [0u8, 1, 2, 3]).unwrap();
    let glob = format!("{}/*", dir.path().display());
    let out = resolve_patterns(&[glob]);
    match &out[0] {
        ResolvedPattern::Dir { files, .. } => {
            assert_eq!(files.len(), 1);
            assert!(files[0].0.ends_with("ok.sh"));
        }
        _ => panic!("expected Dir"),
    }
}

#[test]
fn empty_glob_still_emits_dir_header() {
    let dir = tempdir().unwrap();
    let glob = format!("{}/*", dir.path().display());
    let out = resolve_patterns(&[glob]);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], ResolvedPattern::Dir { .. }));
}

#[test]
fn unresolved_var_skipped_with_warning() {
    let out = resolve_patterns(&["$DEFINITELY_NOT_SET_XYZ/foo.sh".to_string()]);
    assert_eq!(out.len(), 0);
}

#[test]
fn duplicate_file_path_dropped_with_warning() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("a.sh");
    fs::write(&p, b"echo\n").unwrap();
    let glob = format!("{}/*", dir.path().display());
    let literal = p.to_string_lossy().to_string();
    let out = resolve_patterns(&[glob, literal.clone()]);
    // The glob captured a.sh first; literal should be dropped.
    assert_eq!(out.len(), 1);
    match &out[0] {
        ResolvedPattern::Dir { files, .. } => assert_eq!(files.len(), 1),
        _ => panic!("expected Dir"),
    }
}

#[test]
fn display_has_var_suffix_when_var_bearing() {
    // Set a synthetic var pointing at /tmp so expansion produces a real
    // (possibly absent) path. We only care that display contains "($X)".
    std::env::set_var("WENV_TEST_VAR_A", "/tmp");
    let out = resolve_patterns(&["$WENV_TEST_VAR_A/wenv_synth.sh".to_string()]);
    assert_eq!(out.len(), 1);
    let s = format!("{}", out[0]);
    assert!(s.contains("($WENV_TEST_VAR_A/wenv_synth.sh)"), "got: {}", s);
}
```

- [ ] **Step 1.11: Run all tests**

Run: `cargo test --test path_resolver_patterns`
Expected: PASS, 7 tests.

Run: `cargo test --lib` — Expected: existing lib tests still PASS (transitional `resolve_paths` shim keeps current callers working).

- [ ] **Step 1.12: Commit**

```bash
git add src/config/path_resolver.rs tests/path_resolver_patterns.rs
git commit -m "feat(path-resolver): introduce ResolvedPattern with dedup and binary filter

Adds resolve_patterns() returning structured File/Dir records that
preserve the originating config string. Dedup, binary filter, and
tilde-collapsed/(var) display labels live here. resolve_paths()
remains as a transitional shim that flattens; Task 3 removes it."
```

---

## Task 2: Config Fallback Chain, Cache, and Resources/config.toml

**Files:**
- Create: `src/config/cache.rs`
- Create: `Resources/config.toml`
- Create: `tests/config_fallback.rs`
- Modify: `src/model/config.rs` (replace `config_dir`/`config_path`/`load`/`save`)
- Modify: `src/config/mod.rs` (remove `ensure_config_dir`, `load_or_create_config`; export `cache`)
- Modify: `src/model/mod.rs` (re-export `Config` unchanged)
- Modify: `src/main.rs` (use `Config::resolve_or_create`; emit shadow warning)

### Resources/config.toml

- [ ] **Step 2.1: Create the bundled default config**

```bash
mkdir -p Resources
```

Create `Resources/config.toml` with:

```toml
[ui]
language = "en"

[files.zsh]
paths = [
    "/etc/zshenv",
    "/etc/zprofile",
    "/etc/zshrc",
    "~/.zshenv",
    "~/.zprofile",
    "~/.zshrc",
    "~/.zsh_aliases",
]

[files.bash]
paths = [
    "/etc/profile",
    "/etc/profile.d/*.sh",
    "~/.profile",
    "~/.bashrc",
    "~/.bash_aliases",
]

[files.powershell]
paths = ["$PROFILE"]
```

- [ ] **Step 2.2: Commit Resources/**

```bash
git add Resources/config.toml
git commit -m "feat(release): add bundled default Resources/config.toml"
```

### Config: source_path, fallback_paths, resolve_or_create, copy-up save

- [ ] **Step 2.3: Write the failing test for fallback_paths shape**

Create `tests/config_fallback.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use wenv::model::Config;

#[test]
fn wenv_config_dir_env_var_is_highest_priority() {
    let dir = tempdir().unwrap();
    let cfg = dir.path().join("config.toml");
    fs::write(&cfg, "[ui]\nlanguage = \"en\"\n[files]\n[snippets]\n").unwrap();

    std::env::set_var("WENV_CONFIG_DIR", dir.path());
    let resolved = Config::resolve_or_create("zsh").unwrap();
    std::env::remove_var("WENV_CONFIG_DIR");

    assert_eq!(resolved.source_path, cfg);
}

#[test]
fn missing_all_fallbacks_creates_at_first_writable() {
    // Force a writable location via WENV_CONFIG_DIR, ensure no file exists,
    // expect resolve_or_create to create one and set source_path.
    let dir = tempdir().unwrap();
    let target = dir.path().join("config.toml");
    assert!(!target.exists());

    std::env::set_var("WENV_CONFIG_DIR", dir.path());
    let cfg = Config::resolve_or_create("zsh").unwrap();
    std::env::remove_var("WENV_CONFIG_DIR");

    assert!(target.exists(), "expected created config at {}", target.display());
    assert_eq!(cfg.source_path, target);
    // Default created for the chosen shell key:
    assert!(cfg.files.contains_key("zsh"));
}

#[test]
fn save_writes_to_source_path() {
    let dir = tempdir().unwrap();
    std::env::set_var("WENV_CONFIG_DIR", dir.path());
    let mut cfg = Config::resolve_or_create("zsh").unwrap();
    std::env::remove_var("WENV_CONFIG_DIR");

    cfg.ui.language = "zh-TW".into();
    cfg.save().unwrap();

    let s = std::fs::read_to_string(&cfg.source_path).unwrap();
    assert!(s.contains("zh-TW"));
}
```

- [ ] **Step 2.4: Run — expect failure**

Run: `cargo test --test config_fallback`
Expected: FAIL — `Config::resolve_or_create` not defined.

- [ ] **Step 2.5: Rewrite `src/model/config.rs`**

Replace the `impl Config { ... }` block (currently around `src/model/config.rs:65-101`) with:

```rust
impl Config {
    /// Fallback search chain for config.toml. OS-conditional; see spec §4.1
    /// and ADR-0001. WENV_CONFIG_DIR (when set and non-empty) prepends.
    pub fn fallback_paths() -> Vec<PathBuf> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        let home = dirs::home_dir();
        let mut v: Vec<PathBuf> = Vec::new();

        if let Ok(d) = std::env::var("WENV_CONFIG_DIR") {
            if !d.trim().is_empty() {
                v.push(PathBuf::from(&d).join("config.toml"));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Some(d) = &exe_dir { v.push(d.join("Resources/config.toml")); }
            if let Some(h) = &home {
                v.push(h.join(".wenget/apps/wenv/Resources/config.toml"));
                v.push(h.join(".local/bin/Resources/config.toml"));
            }
            v.push(PathBuf::from("/opt/wenget/apps/wenv/Resources/config.toml"));
            v.push(PathBuf::from("/usr/local/bin/config/config.toml"));
        }

        #[cfg(target_os = "windows")]
        {
            let env = |k: &str| std::env::var(k).ok().map(PathBuf::from);
            if let Some(d) = &exe_dir { v.push(d.join("Resources").join("config.toml")); }
            if let Some(p) = env("USERPROFILE")  { v.push(p.join(".wenget/apps/wenv/Resources/config.toml")); }
            if let Some(p) = env("LOCALAPPDATA") { v.push(p.join("Programs/wenv/Resources/config.toml")); }
            if let Some(p) = env("ProgramW6432") { v.push(p.join("wenget/apps/wenv/Resources/config.toml")); }
            if let Some(p) = env("ProgramFiles") { v.push(p.join("gpinstall/Resources/config.toml")); }
        }

        v
    }

    /// Phase 1: find an existing config.toml in the fallback chain.
    /// Phase 2: create a default at the first writable location.
    /// Returns the loaded (or freshly created) Config with `source_path` set.
    pub fn resolve_or_create(shell_key: &str) -> anyhow::Result<Self> {
        for p in Self::fallback_paths() {
            if p.exists() {
                let content = std::fs::read_to_string(&p)?;
                let mut cfg: Config = toml::from_str(&content)?;
                cfg.source_path = p;
                return Ok(cfg);
            }
        }
        // Phase 2: build and serialize directly. No parse round-trip.
        for p in Self::fallback_paths() {
            let Some(parent) = p.parent() else { continue };
            if std::fs::create_dir_all(parent).is_err() { continue; }
            let mut cfg = Config::default();
            if let Some(paths) = crate::config::templates::default_paths(shell_key) {
                cfg.files.insert(
                    shell_key.to_string(),
                    crate::model::FilesConfig { paths },
                );
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

    /// Save to `source_path`. If write fails with PermissionDenied or
    /// ReadOnlyFilesystem, walk the fallback chain, write to the first
    /// writable other location, update source_path, log to stderr.
    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.source_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        match std::fs::write(&self.source_path, &serialized) {
            Ok(()) => Ok(()),
            Err(e) if matches!(e.kind(),
                std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::ReadOnlyFilesystem) =>
            {
                for p in Self::fallback_paths() {
                    if p == self.source_path { continue; }
                    let Some(parent) = p.parent() else { continue };
                    if std::fs::create_dir_all(parent).is_err() { continue; }
                    if std::fs::write(&p, &serialized).is_ok() {
                        eprintln!(
                            "⚠ Config at {} is read-only; saved to {} instead.",
                            self.source_path.display(), p.display()
                        );
                        self.source_path = p;
                        return Ok(());
                    }
                }
                Err(anyhow::anyhow!(
                    "Config save failed: {} is read-only and no writable fallback is available",
                    self.source_path.display()
                ))
            }
            Err(e) => Err(e.into()),
        }
    }
}
```

Also update the `Config` struct (around `src/model/config.rs:18-26`) to add `source_path`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub files: HashMap<String, FilesConfig>,
    #[serde(default)]
    pub snippets: HashMap<String, Vec<Snippet>>,
    #[serde(skip)]
    pub source_path: PathBuf,
}
```

Remove the existing manual `impl Default for Config { ... }` block (struct now derives `Default`).

Delete the unit tests at the bottom of `src/model/config.rs` that reference `Config::config_path()` — replace them with one quick smoke test referencing `fallback_paths` only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_paths_nonempty() {
        // current_exe() always succeeds on test runner
        assert!(!Config::fallback_paths().is_empty());
    }
}
```

- [ ] **Step 2.6: Run integration tests**

Run: `cargo test --test config_fallback`
Expected: PASS, 3 tests.

Run: `cargo test --lib` — Expected: PASS (note: `src/main.rs` won't yet compile against the new API; will fix in step 2.10).

If the lib doesn't compile because `src/config/mod.rs` still calls `ensure_config_dir` / `load_or_create_config`, move on to step 2.7 first.

- [ ] **Step 2.7: Trim `src/config/mod.rs`**

Replace the content of `src/config/mod.rs` with:

```rust
//! Configuration management module

pub mod cache;
pub mod path_resolver;
pub mod templates;

use anyhow::Result;

use crate::model::{Config, FilesConfig, Snippet};

/// Ensure config has file list for the given shell. Returns true if added.
pub fn ensure_shell_files(config: &mut Config, shell_key: &str) -> Result<bool> {
    if config.files.contains_key(shell_key) {
        return Ok(false);
    }
    if let Some(paths) = templates::default_paths(shell_key) {
        config
            .files
            .insert(shell_key.to_string(), FilesConfig { paths });
        config.save()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Ensure config has default snippets for the given shell if none configured.
pub fn ensure_shell_snippets(config: &mut Config, shell_key: &str) -> Result<()> {
    if config.snippets.contains_key(shell_key) {
        return Ok(());
    }
    let defaults = templates::default_snippets(shell_key);
    if !defaults.is_empty() {
        config.snippets.insert(shell_key.to_string(), defaults);
        config.save()?;
    }
    Ok(())
}

/// Load snippets for the given shell, returning configured or defaults.
pub fn load_snippets_for_shell(config: &Config, shell_key: &str) -> Vec<Snippet> {
    if let Some(snippets) = config.snippets.get(shell_key) {
        if !snippets.is_empty() {
            return snippets.clone();
        }
    }
    templates::default_snippets(shell_key)
}
```

(Removed: `ensure_config_dir`, `load_or_create_config`, `first_run_setup`, `save_config`.)

### Cache module

- [ ] **Step 2.8: Write failing test for Cache**

Append to `tests/config_fallback.rs`:

```rust
#[test]
fn cache_lives_next_to_config() {
    let dir = tempdir().unwrap();
    std::env::set_var("WENV_CONFIG_DIR", dir.path());
    let cfg = Config::resolve_or_create("zsh").unwrap();
    std::env::remove_var("WENV_CONFIG_DIR");

    use wenv::config::cache::Cache;
    let mut cache = Cache::load_or_default(&cfg);
    cache.pwsh_profile = Some("/tmp/profile.ps1".into());
    cache.save().unwrap();

    let expected = dir.path().join("cache.toml");
    assert!(expected.exists());
    let s = std::fs::read_to_string(&expected).unwrap();
    assert!(s.contains("profile.ps1"));
}
```

- [ ] **Step 2.9: Implement `src/config/cache.rs`**

Create:

```rust
//! Sibling cache for runtime-discovered paths (currently PowerShell $PROFILE).
//!
//! Lives next to the resolved config.toml (i.e. in `config.source_path.parent()`).
//! Best-effort: a failed save is logged but never fatal.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::model::Config;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub pwsh_profile: Option<String>,
    #[serde(default)]
    pub powershell_profile: Option<String>,
    #[serde(skip)]
    pub source_path: PathBuf,
}

impl Cache {
    /// Derive cache.toml location from the resolved Config.
    /// source_path is always absolute in practice (fallback chain produces
    /// absolute paths); the bare-filename fallback is defensive only.
    pub fn cache_path_for(config: &Config) -> PathBuf {
        config.source_path
            .parent()
            .map(|p| p.join("cache.toml"))
            .unwrap_or_else(|| PathBuf::from("cache.toml"))
    }

    pub fn load_or_default(config: &Config) -> Self {
        let p = Self::cache_path_for(config);
        let mut cache: Cache = if p.exists() {
            std::fs::read_to_string(&p)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Cache::default()
        };
        cache.source_path = p;
        // Lazy invalidation: if cached path no longer exists, drop it.
        // Re-population happens lazily when the caller queries the profile.
        if let Some(ref pp) = cache.pwsh_profile {
            if !std::path::Path::new(pp).exists() { cache.pwsh_profile = None; }
        }
        if let Some(ref pp) = cache.powershell_profile {
            if !std::path::Path::new(pp).exists() { cache.powershell_profile = None; }
        }
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

Also re-export from `src/lib.rs` if there is already a `pub use crate::config` line — verify with:

Run: `grep -n 'pub use' src/lib.rs`

If no `pub use` for `config::cache`, add `pub use crate::config::cache::Cache;` near the existing `pub use crate::config::path_resolver;` line (or wherever path_resolver is re-exported).

- [ ] **Step 2.10: Update src/main.rs to use the new API**

Replace lines 88-102 of `src/main.rs` (the `if cli.config` early-exit block) with a temporary stub that will be expanded in Task 5. For now, just remove the early-exit so the rest compiles:

```rust
// (Removed legacy `cli.config` early-exit; restored properly in Task 5
//  as the new `wenv config` subcommand.)
```

Replace line 108:

```rust
let mut config = wenv::config::load_or_create_config()?;
```

with:

```rust
let mut config = wenv::model::Config::resolve_or_create(shell_type.config_key())?;
```

Add the shadow warning just after the Config is loaded (before `let shell_key = ...`):

```rust
// Spec §4.6: warn if exe_dir-resolved config shadows a pre-existing
// ~/.config/wenv/config.toml.
if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
    let exe_cfg = exe_dir.join("Resources").join("config.toml");
    if config.source_path == exe_cfg {
        if let Some(home) = dirs::home_dir() {
            let legacy = home.join(".config").join("wenv").join("config.toml");
            if legacy.exists() {
                eprintln!(
                    "Note: {} exists but is shadowed by {}",
                    legacy.display(), config.source_path.display()
                );
            }
        }
    }
}
```

Also remove the `wenv::Config` re-export usage if present — replace any `wenv::Config::config_path()` call (in `src/main.rs:61-69` `startup_file_check`) with `config.source_path` based logic. Read those lines first:

Run: `sed -n '58,75p' src/main.rs`

Where it calls `config.save()` — `save()` is now `&mut self`. `config` is already `&mut` in `startup_file_check`, so the call site is fine.

- [ ] **Step 2.11: Run lib + integration tests**

Run: `cargo build --lib`
Expected: clean.

Run: `cargo build --bin wenv`
Expected: clean.

Run: `cargo test --test config_fallback`
Expected: PASS, 4 tests.

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 2.12: Test for copy-up on read-only source_path (Unix only)**

Append to `tests/config_fallback.rs`:

```rust
#[cfg(unix)]
#[test]
fn save_copies_up_when_source_is_read_only() {
    use std::os::unix::fs::PermissionsExt;

    let primary = tempdir().unwrap();
    let secondary = tempdir().unwrap();

    // Set up: primary has an existing config (read-only).
    let primary_cfg = primary.path().join("config.toml");
    fs::write(&primary_cfg,
        "[ui]\nlanguage = \"en\"\n[files]\n[snippets]\n").unwrap();
    let mut perms = fs::metadata(&primary_cfg).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&primary_cfg, perms).unwrap();
    let mut perms_dir = fs::metadata(primary.path()).unwrap().permissions();
    perms_dir.set_mode(0o555);
    fs::set_permissions(primary.path(), perms_dir).unwrap();

    std::env::set_var("WENV_CONFIG_DIR", primary.path());
    // Add secondary as an additional fallback via HOME shimming:
    let saved_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", secondary.path());
    // Create the expected secondary fallback path so it's writable:
    std::fs::create_dir_all(secondary.path().join(".wenget/apps/wenv/Resources")).unwrap();

    let mut cfg = Config::resolve_or_create("zsh").unwrap();
    assert_eq!(cfg.source_path, primary_cfg);

    cfg.ui.language = "zh-TW".into();
    cfg.save().unwrap();

    // After copy-up, source_path should have shifted to the secondary location.
    assert_ne!(cfg.source_path, primary_cfg, "source_path should have copied up");
    assert!(cfg.source_path.exists());

    // Cleanup
    if let Some(h) = saved_home { std::env::set_var("HOME", h); } else { std::env::remove_var("HOME"); }
    std::env::remove_var("WENV_CONFIG_DIR");
    // Restore permissions so tempdir cleanup works
    let mut perms = fs::metadata(&primary_cfg).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&primary_cfg, perms).unwrap();
    let mut perms_dir = fs::metadata(primary.path()).unwrap().permissions();
    perms_dir.set_mode(0o755);
    fs::set_permissions(primary.path(), perms_dir).unwrap();
}
```

- [ ] **Step 2.13: Run copy-up test**

Run: `cargo test --test config_fallback save_copies_up_when_source_is_read_only -- --nocapture`
Expected: PASS.

- [ ] **Step 2.14: Commit**

```bash
git add src/model/config.rs src/config/mod.rs src/config/cache.rs src/main.rs src/lib.rs tests/config_fallback.rs
git commit -m "feat(config): fallback chain, copy-up save, sibling cache.toml

Drop hardcoded ~/.config/wenv path. Config::resolve_or_create walks
the OS-conditional fallback chain (WENV_CONFIG_DIR override first,
then exe_dir, then user/system dirs); creates a default at the
first writable location when none exist. save() detects read-only
source_path and copies up to the next writable fallback, updating
source_path mid-session.

Cache for PowerShell \$PROFILE lives next to the resolved config
at <parent>/cache.toml with lazy exists() invalidation.

main.rs emits a stderr note when exe_dir-resolved config shadows
a pre-existing ~/.config/wenv/config.toml.

Spec §4.1-§4.7, §6.3.1. ADR-0001."
```

---

## Task 3: Three-Layer Profile Model

**Files:**
- Modify: `src/model/profile.rs` (add DirGroup/TreeNode, extend ShellProfile, rewrite ListItem/build_visible_list/toggle_all/load_shell_profile)
- Delete: `resolve_paths()` transitional shim from `src/config/path_resolver.rs`
- Create: `tests/three_layer_tree.rs`
- Create: `tests/dedup.rs`

### Profile model rewrite

- [ ] **Step 3.1: Write failing test for three-layer load**

Create `tests/three_layer_tree.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use wenv::model::profile::{load_shell_profile, ListItem, TreeNode};
use wenv::model::{Config, FilesConfig, ShellType};

fn cfg_with(shell_key: &str, paths: Vec<String>) -> Config {
    let mut c = Config::default();
    c.files.insert(shell_key.to_string(), FilesConfig { paths });
    c.source_path = std::path::PathBuf::from("/tmp/test-source-path.toml");
    c
}

#[test]
fn glob_pattern_produces_group_with_sorted_files() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("b.sh"), "echo b\n").unwrap();
    fs::write(sub.join("a.sh"), "echo a\n").unwrap();

    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    assert_eq!(prof.tree.len(), 1);
    match &prof.tree[0] {
        TreeNode::Dir(g) => {
            assert_eq!(g.file_indices.len(), 2);
            assert!(!g.expanded);
            assert!(prof.files[g.file_indices[0]].path.ends_with("a.sh"));
            assert!(prof.files[g.file_indices[1]].path.ends_with("b.sh"));
        }
        _ => panic!("expected Dir"),
    }
    // Default-collapsed: visible list contains only the dir header.
    let visible = prof.build_visible_list();
    assert_eq!(visible.len(), 1);
    assert!(matches!(visible[0], ListItem::DirHeader(0)));
}

#[test]
fn top_level_file_and_group_interleave_in_config_order() {
    let dir = tempdir().unwrap();
    let zrc = dir.path().join(".zshrc");
    fs::write(&zrc, "alias x=1\n").unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("k.sh"), "echo\n").unwrap();

    let cfg = cfg_with("zsh", vec![
        zrc.to_string_lossy().to_string(),
        format!("{}/*", sub.display()),
    ]);
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    assert_eq!(prof.tree.len(), 2);
    assert!(matches!(prof.tree[0], TreeNode::File(_)));
    assert!(matches!(prof.tree[1], TreeNode::Dir(_)));
}

#[test]
fn toggle_all_flips_dir_and_file() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("zshrc.d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("k.sh"), "x\n").unwrap();
    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let mut prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();

    prof.toggle_all(true);
    assert!(matches!(&prof.tree[0], TreeNode::Dir(g) if g.expanded));
    assert!(prof.files[0].expanded);

    prof.toggle_all(false);
    assert!(matches!(&prof.tree[0], TreeNode::Dir(g) if !g.expanded));
    assert!(!prof.files[0].expanded);
}

#[test]
fn binary_file_filtered_from_group() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("d");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("ok.sh"), "echo\n").unwrap();
    fs::write(sub.join("bin.dat"), [0u8, 1, 2]).unwrap();
    let cfg = cfg_with("zsh", vec![format!("{}/*", sub.display())]);
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    match &prof.tree[0] {
        TreeNode::Dir(g) => assert_eq!(g.file_indices.len(), 1),
        _ => panic!(),
    }
}
```

- [ ] **Step 3.2: Run — expect compile failure**

Run: `cargo test --test three_layer_tree`
Expected: FAIL — `TreeNode`, `Config::default` missing `source_path`, etc.

- [ ] **Step 3.3: Rewrite `src/model/profile.rs`**

Replace the entire content of `src/model/profile.rs` with:

```rust
//! Multi-file profile model with three-layer (group → file → entry) tree.

use crate::model::{Entry, ShellType};
use std::path::PathBuf;

/// Item in the flat visible list for TUI navigation.
#[derive(Debug, Clone, PartialEq)]
pub enum ListItem {
    DirHeader(usize),         // index into ShellProfile.tree (must be Dir)
    FileHeader(usize),        // index into ShellProfile.files
    Entry(usize, usize),      // (file_index, entry_index)
}

/// A single configuration file with its parsed entries.
pub struct ProfileFile {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub content: String,
    pub expanded: bool,
    pub dirty: bool,
    pub exists: bool,
    pub writable: bool,
    /// Display string from path_resolver (may contain "(varname)" suffix).
    pub display_label: String,
}

/// Top-level tree node. Groups bundle multiple files from a glob/dir/var
/// pattern; standalone Files have no Group parent.
pub enum TreeNode {
    Dir(DirGroup),
    File(usize), // index into ShellProfile.files
}

/// A top-level group bundling files from a single pattern (glob, dir, or
/// var that resolves to either). Identity = `source_pattern` (used as the
/// removal key when the user presses `d` on a group header).
pub struct DirGroup {
    pub source_pattern: String,
    pub display_label: String,
    pub file_indices: Vec<usize>,
    pub expanded: bool,
}

pub struct ShellProfile {
    pub shell_type: ShellType,
    pub files: Vec<ProfileFile>,
    pub tree: Vec<TreeNode>,
}

impl ProfileFile {
    pub fn new(path: PathBuf, exists: bool, display_label: String) -> Self {
        Self {
            path,
            entries: Vec::new(),
            content: String::new(),
            expanded: false,
            dirty: false,
            exists,
            writable: true,
            display_label,
        }
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Recalculate line_number / end_line / name for all entries.
    /// (Identical logic to the previous implementation.)
    pub fn recalculate_line_numbers(&mut self) {
        let mut current_line = 1usize;
        for entry in &mut self.entries {
            let line_count = entry.value.split('\n').count();
            entry.line_number = Some(current_line);
            let end = current_line + line_count - 1;
            entry.end_line = if end > current_line { Some(end) } else { entry.line_number };

            match entry.entry_type {
                crate::model::EntryType::Comment => {
                    entry.name = if end > current_line {
                        format!("#L{}-L{}", current_line, end)
                    } else {
                        format!("#L{}", current_line)
                    };
                }
                crate::model::EntryType::Code => {
                    entry.name = if end > current_line {
                        format!("L{}-L{}", current_line, end)
                    } else {
                        format!("L{}", current_line)
                    };
                }
                _ => {}
            }
            current_line = end + 1;
        }
    }

    /// Tilde-collapsed display name (for legacy callers; new code uses display_label).
    pub fn display_name(&self) -> String {
        if !self.display_label.is_empty() {
            return self.display_label.clone();
        }
        let path_str = self.path.to_string_lossy();
        if let Some(home) = dirs::home_dir() {
            let home_str = home.to_string_lossy();
            if path_str.starts_with(home_str.as_ref()) {
                return format!("~{}", &path_str[home_str.len()..]);
            }
        }
        path_str.to_string()
    }
}

impl ShellProfile {
    pub fn build_visible_list(&self) -> Vec<ListItem> {
        let mut items = Vec::new();
        for (ti, node) in self.tree.iter().enumerate() {
            match node {
                TreeNode::Dir(g) => {
                    items.push(ListItem::DirHeader(ti));
                    if g.expanded {
                        for &fi in &g.file_indices {
                            items.push(ListItem::FileHeader(fi));
                            if self.files[fi].expanded {
                                for ei in 0..self.files[fi].entries.len() {
                                    items.push(ListItem::Entry(fi, ei));
                                }
                            }
                        }
                    }
                }
                TreeNode::File(fi) => {
                    items.push(ListItem::FileHeader(*fi));
                    if self.files[*fi].expanded {
                        for ei in 0..self.files[*fi].entries.len() {
                            items.push(ListItem::Entry(*fi, ei));
                        }
                    }
                }
            }
        }
        items
    }

    pub fn total_entries(&self) -> usize {
        self.files.iter().map(|f| f.entries.len()).sum()
    }

    pub fn any_dirty(&self) -> bool {
        self.files.iter().any(|f| f.dirty)
    }

    pub fn dirty_files(&self) -> Vec<&ProfileFile> {
        self.files.iter().filter(|f| f.dirty).collect()
    }

    pub fn toggle_all(&mut self, expanded: bool) {
        for f in &mut self.files { f.expanded = expanded; }
        for n in &mut self.tree {
            if let TreeNode::Dir(g) = n { g.expanded = expanded; }
        }
    }
}

use crate::config::path_resolver::{self, ResolvedPattern};
use crate::model::Config;
use crate::parser::get_parser;

pub fn load_shell_profile(config: &Config, shell_type: ShellType) -> anyhow::Result<ShellProfile> {
    let shell_key = shell_type.config_key();
    let file_configs = config
        .files
        .get(shell_key)
        .ok_or_else(|| anyhow::anyhow!("No file list for {}", shell_key))?;

    let resolved = path_resolver::resolve_patterns(&file_configs.paths);
    let parser = get_parser(shell_type);

    let mut files: Vec<ProfileFile> = Vec::new();
    let mut tree: Vec<TreeNode> = Vec::new();

    for rp in resolved {
        match rp {
            ResolvedPattern::File { path, exists, display, .. } => {
                let fi = files.len();
                let mut pf = ProfileFile::new(path.clone(), exists, display);
                if exists {
                    let content = std::fs::read_to_string(&path)?;
                    let result = parser.parse(&content);
                    for mut entry in result.entries {
                        entry.file_index = fi;
                        pf.entries.push(entry);
                    }
                    pf.content = content;
                }
                pf.expanded = false; // spec §6.1: startup-collapsed
                files.push(pf);
                tree.push(TreeNode::File(fi));
            }
            ResolvedPattern::Dir { original, display, files: dir_files } => {
                let mut indices: Vec<usize> = Vec::new();
                for (path, exists) in dir_files {
                    let fi = files.len();
                    let mut pf = ProfileFile::new(
                        path.clone(),
                        exists,
                        // Each file inside a Group uses the file's own
                        // tilde-collapsed path as its display_label.
                        {
                            let s = path.to_string_lossy();
                            if let Some(home) = dirs::home_dir() {
                                let h = home.to_string_lossy();
                                if s.starts_with(h.as_ref()) {
                                    format!("~{}", &s[h.len()..])
                                } else { s.into_owned() }
                            } else { s.into_owned() }
                        },
                    );
                    if exists {
                        let content = std::fs::read_to_string(&path)?;
                        let result = parser.parse(&content);
                        for mut entry in result.entries {
                            entry.file_index = fi;
                            pf.entries.push(entry);
                        }
                        pf.content = content;
                    }
                    pf.expanded = false;
                    files.push(pf);
                    indices.push(fi);
                }
                tree.push(TreeNode::Dir(DirGroup {
                    source_pattern: original,
                    display_label: display,
                    file_indices: indices,
                    expanded: false,
                }));
            }
        }
    }

    Ok(ShellProfile { shell_type, files, tree })
}
```

- [ ] **Step 3.4: Run three_layer_tree tests**

Run: `cargo test --test three_layer_tree`
Expected: PASS, 4 tests.

If `Config::default()` doesn't compile (because we added `source_path: PathBuf` and now need `Default`), confirm the struct derives `Default` — `PathBuf::default()` is empty path, which is fine for the tests that explicitly assign `source_path` afterward.

### Remove transitional shim and update all callers

- [ ] **Step 3.5: Delete `resolve_paths` from path_resolver.rs**

In `src/config/path_resolver.rs`, delete the shim block (Step 1.9). Verify no other code references it:

Run: `rg 'resolve_paths' src/`
Expected: only the deleted function definition (now removed) and the new `resolve_patterns` references should remain.

Run: `cargo build --lib`
Expected: clean.

If a non-test file still references `resolve_paths`, follow the chain and update it to consume `ResolvedPattern` directly (the only known caller was `load_shell_profile`, now rewritten).

### Dedup integration test

- [ ] **Step 3.6: Add dedup integration test**

Create `tests/dedup.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use wenv::model::profile::{load_shell_profile, TreeNode};
use wenv::model::{Config, FilesConfig, ShellType};

#[test]
fn glob_and_literal_overlap_dedups_keeping_first() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("d");
    fs::create_dir_all(&sub).unwrap();
    let a = sub.join("a.sh");
    fs::write(&a, "echo a\n").unwrap();

    let mut cfg = Config::default();
    cfg.source_path = std::path::PathBuf::from("/tmp/x.toml");
    cfg.files.insert("zsh".into(), FilesConfig {
        paths: vec![
            format!("{}/*", sub.display()),   // captures a.sh
            a.to_string_lossy().to_string(),   // duplicate — should be dropped
        ],
    });

    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    // First node: Dir(a.sh). Second node: File but the path was already
    // consumed by the Dir → the File node has no backing ProfileFile.
    // Implementation: duplicate File entirely dropped from tree.
    assert_eq!(prof.tree.len(), 1, "second node should have been dropped");
    match &prof.tree[0] {
        TreeNode::Dir(g) => assert_eq!(g.file_indices.len(), 1),
        _ => panic!(),
    }
}
```

- [ ] **Step 3.7: Run dedup test**

Run: `cargo test --test dedup`
Expected: PASS.

- [ ] **Step 3.8: Sanity check — full test suite**

Run: `cargo test`
Expected: all green. Existing TUI logic tests should still pass because `ListItem::FileHeader` / `Entry` variants survived (only `DirHeader` was added).

If a TUI test fails because it pattern-matched on `ListItem` exhaustively, add a `_ => unreachable!()` arm or update the test.

- [ ] **Step 3.9: Commit**

```bash
git add src/model/profile.rs src/config/path_resolver.rs tests/three_layer_tree.rs tests/dedup.rs
git commit -m "feat(model): three-layer tree with DirGroup + TreeNode

Add DirGroup/TreeNode; ShellProfile gains tree: Vec<TreeNode>
while files: Vec<ProfileFile> stays linear so existing
cut/paste/undo continue to work. ListItem gets DirHeader variant.
build_visible_list / toggle_all walk the tree. load_shell_profile
consumes path_resolver::resolve_patterns directly; transitional
resolve_paths shim removed.

Default-collapsed at load (spec §6.1).
Dedup honored (first pattern wins).

Spec §2, §3.2.1, §6.1."
```

---

## Task 4: TUI Keys and Rendering

**Files:**
- Modify: `src/tui/app.rs` (DirHeader events, targeted reload, filter, saved_expanded)
- Modify: `src/tui/list.rs`, `src/tui/ui.rs` (three-layer rendering)
- Modify: `src/tui/state.rs` (extend saved_expanded)
- Create: `tests/targeted_reload.rs`

### Targeted reload tests first

- [ ] **Step 4.1: Write failing test for targeted reload**

Create `tests/targeted_reload.rs`:

```rust
use std::fs;
use tempfile::tempdir;
use wenv::model::profile::{load_shell_profile, TreeNode};
use wenv::model::{Config, FilesConfig, ShellType};

fn make_cfg(td: &tempfile::TempDir, paths: Vec<String>) -> Config {
    let mut c = Config::default();
    c.source_path = td.path().join("config.toml");
    c.files.insert("zsh".into(), FilesConfig { paths });
    c
}

#[test]
fn loading_and_reloading_keeps_indices_stable() {
    let td = tempdir().unwrap();
    fs::write(td.path().join("a.sh"), "alias a=1\n").unwrap();
    fs::write(td.path().join("b.sh"), "alias b=2\n").unwrap();
    let cfg = make_cfg(&td, vec![
        td.path().join("a.sh").to_string_lossy().to_string(),
        td.path().join("b.sh").to_string_lossy().to_string(),
    ]);
    let prof = load_shell_profile(&cfg, ShellType::Zsh).unwrap();
    assert_eq!(prof.files.len(), 2);
    // After removing the second pattern, only a.sh remains
    let cfg2 = make_cfg(&td, vec![
        td.path().join("a.sh").to_string_lossy().to_string(),
    ]);
    let prof2 = load_shell_profile(&cfg2, ShellType::Zsh).unwrap();
    assert_eq!(prof2.files.len(), 1);
    assert_eq!(prof2.tree.len(), 1);
    assert!(matches!(prof2.tree[0], TreeNode::File(0)));
}
```

(Targeted-patch logic itself lives in `app.rs` and is harder to unit-test in isolation; the integration test confirms `load_shell_profile` is deterministic after pattern removal — the app uses this same function via the patch helper.)

- [ ] **Step 4.2: Run — should pass already (load_shell_profile already implemented)**

Run: `cargo test --test targeted_reload`
Expected: PASS.

### Extend `src/tui/state.rs`

- [ ] **Step 4.3: Read state.rs and extend saved_expanded**

Run: `cat src/tui/state.rs`

Find any struct field named `saved_expanded` (probably `Vec<bool>` over files). Replace with:

```rust
/// Snapshot of expanded state across both layers, captured before filter
/// activation or file-move mode, restored on exit.
#[derive(Default, Clone)]
pub struct ExpandedSnapshot {
    pub files: Vec<bool>,
    pub dirs:  Vec<bool>, // indexed by tree position; non-Dir entries are ignored
}
```

Replace existing `saved_expanded: Vec<bool>` declarations with `saved_expanded: ExpandedSnapshot`. Re-export `ExpandedSnapshot` from `state.rs`.

### Event handler — DirHeader

- [ ] **Step 4.4: Locate the key dispatch and add DirHeader branches**

Run: `grep -n "ListItem::FileHeader" src/tui/app.rs | head -20`

For each location that currently does `match item { ListItem::FileHeader(fi) => ..., ListItem::Entry(...) => ... }`, add a `ListItem::DirHeader(ti) => ...` arm. The required behaviors per spec §6.2:

For the Enter/Space toggle (around `src/tui/app.rs:1113-1121`):

```rust
ListItem::DirHeader(ti) => {
    if let TreeNode::Dir(g) = &mut self.profile.tree[*ti] {
        g.expanded = !g.expanded;
    }
}
```

For the `d` (delete) key handler (search `'d' =>` or similar — the existing key match for `KeyCode::Char('d')`):

```rust
Some(ListItem::DirHeader(ti)) => {
    let (pattern, file_count) = match &self.profile.tree[*ti] {
        TreeNode::Dir(g) => (g.source_pattern.clone(), g.file_indices.len()),
        _ => return Ok(()),
    };
    let confirm = dialoguer::Confirm::new()
        .with_prompt(format!(
            "Remove group '{}' from config? ({} files will be hidden)",
            pattern, file_count
        ))
        .default(false)
        .interact_opt()?
        .unwrap_or(false);
    if confirm {
        if let Some(fc) = self.config.files.get_mut(&self.shell_key) {
            fc.paths.retain(|p| p != &pattern);
        }
        self.config.save()?;
        self.reload_profile()?;
    }
}
```

For the `m` (move) handler — DirHeader is unsupported:

```rust
Some(ListItem::DirHeader(_)) => {
    self.status_line = "Move is not supported on groups".into();
    return Ok(());
}
```

For the `e` / `s` / `x` / `c` / `v` / `r` handlers, add `Some(ListItem::DirHeader(_)) => { return Ok(()); }` no-op arms.

For File-inside-Group `m` blocker — locate the existing `m` for `FileHeader`:

```rust
Some(ListItem::FileHeader(fi)) => {
    let is_inside_group = self.profile.tree.iter().any(|n| matches!(n,
        TreeNode::Dir(g) if g.file_indices.contains(fi)));
    if is_inside_group {
        self.status_line =
            "Files inside a group are sorted alphabetically; move is not supported".into();
        return Ok(());
    }
    // ... existing top-level file move logic
}
```

### Targeted reload helper

- [ ] **Step 4.5: Add `reload_profile()` to `impl TuiApp`**

Insert in `src/tui/app.rs` (anywhere in `impl TuiApp { ... }`):

```rust
/// Spec §6.8 — targeted patch reload after `a` / `d` / startup_file_check.
///
/// Strategy for v0.17: rebuild profile from scratch via load_shell_profile,
/// but preserve `expanded` state for files whose `path` survives, and clear
/// clipboard/undo if any cut/copy targeted a removed file.
fn reload_profile(&mut self) -> anyhow::Result<()> {
    use std::collections::HashMap;
    // Snapshot expanded state by path.
    let old_file_expanded: HashMap<std::path::PathBuf, bool> = self.profile.files
        .iter().map(|f| (f.path.clone(), f.expanded)).collect();
    let old_dir_expanded: HashMap<String, bool> = self.profile.tree.iter()
        .filter_map(|n| match n {
            crate::model::profile::TreeNode::Dir(g) =>
                Some((g.source_pattern.clone(), g.expanded)),
            _ => None,
        }).collect();

    let mut new_profile = crate::model::profile::load_shell_profile(
        &self.config, self.profile.shell_type)?;

    // Restore expanded state by path / pattern key.
    for f in &mut new_profile.files {
        if let Some(&e) = old_file_expanded.get(&f.path) { f.expanded = e; }
    }
    for n in &mut new_profile.tree {
        if let crate::model::profile::TreeNode::Dir(g) = n {
            if let Some(&e) = old_dir_expanded.get(&g.source_pattern) {
                g.expanded = e;
            }
        }
    }

    // Clipboard/undo invalidation: if old file_index references no longer
    // resolve to the same path, clear the buffers and notify.
    let old_paths: Vec<std::path::PathBuf> = self.profile.files
        .iter().map(|f| f.path.clone()).collect();
    let new_paths: Vec<std::path::PathBuf> = new_profile.files
        .iter().map(|f| f.path.clone()).collect();
    if old_paths != new_paths {
        // Conservative: clear clipboard + undo so stale file_index refs
        // can't surface (the buffers are bounded; loss is acceptable).
        self.state.clipboard.clear();
        self.state.undo_stack.clear();
        self.status_line = "Clipboard and undo cleared (file set changed)".into();
    }

    self.profile = new_profile;
    self.visible_items = self.profile.build_visible_list();
    // Clamp cursor.
    if self.cursor >= self.visible_items.len() {
        self.cursor = self.visible_items.len().saturating_sub(1);
    }
    Ok(())
}
```

Adjust field names (`self.state.clipboard`, `self.state.undo_stack`, `self.status_line`, `self.cursor`, `self.visible_items`) to whatever the actual TuiApp struct uses — read `src/tui/app.rs` definition first:

Run: `sed -n '1,80p' src/tui/app.rs`

### `a` key prompt text

- [ ] **Step 4.6: Update the `a` key prompt**

Search for the existing `a`-key handler in `src/tui/app.rs`. It likely calls `dialoguer::Input::new().with_prompt(...)`. Replace the prompt string with:

```rust
"Add file, group, glob, or $VAR to config"
```

After accepting input, call `self.reload_profile()` instead of any ad-hoc rebuild.

### List rendering

- [ ] **Step 4.7: Read `src/tui/list.rs`**

Run: `cat src/tui/list.rs`

- [ ] **Step 4.8: Update rendering for three-layer indentation**

Identify the function that maps `ListItem` → `ratatui::widgets::ListItem`. Add a `DirHeader` arm and use indent levels:

```rust
ListItem::DirHeader(ti) => {
    let g = match &profile.tree[*ti] {
        TreeNode::Dir(g) => g,
        _ => return None,
    };
    let marker = if g.expanded { "▼" } else { "▶" };
    let line = format!(
        "{} {}    [{} files]",
        marker, g.display_label, g.file_indices.len()
    );
    Some(ratatui::widgets::ListItem::new(line))
}
ListItem::FileHeader(fi) => {
    let f = &profile.files[*fi];
    let marker = if f.expanded { "▼" } else { "▶" };
    // Indent files that live under a DirGroup.
    let inside_group = profile.tree.iter().any(|n| matches!(n,
        TreeNode::Dir(g) if g.file_indices.contains(fi)));
    let prefix = if inside_group { "  " } else { "" };
    let line = format!(
        "{}{} {}    [{} entries]",
        prefix, marker, f.display_name(), f.entry_count()
    );
    Some(ratatui::widgets::ListItem::new(line))
}
ListItem::Entry(fi, ei) => {
    let f = &profile.files[*fi];
    let e = &f.entries[*ei];
    let inside_group = profile.tree.iter().any(|n| matches!(n,
        TreeNode::Dir(g) if g.file_indices.contains(fi)));
    let prefix = if inside_group { "    " } else { "  " };
    let line = format!("{}{}", prefix, e.summary_line()); // existing helper
    Some(ratatui::widgets::ListItem::new(line))
}
```

(`summary_line()` is the existing method that renders an entry — if it's named differently in the current code, keep the existing line-building expression and only change the prefix string.)

### Filter mode

- [ ] **Step 4.9: Update filter logic**

Locate the existing filter handler (around `src/tui/app.rs:1133`). The current code sets `file.expanded = matched_files.contains(&i)` for each file. Add Group handling:

```rust
fn apply_filter(&mut self) {
    let query = self.filter.query.to_lowercase();
    // Determine which files have at least one matching entry.
    let mut matched_files: std::collections::HashSet<usize> =
        std::collections::HashSet::new();
    for (fi, f) in self.profile.files.iter().enumerate() {
        if f.entries.iter().any(|e| e.matches_query(&query)) {
            matched_files.insert(fi);
        }
    }

    // Expand matching files; collapse non-matching.
    for (fi, f) in self.profile.files.iter_mut().enumerate() {
        f.expanded = matched_files.contains(&fi);
    }

    // Expand groups that have at least one matching descendant; collapse others.
    for n in &mut self.profile.tree {
        if let TreeNode::Dir(g) = n {
            g.expanded = g.file_indices.iter().any(|fi| matched_files.contains(fi));
        }
    }

    self.matched_files = matched_files;
    self.visible_items = self.profile.build_visible_list_filtered();
}
```

Then add a sister method on `ShellProfile`:

```rust
// in src/model/profile.rs
impl ShellProfile {
    /// Like build_visible_list but additionally skips non-matching items
    /// based on the provided set. Used in filter mode.
    pub fn build_visible_list_filtered(
        &self,
        matched_files: &std::collections::HashSet<usize>,
    ) -> Vec<ListItem> {
        let mut items = Vec::new();
        for (ti, node) in self.tree.iter().enumerate() {
            match node {
                TreeNode::Dir(g) => {
                    let any_match = g.file_indices.iter()
                        .any(|fi| matched_files.contains(fi));
                    if !any_match { continue; }
                    items.push(ListItem::DirHeader(ti));
                    if g.expanded {
                        for &fi in &g.file_indices {
                            if !matched_files.contains(&fi) { continue; }
                            items.push(ListItem::FileHeader(fi));
                            if self.files[fi].expanded {
                                for ei in 0..self.files[fi].entries.len() {
                                    items.push(ListItem::Entry(fi, ei));
                                }
                            }
                        }
                    }
                }
                TreeNode::File(fi) => {
                    if !matched_files.contains(fi) { continue; }
                    items.push(ListItem::FileHeader(*fi));
                    if self.files[*fi].expanded {
                        for ei in 0..self.files[*fi].entries.len() {
                            items.push(ListItem::Entry(*fi, ei));
                        }
                    }
                }
            }
        }
        items
    }
}
```

Update the `apply_filter` caller signature: `self.profile.build_visible_list_filtered(&matched_files)`.

### Saved expanded restoration

- [ ] **Step 4.10: Capture and restore snapshot on filter Esc**

Locate the filter Esc handler. Before applying filter, save:

```rust
self.state.saved_expanded = ExpandedSnapshot {
    files: self.profile.files.iter().map(|f| f.expanded).collect(),
    dirs:  self.profile.tree.iter().map(|n| match n {
        TreeNode::Dir(g) => g.expanded,
        _ => false,
    }).collect(),
};
```

On Esc (filter exit), restore:

```rust
for (i, f) in self.profile.files.iter_mut().enumerate() {
    if let Some(&e) = self.state.saved_expanded.files.get(i) { f.expanded = e; }
}
for (i, n) in self.profile.tree.iter_mut().enumerate() {
    if let (TreeNode::Dir(g), Some(&e)) = (n, self.state.saved_expanded.dirs.get(i)) {
        g.expanded = e;
    }
}
```

### Build + smoke

- [ ] **Step 4.11: Build and run**

Run: `cargo build --bin wenv`
Expected: clean. Fix any compile errors as they surface (field names, method names).

Run: `cargo test`
Expected: all green.

- [ ] **Step 4.12: Commit**

```bash
git add src/tui/app.rs src/tui/list.rs src/tui/ui.rs src/tui/state.rs src/model/profile.rs tests/targeted_reload.rs
git commit -m "feat(tui): three-layer rendering, DirHeader keys, targeted reload

- DirHeader events: Enter toggles group; d confirms + removes pattern
  from config; m/e/s/x/c/v/r are no-ops with status hints.
- Files inside a group reject m with explanatory status line.
- a prompt: 'Add file, group, glob, or \$VAR to config'.
- Three-level indentation (0/2/4) + [N files] / [N entries] suffixes.
- Filter hides non-matching files inside expanded groups and hides
  groups with zero matching descendants. ExpandedSnapshot covers both
  file and group state on filter activate/exit.
- reload_profile() preserves expanded state by path/pattern key;
  clipboard + undo invalidated when the file set changes.

Spec §6.2, §6.3, §6.4, §6.5, §6.6, §6.8."
```

---

## Task 5: CLI `wenv config` Subcommand

**Files:**
- Modify: `src/cli/args.rs`
- Modify: `src/main.rs`

- [ ] **Step 5.1: Write the args module**

Replace `src/cli/args.rs` content with:

```rust
//! CLI argument definitions

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "wenv")]
#[command(about = "Shell configuration file manager")]
#[command(version, author)]
pub struct Cli {
    /// Specify shell type
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

#[derive(Subcommand)]
pub enum SubCmd {
    /// Open wenv config file in $EDITOR
    Config,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellArg {
    Bash,
    Zsh,
    Pwsh,
}

impl From<ShellArg> for crate::model::ShellType {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => crate::model::ShellType::Bash,
            ShellArg::Zsh => crate::model::ShellType::Zsh,
            ShellArg::Pwsh => crate::model::ShellType::PowerShell,
        }
    }
}
```

- [ ] **Step 5.2: Wire `wenv config` in main.rs**

In `src/main.rs`, after `let cli = Cli::parse();` (before the shell type detection), add:

```rust
if matches!(cli.subcommand, Some(wenv::cli::args::SubCmd::Config)) {
    let shell_type = wenv::utils::shell_detect::get_shell_type(
        cli.shell.map(Into::into), None);
    let cfg = wenv::model::Config::resolve_or_create(shell_type.config_key())?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) { "notepad".into() } else { "vi".into() }
    });
    std::process::Command::new(&editor).arg(&cfg.source_path).status()?;
    return Ok(());
}
```

- [ ] **Step 5.3: Build and smoke-test help output**

Run: `cargo build --bin wenv`
Expected: clean.

Run: `cargo run -- --help`
Expected output includes a `Commands:` section listing `config`. No `-c` flag.

Run: `cargo run -- config --help`
Expected: clap-formatted help for `wenv config` (the bare subcommand has no extra args).

- [ ] **Step 5.4: Commit**

```bash
git add src/cli/args.rs src/main.rs
git commit -m "feat(cli): replace -c/--config flag with 'wenv config' subcommand

Breaking: 'wenv -c' / 'wenv --config' no longer accepted.
'wenv config' opens the currently-resolved config in \$EDITOR
(or notepad/vi default), matching legacy behavior.

Spec §5."
```

---

## Task 6: Release Workflow + README/CHANGELOG

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

### Release workflow

- [ ] **Step 6.1: Read the current workflow archive section**

Run: `sed -n '188,215p' .github/workflows/release.yml`

- [ ] **Step 6.2: Replace archive steps to bundle Resources/**

In `.github/workflows/release.yml`, replace the existing two steps "Create tarball (Linux and macOS)" and "Upload artifacts (Windows)" with the staged version below. The "Upload artifacts (Linux and macOS)" step stays but now uploads from the staged path.

```yaml
      - name: Stage artifacts
        shell: bash
        run: |
          STAGE="${{ matrix.asset_name }}-staged"
          mkdir -p "$STAGE/Resources"
          if [[ "${{ matrix.os }}" == "windows-latest" ]]; then
            cp "target/${{ matrix.target }}/release/${{ matrix.artifact_name }}" "$STAGE/"
          else
            cp "target/${{ matrix.target }}/release/${{ matrix.artifact_name }}" "$STAGE/"
          fi
          cp Resources/config.toml "$STAGE/Resources/config.toml"
          echo "STAGE=$STAGE" >> "$GITHUB_ENV"

      - name: Create tarball (Linux and macOS)
        if: matrix.os != 'windows-latest'
        run: |
          tar czf "${{ matrix.asset_name }}.tar.gz" -C "$STAGE" .
          # alternative: tar czf "${{ matrix.asset_name }}.tar.gz" "$STAGE"
          # keep the simpler layout above

      - name: Create zip (Windows)
        if: matrix.os == 'windows-latest'
        shell: pwsh
        run: |
          Compress-Archive -Path "$env:STAGE/*" -DestinationPath "${{ matrix.asset_name }}.zip"

      - name: Upload artifacts (Linux and macOS)
        if: matrix.os != 'windows-latest'
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset_name }}
          path: ${{ matrix.asset_name }}.tar.gz

      - name: Upload artifacts (Windows)
        if: matrix.os == 'windows-latest'
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset_name }}
          path: ${{ matrix.asset_name }}.zip
```

Also update the "Prepare release files" step in the `release` job (around `release.yml:236`):

```yaml
      - name: Prepare release files
        run: |
          mkdir -p release_files
          # Tarballs from Linux/macOS builds
          find artifacts -type f -name "*.tar.gz" -exec cp {} release_files/ \;
          # Zips from Windows builds
          find artifacts -type f -name "*.zip" -exec cp {} release_files/ \;
```

(Remove the bespoke "Handle Windows exe files by renaming with platform suffix" block — Windows now ships a zip.)

- [ ] **Step 6.3: Verify YAML syntactically**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: no output (valid YAML).

### README

- [ ] **Step 6.4: Add Development + tarball-layout sections to README**

Append to `README.md`:

```markdown
## Configuration

wenv searches for `config.toml` in an OS-conditional fallback chain and creates a default at the first writable location if none exist. On Linux/macOS the chain is:

1. `$WENV_CONFIG_DIR/config.toml` (when set, for development)
2. `<binary_dir>/Resources/config.toml` (bundled with release tarballs)
3. `$HOME/.wenget/apps/wenv/Resources/config.toml`
4. `$HOME/.local/bin/Resources/config.toml`
5. `/opt/wenget/apps/wenv/Resources/config.toml`
6. `/usr/local/bin/config/config.toml`

Run `wenv config` to open the currently-resolved file in `$EDITOR`.

If the resolved config sits on a read-only filesystem and you make changes in the TUI, wenv writes to the next writable fallback and prints a notice; subsequent runs find the new copy.

See `docs/adr/0001-config-resolution-strategy.md` for the rationale.

## Release tarball layout

Each release tarball unpacks to:

```
wenv-vX.Y.Z-<target>/
├── wenv (or wenv.exe)
└── Resources/
    └── config.toml
```

## Development

To run a development build against an isolated config, set `WENV_CONFIG_DIR`:

```bash
WENV_CONFIG_DIR=$(pwd)/Resources cargo run
```

This prepends the in-repo `Resources/config.toml` to the fallback chain so `cargo run` never touches your installed config.
```

### CHANGELOG

- [ ] **Step 6.5: Add Unreleased entry**

Prepend to `CHANGELOG.md` (after any existing top heading):

```markdown
## Unreleased

### Breaking changes
- `-c, --config` flag removed. Use `wenv config` subcommand instead.
- Config moved from `~/.config/wenv/config.toml` to an OS-conditional fallback chain (see README "Configuration"). No automatic migration; existing files are silently shadowed if a higher-priority fallback exists.
- TUI startup default: all groups and files start collapsed; press `9` to expand all or `0` to collapse all.

### Added
- Three-layer TUI tree: group → file → entry. Globs (`~/.zshrc.d/*`), directory paths, and variables that resolve to directories produce a group.
- `wenv config` subcommand opens the active config in `$EDITOR`.
- `WENV_CONFIG_DIR` env var (development override).
- Release tarballs bundle `Resources/config.toml` next to the binary.
- Sibling `cache.toml` for PowerShell `$PROFILE` resolution (lazy invalidation).
- Variable-bearing paths display as `<resolved> (<original-pattern>)`.

### Fixed
- (None — feature release.)
```

- [ ] **Step 6.6: Commit**

```bash
git add .github/workflows/release.yml README.md CHANGELOG.md
git commit -m "feat(release): bundle Resources/config.toml; docs for v0.17

- release.yml stages each platform binary into wenv-vX.Y.Z-<target>/
  with Resources/config.toml sibling; Windows now ships zip.
- README adds Configuration, Tarball layout, Development sections.
- CHANGELOG records three breaking changes for v0.17.

Spec §7, §9."
```

---

## Done Criteria (run from repo root)

- [ ] `cargo test` — all green
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (or document any remaining warnings inline)
- [ ] `cargo fmt --check` — clean
- [ ] Manual smoke (macOS shell session):
  - `cargo run` → TUI starts fully collapsed; `9` expands all, `0` collapses all.
  - Add a glob to config (`a` in TUI, e.g. `~/.zshrc.d/*`) and verify a Group header appears with the matched files.
  - Pwsh: with `$PROFILE` in config, the file label shows `<resolved> ($PROFILE)`.
  - `wenv config` opens the config in `$EDITOR`; change `language`, save, re-run, observe new language.
  - `d` on a Group → confirms, removes pattern, files vanish.
  - `m` on a Group → status hint "Move is not supported on groups".
  - `m` on a file inside a Group → status hint about alphabetical ordering.
- [ ] CHANGELOG `Unreleased` lists three breaking changes
- [ ] README "Configuration" / "Development" / "Tarball layout" sections present
- [ ] `Resources/config.toml` committed at repo root
- [ ] `docs/adr/0001-config-resolution-strategy.md` committed (already done during grilling)
