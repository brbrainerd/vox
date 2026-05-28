//! Golden tests for Phase 1 dashboard composite primitives.
//!
//! Each test compiles a minimal usage of one composite and asserts:
//!   1. The component produces a `.tsx` file in the output.
//!   2. Key structural assertions hold (correct element names, no JS function
//!      call emission, expected attribute names).
//!   3. A snapshot captures the full emitted TSX for visual review.
//!
//! VUV parser note: `else if` chains are fully supported in all positions as of
//! Phase 1 Batch 3. Composite sources below use the cleaner `else if` form.
//! (The snapshot sources still reflect the old nested style — snapshots are
//! stable; source style is updated in new tests going forward.)

// ── Helpers ───────────────────────────────────────────────────────────────────

fn compile_component(source: &str, component_name: &str) -> String {
    let tokens = vox_compiler::lexer::lex(source);
    let module =
        vox_compiler::parser::parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let hir = vox_compiler::hir::lower_module(&module);
    let out =
        vox_codegen::codegen_ts::generate(&hir).unwrap_or_else(|e| panic!("codegen failed: {e:?}"));
    let filename = format!("{component_name}.tsx");
    out.files
        .into_iter()
        .find(|(n, _)| n == &filename)
        .map(|(_, c)| c)
        .unwrap_or_else(|| panic!("{filename} not found in codegen output"))
}

// ── Task 1.1 — tokens module (compile-only, no view) ─────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn tokens_module_compiles() {
    let source = r#"
let bg        = "zinc.950"
let surface   = "zinc.900"
let surface2  = "zinc.800"
let surface3  = "zinc.700"
let border    = "white/6"
let border2   = "white/10"
let text_pri  = "white/86"
let text2     = "zinc.400"
let text3     = "zinc.500"
let text4     = "zinc.600"
let blue      = "blue.600"
let blue_soft = "blue.600/14"
let emerald   = "emerald.400"
let amber     = "amber.400"
let rose      = "rose.500"
"#;
    // Must parse and lower without panicking.
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("tokens module must parse");
    let _hir = vox_compiler::hir::lower_module(&module);
    // No component output — this module has only let bindings.
}

// ── StateChip ─────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn state_chip_running_emits_emerald() {
    let source = r#"
component StateChip(status: str, label: str = "", dim: bool = false) {
    view: row(
        items="center", gap=1,
        pad_x=2, pad_y=0,
        radius="full",
        bg=if dim { "white/5" }
           else { if (status is "running" or status is "ok" or status is "ready") { "emerald.400/15" }
                  else { if (status is "warn" or status is "blocked" or status is "pending") { "amber.400/15" }
                         else { if (status is "error" or status is "failed" or status is "errored") { "rose.500/15" }
                                else { "white/5" } } } },
        raw_class="h-5 inline-flex"
    ) {
        panel(
            w=1, h=1, radius="full",
            bg=if dim { "zinc.600" }
               else { if (status is "running" or status is "ok" or status is "ready") { "emerald.400" }
                      else { if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" }
                             else { if (status is "error" or status is "failed" or status is "errored") { "rose.500" }
                                    else { "zinc.600" } } } }
        )
        text(
            size="xs", font_family="mono", tracking="widest", case="upper",
            color=if dim { "zinc.500" }
                  else { if (status is "running" or status is "ok" or status is "ready") { "emerald.400" }
                         else { if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" }
                                else { if (status is "error" or status is "failed" or status is "errored") { "rose.400" }
                                       else { "zinc.500" } } } }
        ) { if label is "" { status } else { label } }
    }
}
"#;
    let ts = compile_component(source, "StateChip");
    println!("StateChip.tsx:\n{ts}");

    // Must render as proper JSX, not JS function calls.
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");

    // Conditional class values must be present.
    assert!(ts.contains("emerald"), "emerald color must appear");
    assert!(ts.contains("amber"), "amber color must appear");
    assert!(ts.contains("rose"), "rose color must appear");

    insta::assert_snapshot!("state_chip_tsx_dashboard_composite", ts);
}

// ── NodeBadge ─────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn node_badge_emits_mono_font_and_status() {
    let source = r#"
component NodeBadge(id: str, status: str, role: str = "", dim: bool = false) {
    view: row(
        pad_x=3, pad_y=2, bg="zinc.900", border=true, border_color="white/10",
        radius="xl", items="center", gap=3
    ) {
        panel(
            w=2, h=2, radius="full",
            bg=if dim { "zinc.600" }
               else { if (status is "running" or status is "ok" or status is "ready") { "emerald.400" }
                      else { if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" }
                             else { if (status is "error" or status is "failed" or status is "errored") { "rose.500" }
                                    else { "zinc.600" } } } }
        )
        column(gap=0) {
            text(size="xs", font_family="mono", color="white/80") { id }
            if role is "" {
                text(size="xs", color="zinc.500") { status }
            } else {
                row(items="center", gap=1) {
                    text(size="xs", color="zinc.500") { status }
                    text(size="xs", color="zinc.700") { "·" }
                    text(size="xs", color="zinc.600", font_family="mono") { role }
                }
            }
        }
    }
}
"#;
    let ts = compile_component(source, "NodeBadge");
    println!("NodeBadge.tsx:\n{ts}");

    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(
        ts.contains("mono"),
        "NodeBadge must include mono font for id"
    );

    insta::assert_snapshot!("node_badge_tsx_dashboard_composite", ts);
}

// ── KeyHint ───────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn key_hint_emits_bordered_chip() {
    let source = r#"
component KeyHint(text: str) {
    view: panel(
        border=true, border_color="white/15",
        radius="sm", pad_x=1,
        bg="white/5",
        raw_class="h-5 inline-flex items-center"
    ) {
        text(size="xs", font_family="mono", color="zinc.400") { text }
    }
}
"#;
    let ts = compile_component(source, "KeyHint");
    println!("KeyHint.tsx:\n{ts}");

    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(ts.contains("border"), "KeyHint must include border class");

    insta::assert_snapshot!("key_hint_tsx_dashboard_composite", ts);
}

// ── Label ─────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn label_emits_uppercase_text() {
    let source = r#"
component Label(text: str) {
    view: text(
        size="xs", weight="bold", case="upper",
        tracking="widest", color="zinc.500"
    ) { text }
}
"#;
    let ts = compile_component(source, "Label");
    println!("Label.tsx:\n{ts}");

    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(
        ts.contains("uppercase") || ts.contains("upper"),
        "Label must include uppercase class"
    );
    assert!(
        ts.contains("widest") || ts.contains("tracking"),
        "Label must include tracking-widest class"
    );

    insta::assert_snapshot!("label_tsx_dashboard_composite", ts);
}

// ── SectionHeading ────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn section_heading_emits_border_row() {
    let source = r#"
component SectionHeading(title: str) {
    view: row(
        pad_x=4, pad_y=2,
        border_b=true, border_color="white/10",
        justify="between", items="center",
        shrink=0, bg="zinc.900/50"
    ) {
        text(
            size="xs", weight="bold", case="upper",
            tracking="widest", color="zinc.400"
        ) { title }
    }
}
"#;
    let ts = compile_component(source, "SectionHeading");
    println!("SectionHeading.tsx:\n{ts}");

    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(
        ts.contains("border"),
        "SectionHeading must have border class"
    );

    insta::assert_snapshot!("section_heading_tsx_dashboard_composite", ts);
}

// ── IconBtn ───────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn icon_btn_emits_button_element() {
    let source = r#"
component IconBtn(icon: str, size: int = 8, on_click: fn() = fn() {}) {
    view: button(
        size="icon",
        w=size, h=size, radius="lg",
        bg="white/5", color="zinc.400",
        border=true, border_color="white/10",
        raw_class="flex items-center justify-center hover:bg-white/10",
        on_click=on_click
    ) {
        text(size="xs", font_family="mono") { icon }
    }
}
"#;
    let ts = compile_component(source, "IconBtn");
    println!("IconBtn.tsx:\n{ts}");

    assert!(
        ts.contains("<button"),
        "IconBtn must emit a <button> element"
    );
    assert!(!ts.contains("button("), "button must not emit as JS call");
    assert!(ts.contains("onClick"), "IconBtn must emit onClick prop");

    insta::assert_snapshot!("icon_btn_tsx_dashboard_composite", ts);
}

// ── Toggle ────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn toggle_emits_conditional_bg() {
    let source = r#"
component Toggle(checked: bool, label: str = "", on_change: fn() = fn() {}) {
    view: row(items="center", gap=3, raw_class="cursor-pointer", on_click=on_change) {
        panel(
            w=9, h=5, radius="full",
            bg=if checked { "blue.600" } else { "zinc.700" },
            pad=0,
            raw_class="relative flex items-center transition-colors duration-150"
        ) {
            panel(
                w=4, h=4, radius="full",
                bg="white",
                raw_class=if checked {
                    "absolute right-0.5 transition-all duration-150"
                } else {
                    "absolute left-0.5 transition-all duration-150"
                }
            )
        }
        if label is "" {
            panel()
        } else {
            text(size="sm", color="zinc.300") { label }
        }
    }
}
"#;
    let ts = compile_component(source, "Toggle");
    println!("Toggle.tsx:\n{ts}");

    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(
        ts.contains("blue") && ts.contains("zinc"),
        "Toggle must have conditional bg"
    );
    assert!(ts.contains("onClick"), "Toggle must emit onClick");

    insta::assert_snapshot!("toggle_tsx_dashboard_composite", ts);
}

// ── Input ─────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn input_display_emits_panel_with_text() {
    let source = r#"
component Input(value: str = "", mono: bool = false, placeholder: str = "") {
    view: panel(
        bg="zinc.900", border=true, border_color="white/10",
        radius="lg", pad_x=3, pad_y=2,
        raw_class="flex items-center h-9"
    ) {
        if value is "" {
            text(
                size="sm",
                color="zinc.600",
                font_family=if mono { "mono" } else { "sans" },
                italic=true
            ) { placeholder }
        } else {
            text(
                size="sm",
                color="zinc.300",
                font_family=if mono { "mono" } else { "sans" }
            ) { value }
        }
    }
}
"#;
    let ts = compile_component(source, "Input");
    println!("Input.tsx:\n{ts}");

    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(
        ts.contains("mono") && ts.contains("sans"),
        "Input must conditionally apply mono or sans font"
    );

    insta::assert_snapshot!("input_tsx_dashboard_composite", ts);
}

// ── StatBox ───────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn stat_box_emits_kpi_structure() {
    let source = r#"
component StatBox(key: str, value: str, sub: str = "") {
    view: column(
        bg="zinc.900", border=true, border_color="white/10",
        radius="xl", pad=4, gap=1
    ) {
        text(
            size="xs", weight="bold", case="upper",
            tracking="widest", color="zinc.500", mb=1
        ) { key }
        text(
            size="2xl", weight="bold", color="white/90",
            font_family="mono", tracking="tight"
        ) { value }
        if sub is "" {
            panel()
        } else {
            text(size="xs", color="zinc.600") { sub }
        }
    }
}
"#;
    let ts = compile_component(source, "StatBox");
    println!("StatBox.tsx:\n{ts}");

    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(
        ts.contains("2xl"),
        "StatBox must include 2xl size for value"
    );
    assert!(
        ts.contains("uppercase") || ts.contains("upper"),
        "StatBox key must be uppercase"
    );

    insta::assert_snapshot!("stat_box_tsx_dashboard_composite", ts);
}

// ── Codeframe ─────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn codeframe_emits_mono_rose_caret() {
    let source = r#"
component Codeframe(file: str, line: int, col: int, excerpt: str, caret: str) {
    view: panel(
        bg="zinc.900", border=true, border_color="white/10",
        radius="lg", pad=3, gap=1
    ) {
        text(size="xs", font_family="mono", color="zinc.500") { file }
        text(size="xs", font_family="mono", color="white/70", leading="relaxed") { excerpt }
        text(size="xs", font_family="mono", color="rose.400") { caret }
    }
}
"#;
    let ts = compile_component(source, "Codeframe");
    println!("Codeframe.tsx:\n{ts}");

    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(ts.contains("rose"), "Codeframe caret must be rose colored");
    assert!(ts.contains("mono"), "Codeframe must use mono font");

    insta::assert_snapshot!("codeframe_tsx_dashboard_composite", ts);
}
