//! `vox-term` — headless-capable ratatui TUI for vox-terminal-core.
//! Re-exported so integration tests (`tests/`) can access modules.

pub mod app;
pub mod term_setup;
pub mod theme;
pub mod vt;
pub mod ui {
    pub mod blocks;
    pub mod input;
    pub mod palette;
}
