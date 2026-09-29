//! Path utilities

use std::path::Path;

/// Check if a file path is writable.
/// For existing files: try opening for write.
/// For non-existent files: check parent directory is writable via metadata.
pub fn check_writable(path: &Path) -> bool {
    if path.exists() {
        std::fs::OpenOptions::new().write(true).open(path).is_ok()
    } else {
        path.parent().is_some_and(|p| {
            p.exists()
                && std::fs::metadata(p)
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false)
        })
    }
}

/// Returns true if the file at `path` appears to be text (no null bytes
/// in the first 8 KiB). Missing files or unreadable files return true so
/// the dir-expansion filter doesn't accidentally hide pending or transient
/// entries.
pub fn is_likely_text(path: &std::path::Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 8192];
    let n = f.read(&mut buf).unwrap_or(0);
    !buf[..n].contains(&0)
}

/// Probe whether `dir` is writable by attempting to create and delete a
/// unique temporary file. Used by Cache::cache_path_for fallback logic.
pub fn is_dir_writable(dir: &std::path::Path) -> bool {
    if !dir.exists() {
        return false;
    }
    let probe = dir.join(format!(".wenv-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

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
