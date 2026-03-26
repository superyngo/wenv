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
    let re = regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    for cap in re.captures_iter(path) {
        let var_name = &cap[1];
        if let Ok(val) = std::env::var(var_name) {
            result = result.replace(&cap[0], &val);
        }
    }
    result
}

pub fn resolve_paths(patterns: &[String]) -> Vec<(PathBuf, bool)> {
    let mut results = Vec::new();
    for pattern in patterns {
        let expanded = expand_env_vars(&expand_tilde(pattern));
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
