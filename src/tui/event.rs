//! Event handling for TUI

use crossterm::event::{self, Event, KeyEvent};
use std::time::Duration;

/// Read a key event with timeout
pub fn read_key(timeout: Duration) -> std::io::Result<Option<KeyEvent>> {
    if event::poll(timeout)? {
        if let Event::Key(key) = event::read()? {
            return Ok(Some(key));
        }
    }
    Ok(None)
}
