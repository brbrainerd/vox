//! Application state + main event loop.
//!
//! `run()` is the entry-point for the TUI. It:
//! 1. Tries to enter raw mode (degrades gracefully under TERM=dumb/headless).
//! 2. Spins a `Session` and subscribes to `SessionEvent`s.
//! 3. Runs the crossterm event loop: key events → `InputBox` → `Session::submit`.
//! 4. Drains `SessionEvent`s each tick so agent streams repaint live.
//! 5. Redraws the ratatui frame on each tick.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::io;
use tokio::sync::broadcast::error::TryRecvError;

use vox_terminal_core::session::{Session, SessionEvent};

use crate::{
    term_setup::TermSetup,
    ui::{blocks, input::InputBox},
    vt::VtGrid,
};

/// Entry-point for the TUI. Headless-safe: degrades to plain stdout under TERM=dumb.
pub fn run() -> Result<()> {
    // Attempt raw mode; under dumb terminals this returns Err and we skip TUI.
    let _setup = TermSetup::new().ok();

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let session = Session::new("main");
    // Subscribe before the loop so we don't miss early events.
    let mut session_rx = session.subscribe();
    let mut input = InputBox::new();
    let mut grid = VtGrid::new(80, 24);
    let mode = "vox";

    // Agent strip: accumulates streamed agent messages for display below the block list.
    let mut agent_lines: Vec<String> = Vec::new();

    loop {
        // Drain any buffered SessionEvents (non-blocking).
        loop {
            match session_rx.try_recv() {
                Ok(SessionEvent::AgentMessage { text }) => {
                    agent_lines.push(text);
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Lagged(_)) => break,
                Err(TryRecvError::Closed) => break,
            }
        }

        // Draw
        terminal.draw(|frame| {
            let area = frame.area();
            // Layout: block list | agent strip (if any) | input bar
            let has_agent = !agent_lines.is_empty();
            let constraints = if has_agent {
                vec![
                    Constraint::Min(1),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ]
            } else {
                vec![Constraint::Min(1), Constraint::Length(1)]
            };
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);

            blocks::render(frame, chunks[0], session.blocks(), &mut grid);

            if has_agent {
                let tail: Vec<Line> = agent_lines
                    .iter()
                    .rev()
                    .take(3)
                    .rev()
                    .map(|t| {
                        Line::from(vec![
                            Span::styled("AI ", Style::default().fg(Color::Cyan)),
                            Span::raw(t.clone()),
                        ])
                    })
                    .collect();
                frame.render_widget(Paragraph::new(tail), chunks[1]);
                input.render(frame, chunks[2], mode);
            } else {
                input.render(frame, chunks[1], mode);
            }
        })?;

        // Poll for input events (50 ms tick = ~20 fps).
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
                // Track 4: dispatch intent through command registry / Session::submit
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
