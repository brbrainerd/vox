//! Crossterm raw-mode lifecycle. `TermSetup` enters raw mode + alternate screen
//! on construction and restores state on drop — even on panic.
//!
//! Under a dumb terminal (CI, SSH without `TERM`) `new()` returns `Err`; the
//! caller should fall back to plain stdout rendering.

use std::io;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send<T: Send>() {}

    #[test]
    fn term_setup_is_send() {
        assert_send::<TermSetup>();
    }

    #[test]
    #[ignore = "requires interactive terminal session"]
    fn term_setup_new_and_drop_does_not_panic() {
        let _ = TermSetup::new();
    }
}

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

/// RAII guard: enters raw mode + alternate screen; restores both on drop.
pub struct TermSetup {
    _priv: (),
}

impl TermSetup {
    /// Enter raw mode and the alternate screen.
    ///
    /// Returns `Err` if the terminal does not support raw mode (e.g. `TERM=dumb`,
    /// pipe, CI). Callers should degrade gracefully.
    pub fn new() -> anyhow::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self { _priv: () })
    }
}

impl Drop for TermSetup {
    fn drop(&mut self) {
        // Best-effort restore — ignore errors (we may be panicking).
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
