//! Shell detection utilities

use crate::model::ShellType;
use std::path::Path;

/// Detect shell type from file extension
pub fn detect_from_file(path: &Path) -> Option<ShellType> {
    // First check filename for common patterns (for files without extensions)
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        if filename.contains("bashrc")
            || filename.contains("bash_profile")
            || filename.contains("bash_aliases")
        {
            return Some(ShellType::Bash);
        }
        if filename.contains("profile.ps1") || filename.contains("PowerShell") {
            return Some(ShellType::PowerShell);
        }
    }

    // Then check extension
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        match extension.to_lowercase().as_str() {
            "sh" | "bash" => return Some(ShellType::Bash),
            "ps1" | "psm1" => return Some(ShellType::PowerShell),
            _ => {}
        }
    }

    None
}

/// Get the appropriate shell type for the current context
pub fn get_shell_type(specified: Option<ShellType>, file_path: Option<&Path>) -> ShellType {
    // Priority: specified > file detection > environment detection > default
    if let Some(shell) = specified {
        return shell;
    }

    if let Some(path) = file_path {
        if let Some(shell) = detect_from_file(path) {
            return shell;
        }
    }

    ShellType::detect().unwrap_or(ShellType::Bash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_from_file_bash() {
        let path = PathBuf::from("/home/user/.bashrc");
        assert_eq!(detect_from_file(&path), Some(ShellType::Bash));
    }

    #[test]
    fn test_detect_from_file_ps1() {
        let path = PathBuf::from("/home/user/profile.ps1");
        assert_eq!(detect_from_file(&path), Some(ShellType::PowerShell));
    }

    #[test]
    fn test_get_shell_type_specified() {
        let result = get_shell_type(Some(ShellType::PowerShell), None);
        assert_eq!(result, ShellType::PowerShell);
    }
}
