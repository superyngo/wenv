//! Resolve config path patterns to concrete file paths

use std::path::PathBuf;

pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

pub fn expand_env_vars(path: &str) -> String {
    let mut result = path.to_string();

    // Unix-style $VAR expansion
    let re_unix = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in re_unix.captures_iter(path) {
        let var_name = &cap[1];
        if let Ok(val) = std::env::var(var_name) {
            result = result.replace(&cap[0], &val);
        } else if var_name == "PROFILE" {
            if let Some(val) = query_powershell_profile() {
                result = result.replace(&cap[0], &val);
            }
        }
    }

    // Windows-style %VAR% expansion
    let re_win = regex::Regex::new(r"%([A-Za-z_][A-Za-z0-9_]*)%").unwrap();
    let snapshot = result.clone();
    for cap in re_win.captures_iter(&snapshot) {
        let var_name = &cap[1];
        if let Ok(val) = std::env::var(var_name) {
            result = result.replace(&cap[0], &val);
        }
    }

    result
}

/// Query PowerShell for the $PROFILE path when not available as env var.
/// Tries `pwsh` first (cross-platform), then `powershell` (Windows-only).
/// Forces UTF-8 output encoding so paths with non-ASCII characters are decoded correctly.
fn query_powershell_profile() -> Option<String> {
    for cmd in &["pwsh", "powershell"] {
        if let Ok(output) = std::process::Command::new(cmd)
            .args([
                "-NoProfile",
                "-Command",
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Write-Output $PROFILE",
            ])
            .output()
        {
            if output.status.success() {
                let val = decode_utf8_strip_bom(&output.stdout).trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// Decode bytes as UTF-8, stripping a leading BOM if present.
/// PowerShell on Windows sometimes emits a UTF-8 BOM when OutputEncoding is set to UTF-8.
fn decode_utf8_strip_bom(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Returns true if the path string still contains unresolved `$VAR` or `%VAR%` placeholders.
pub(crate) fn has_unresolved_vars(path: &str) -> bool {
    let re_unix = regex::Regex::new(r"\$[A-Za-z_][A-Za-z0-9_]*").unwrap();
    let re_win = regex::Regex::new(r"%[A-Za-z_][A-Za-z0-9_]*%").unwrap();
    re_unix.is_match(path) || re_win.is_match(path)
}

/// DEPRECATED transitional shim — flattens resolve_patterns output to the
/// old (path, exists) tuple list. Task 3 removes this entirely.
pub fn resolve_paths(patterns: &[String]) -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    for p in resolve_patterns(patterns) {
        match p {
            ResolvedPattern::File { path, exists, .. } => out.push((path, exists)),
            ResolvedPattern::Dir { files, .. } => out.extend(files),
        }
    }
    out
}

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

pub fn tilde_collapse(expanded: &str) -> String {
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
