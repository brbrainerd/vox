//! Theme constants — IBM Plex Mono Nerd Font glyphs and color palette.
//!
//! Font requirement: IBM Plex Mono Nerd Font (or any Nerd Font patched variant)
//! must be installed in the terminal emulator for powerline/glyph symbols to
//! render correctly. Falls back gracefully to ASCII under `TERM=dumb`.

use ratatui::style::Color;

// Powerline glyphs (Nerd Font codepoints)
pub const GLYPH_ARROW_RIGHT: &str = "\u{e0b0}"; //
pub const GLYPH_ARROW_LEFT: &str = "\u{e0b2}"; //
pub const GLYPH_BRANCH: &str = "\u{e0a0}"; //
pub const GLYPH_LOCK: &str = "\u{e0a2}"; //
pub const GLYPH_PROMPT: &str = "\u{276f}"; // ❯

// Color palette (Vox Axis "Limes" tokens adapted to ratatui)
pub const COLOR_BASALT: Color = Color::Rgb(0x1c, 0x1c, 0x1c);
pub const COLOR_TRAVERTINE: Color = Color::Rgb(0xf5, 0xf0, 0xe8);
pub const COLOR_GOLD: Color = Color::Rgb(0xc9, 0xa0, 0x2c);
pub const COLOR_VERDIGRIS: Color = Color::Rgb(0x4a, 0x9b, 0x8e);
pub const COLOR_ERROR: Color = Color::Rgb(0xd4, 0x4a, 0x3a);
pub const COLOR_SUCCESS: Color = Color::Rgb(0x5a, 0xb0, 0x6b);

/// Returns `true` when the terminal is known-dumb (no color/glyph support).
pub fn is_dumb_terminal() -> bool {
    std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false)
}

/// Prompt glyph — ASCII fallback under dumb terminals.
pub fn prompt_glyph() -> &'static str {
    if is_dumb_terminal() {
        ">"
    } else {
        GLYPH_PROMPT
    }
}
