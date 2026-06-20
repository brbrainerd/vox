//! Application state + main event loop.
//!
//! `run()` is the entry-point for the TUI. It:
//! 1. Tries to enter raw mode (degrades gracefully under TERM=dumb/headless).
//! 2. Spins a `Session` and subscribes to `SessionEvent`s.
//! 3. Runs the crossterm event loop: key events → `InputBox` → `Session::submit`.
//! 4. Redraws the ratatui frame on each event.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use std::io;

use vox_terminal_core::session::Session;

use crate::{
    term_setup::TermSetup,
    theme::prompt_glyph,
    ui::{blocks, input::InputBox, palette::Palette},
    vt::VtGrid,
};

pub fn run() -> Result<()> {
    // Attempt raw mode; under dumb terminals this returns Err and we skip TUI.
    let _setup = TermSetup::new().ok();

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut session = Session::new("main");
    let mut input = InputBox::new();
    let mut grid = VtGrid::new(80, 24);
    let mut mode = "vox";

    loop {
        // Draw
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(frame.area());

            blocks::render(frame, chunks[0], session.blocks(), &mut grid);
            input.render(frame, chunks[1], mode);
        })?;

        // Input
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        match event::read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => break,
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                let _intent = input.submit();
                // TODO Track 4: dispatch intent through command registry / Session
            }
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => input.backspace(),
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) => input.push_char(c),
            _ => {}
        }
    }

    Ok(())
}
