//! Shell type detection and configuration paths

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Supported shell types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    PowerShell,
}

impl ShellType {
    /// Detect shell type from environment
    pub fn detect() -> Option<Self> {
        // Check $SHELL environment variable
        if let Ok(shell) = env::var("SHELL") {
            if shell.contains("bash") {
                return Some(ShellType::Bash);
            }
            if shell.contains("pwsh") || shell.contains("powershell") {
                return Some(ShellType::PowerShell);
            }
        }

        // Check for PowerShell-specific environment variable
        if env::var("PSModulePath").is_ok() {
            return Some(ShellType::PowerShell);
        }

        // Check Windows default
        #[cfg(windows)]
        {
            if env::var("COMSPEC").is_ok() {
                // On Windows, check if PowerShell is available
                Some(ShellType::PowerShell)
            } else {
                None
            }
        }

        // Default to Bash on Unix-like systems
        #[cfg(unix)]
        {
            Some(ShellType::Bash)
        }

        #[cfg(not(any(unix, windows)))]
        None
    }

    /// Get the default configuration file path for this shell
    pub fn default_config_path(&self) -> PathBuf {
        match self {
            ShellType::Bash => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".bashrc"),
            ShellType::PowerShell => {
                // Prioritize $PROFILE environment variable (only available in PowerShell sessions)
                if let Ok(profile_path) = env::var("PROFILE") {
                    return PathBuf::from(profile_path);
                }

                // Try to query the shell directly for the profile path
                let get_profile = |cmd: &str| -> Option<PathBuf> {
                    Command::new(cmd)
                        .args(["-NoProfile", "-Command", "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Write-Output $PROFILE"])
                        .output()
                        .ok()
                        .and_then(|output| {
                            if output.status.success() {
                                let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
                                if !s.is_empty() {
                                    return Some(PathBuf::from(s));
                                }
                            }
                            None
                        })
                };

                // Try pwsh (PowerShell Core) first, then powershell (Windows PowerShell)
                if let Some(path) = get_profile("pwsh").or_else(|| get_profile("powershell")) {
                    return path;
                }

                // Fallback to standard paths
                #[cfg(windows)]
                {
                    dirs::document_dir()
                        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("~")))
                        .join("PowerShell")
                        .join("Microsoft.PowerShell_profile.ps1")
                }
                #[cfg(not(windows))]
                {
                    dirs::config_dir()
                        .unwrap_or_else(|| {
                            dirs::home_dir()
                                .unwrap_or_else(|| PathBuf::from("~"))
                                .join(".config")
                        })
                        .join("powershell")
                        .join("Microsoft.PowerShell_profile.ps1")
                }
            }
        }
    }

    /// Get shell name as string
    pub fn name(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::PowerShell => "pwsh",
        }
    }
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for ShellType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(ShellType::Bash),
            "pwsh" | "powershell" => Ok(ShellType::PowerShell),
            _ => Err(format!("Unknown shell type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_name() {
        assert_eq!(ShellType::Bash.name(), "bash");
        assert_eq!(ShellType::PowerShell.name(), "pwsh");
    }

    #[test]
    fn test_shell_type_from_str() {
        assert_eq!("bash".parse::<ShellType>().unwrap(), ShellType::Bash);
        assert_eq!("pwsh".parse::<ShellType>().unwrap(), ShellType::PowerShell);
        assert_eq!(
            "powershell".parse::<ShellType>().unwrap(),
            ShellType::PowerShell
        );
    }

    #[test]
    fn test_default_config_path() {
        let bash_path = ShellType::Bash.default_config_path();
        assert!(bash_path.to_string_lossy().contains(".bashrc"));
    }
}
