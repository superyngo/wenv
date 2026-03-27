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
fn has_unresolved_vars(path: &str) -> bool {
    let re_unix = regex::Regex::new(r"\$[A-Za-z_][A-Za-z0-9_]*").unwrap();
    let re_win = regex::Regex::new(r"%[A-Za-z_][A-Za-z0-9_]*%").unwrap();
    re_unix.is_match(path) || re_win.is_match(path)
}

pub fn resolve_paths(patterns: &[String]) -> Vec<(PathBuf, bool)> {
    let mut results = Vec::new();
    for pattern in patterns {
        let expanded = expand_env_vars(&expand_tilde(pattern));

        // Skip paths that are empty or whitespace-only after expansion
        if expanded.trim().is_empty() {
            eprintln!(
                "⚠ Skipping config path (empty after expansion): {:?}",
                pattern
            );
            continue;
        }

        // Skip paths that still contain unresolved variable placeholders
        if has_unresolved_vars(&expanded) {
            eprintln!(
                "⚠ Skipping config path (unresolved variables): {:?} → {:?}",
                pattern, expanded
            );
            continue;
        }

        if expanded.contains('*') || expanded.contains('?') {
            if let Ok(paths) = glob::glob(&expanded) {
                for entry in paths.flatten() {
                    let exists = entry.exists();
                    results.push((entry, exists));
                }
            }
        } else {
            let path = PathBuf::from(&expanded);
            let exists = path.exists();
            results.push((path, exists));
        }
    }
    results
}
