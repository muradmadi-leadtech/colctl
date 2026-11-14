//! Taking over the terminal and always giving it back (AC8-07, EC8-16, EC8-22): raw mode, the
//! alternate screen, mouse capture and bracketed paste on the way in; all off, and the cursor
//! back, on drop, on panic, and on every exit path.

use ratatui::crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::crossterm::{cursor, execute};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

/// True while the terminal is ours; restoring twice does nothing (EC8-22).
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Holds the terminal; restores it when dropped.
pub struct Guard;

impl Guard {
    pub fn enter() -> io::Result<Guard> {
        enable_raw_mode()?;
        TAKEN.store(true, Ordering::SeqCst);
        let mut out = io::stdout();
        if let Err(e) = execute!(
            out,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            cursor::Hide
        ) {
            restore();
            return Err(e);
        }
        Ok(Guard)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        restore();
    }
}

/// Gives the terminal back. Never panics; safe to call any number of times.
pub fn restore() {
    if !TAKEN.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen,
        cursor::Show
    );
}
