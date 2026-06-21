/// Task 2.2 — VT-grid renderer: alacritty_terminal wrapping.
use vox_term::vt::VtGrid;

#[test]
fn ansi_color_produces_styled_span() {
    let mut grid = VtGrid::new(80, 24);
    grid.feed(b"\x1b[31mhi\x1b[0m");
    let lines = grid.render_lines();
    // At least one span should contain "hi"
    let text: String = lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.as_ref())
        .collect();
    assert!(text.contains("hi"), "rendered text: {text:?}");
    // The span containing "hi" should be styled red
    let red_span = lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .find(|s| s.content.contains("hi"));
    assert!(red_span.is_some(), "no span contains 'hi'");
    use ratatui::style::Color;
    let style = red_span.unwrap().style;
    assert_eq!(style.fg, Some(Color::Red), "expected red fg, got {style:?}");
}

#[test]
fn alternate_screen_does_not_panic() {
    let mut grid = VtGrid::new(80, 24);
    // Alternate screen enter + exit should not panic
    grid.feed(b"\x1b[?1049h");
    grid.feed(b"hello\r\n");
    grid.feed(b"\x1b[?1049l");
    let _ = grid.render_lines();
}
