use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

/// Input events for the TUI.
pub enum AppEvent {
    /// A key was pressed.
    Key(KeyEvent),
    /// A tick interval passed (for periodic refresh).
    Tick,
}

/// Poll for input events with a timeout (for tick-based refresh).
pub fn poll(tick_rate: Duration) -> Result<AppEvent> {
    if event::poll(tick_rate)? {
        if let Event::Key(key) = event::read()? {
            return Ok(AppEvent::Key(key));
        }
    }
    Ok(AppEvent::Tick)
}

/// Check if this key event is a quit signal.
pub fn is_quit(key: &KeyEvent) -> bool {
    matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        } | KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    )
}
