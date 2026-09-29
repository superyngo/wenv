//! External editor integration

use std::io::Write;
use std::path::Path;

pub fn get_editor() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) {
            "notepad".into()
        } else {
            "vi".into()
        }
    })
}

/// Split an `$EDITOR` value into program + arguments (`code --wait`, `"my ed" -x`).
/// Understands single/double quotes; no shell expansion is performed.
pub fn split_command(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                in_token = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_token {
                    parts.push(std::mem::take(&mut cur));
                    in_token = false;
                }
            }
            (None, c) => {
                cur.push(c);
                in_token = true;
            }
        }
    }
    if in_token {
        parts.push(cur);
    }
    parts
}

/// Build a `Command` that opens `path` in `$EDITOR` (arguments in `$EDITOR` are honored).
pub fn editor_command(path: &Path) -> std::process::Command {
    let mut parts = split_command(&get_editor()).into_iter();
    let program = parts.next().unwrap_or_else(|| "vi".into());
    let mut cmd = std::process::Command::new(program);
    cmd.args(parts).arg(path);
    cmd
}

/// Launch editor on a file. Returns Ok(true) if the file was modified.
pub fn edit_file(path: &Path) -> anyhow::Result<bool> {
    let before = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

    let status = editor_command(path).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Editor exited with error"));
    }

    let after = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    Ok(before != after)
}

/// Write content to a temp file, edit it, read back.
/// Returns the new content, or None if user didn't save/change.
pub fn edit_temp_content(content: &str, suffix: &str) -> anyhow::Result<Option<String>> {
    let mut tmp = tempfile::Builder::new()
        .prefix(".wenv_edit_")
        .suffix(suffix)
        .tempfile()?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;

    let path = tmp.path().to_path_buf();
    let modified = edit_file(&path)?;

    if modified {
        let new_content = std::fs::read_to_string(&path)?;
        Ok(Some(new_content))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_command_handles_args_and_quotes() {
        assert_eq!(split_command("vim"), vec!["vim"]);
        assert_eq!(split_command("code --wait"), vec!["code", "--wait"]);
        assert_eq!(
            split_command("\"/opt/my ed/vim\" -u NONE"),
            vec!["/opt/my ed/vim", "-u", "NONE"]
        );
        assert_eq!(split_command("  "), Vec::<String>::new());
    }
}
