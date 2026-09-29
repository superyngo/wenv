//! Shared caret editing (kill/yank/word motion) for the TUI's single-line text inputs

/// Editing operation beyond plain typing, shared by filter, path prompt and inline editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEditOp {
    /// Delete from caret to start of line (`Ctrl+U`).
    KillToStart,
    /// Delete from caret to end of line (`Ctrl+K`).
    KillToEnd,
    /// Delete the word before the caret (`Ctrl+W`).
    DeleteWordBack,
    /// Move caret one word left (`Alt+B` / `Ctrl+←`).
    WordLeft,
    /// Move caret one word right (`Alt+F` / `Ctrl+→`).
    WordRight,
    /// Insert the last killed text at the caret (`Ctrl+Y`).
    Yank,
}

fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn word_start(chars: &[char], mut i: usize) -> usize {
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

fn word_end(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Apply `op` to `buf` with a char-indexed `cursor`. Killed text is stored in `kill_ring`.
pub fn apply(op: LineEditOp, buf: &mut String, cursor: &mut usize, kill_ring: &mut String) {
    let chars: Vec<char> = buf.chars().collect();
    let cur = (*cursor).min(chars.len());
    match op {
        LineEditOp::KillToStart => {
            *kill_ring = chars[..cur].iter().collect();
            *buf = chars[cur..].iter().collect();
            *cursor = 0;
        }
        LineEditOp::KillToEnd => {
            *kill_ring = chars[cur..].iter().collect();
            *buf = chars[..cur].iter().collect();
        }
        LineEditOp::DeleteWordBack => {
            let start = word_start(&chars, cur);
            *kill_ring = chars[start..cur].iter().collect();
            *buf = chars[..start].iter().chain(&chars[cur..]).collect();
            *cursor = start;
        }
        LineEditOp::WordLeft => *cursor = word_start(&chars, cur),
        LineEditOp::WordRight => *cursor = word_end(&chars, cur),
        LineEditOp::Yank => {
            let at = byte_at(buf, cur);
            buf.insert_str(at, kill_ring);
            *cursor = cur + kill_ring.chars().count();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(op: LineEditOp, text: &str, cur: usize) -> (String, usize, String) {
        let mut buf = text.to_string();
        let mut c = cur;
        let mut ring = String::from("Z");
        apply(op, &mut buf, &mut c, &mut ring);
        (buf, c, ring)
    }

    #[test]
    fn test_kill_and_yank() {
        assert_eq!(
            run(LineEditOp::KillToStart, "ab cd", 2),
            (" cd".into(), 0, "ab".into())
        );
        assert_eq!(
            run(LineEditOp::KillToEnd, "ab cd", 2),
            ("ab".into(), 2, " cd".into())
        );
        assert_eq!(
            run(LineEditOp::Yank, "ab", 1),
            ("aZb".into(), 2, "Z".into())
        );
    }

    #[test]
    fn test_word_motion_and_delete_multibyte() {
        assert_eq!(
            run(LineEditOp::DeleteWordBack, "ls 日本 x", 6),
            ("ls x".into(), 3, "日本 ".into())
        );
        assert_eq!(run(LineEditOp::WordLeft, "aa bb", 5).1, 3);
        assert_eq!(run(LineEditOp::WordRight, "aa bb", 0).1, 2);
    }
}
