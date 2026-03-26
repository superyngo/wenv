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

/// Launch editor on a file. Returns Ok(true) if the file was modified.
pub fn edit_file(path: &Path) -> anyhow::Result<bool> {
    let before = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());

    let editor = get_editor();
    let status = std::process::Command::new(&editor).arg(path).status()?;
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
