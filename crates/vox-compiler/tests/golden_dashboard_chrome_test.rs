//! Golden tests for Phase 1 dashboard chrome components.
//!
//! Covers: TopBar, LeftRail (expanded + collapsed), StatusBar, Shell.
//! Each test compiles a minimal, self-contained source string that includes
//! the chrome component under test plus any helper components it calls
//! (NavIcon, NavItem, StatusSegment, StateChip).
//!
//! Assertions:
//!   1. The component produces a `.tsx` file in the output.
//!   2. Key structural assertions hold (JSX elements not JS calls; expected
//!      attribute names; expected content strings).
//!   3. A snapshot captures the full emitted TSX for visual review.
//!
//! VUV notes (current state after Phase 1 Batch 3):
//!   • `else if` chains are fully supported in all positions (parser fix landed).
//!   • `on` is a reserved keyword → callbacks named `on_click`, `on_select` etc.
//!   • Multiline component parameter lists do not parse — all `component Foo(a, b)`
//!     declarations must fit on one line.

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

// ── Shared helper sources ─────────────────────────────────────────────────────
// StateChip, NavIcon, NavItem, StatusSegment — inlined into tests that need them.
// Note: all component declarations use single-line parameter lists (VUV gap).

const STATE_CHIP_SRC: &str = r#"
component StateChip(status: str, label: str = "", dim: bool = false) {
    view: row(items="center", gap=1, pad_x=2, pad_y=0, radius="full", bg=if dim { "white/5" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400/15" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400/15" } else if (status is "error" or status is "failed" or status is "errored") { "rose.500/15" } else { "white/5" }, raw_class="h-5 inline-flex") {
        panel(w=1, h=1, radius="full", bg=if dim { "zinc.600" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" } else if (status is "error" or status is "failed" or status is "errored") { "rose.500" } else { "zinc.600" })
        text(size="xs", font_family="mono", tracking="widest", case="upper", color=if dim { "zinc.500" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" } else if (status is "error" or status is "failed" or status is "errored") { "rose.400" } else { "zinc.500" }) { if label is "" { status } else { label } }
    }
}
"#;

const NAV_ICON_SRC: &str = r#"
component NavIcon(name: str) {
    view: text(size="xs", font_family="mono", color="inherit", raw_class="w-4 h-4 flex items-center justify-center select-none") {
        if name is "speak"    { "◉" }
        else if name is "mesh"     { "◈" }
        else if name is "forge"    { "◧" }
        else if name is "code"     { "▤" }
        else if name is "models"   { "◎" }
        else if name is "runs"     { "▶" }
        else if name is "settings" { "⚙" }
        else if name is "search"   { "◌" }
        else { "·" }
    }
}
"#;

const NAV_ITEM_SRC: &str = r#"
component NavItem(label: str, surface_key: str, active: str, collapsed: bool) {
    view: button(raw_class=if collapsed { "flex items-center justify-center w-full rounded-lg py-2 transition-colors" } else { "flex items-center gap-3 w-full text-left rounded-lg px-3 py-2 transition-colors" }, bg=if active is surface_key { "white/8" } else { "transparent" }, color=if active is surface_key { "white" } else { "zinc.500" }, border=false) {
        NavIcon(name=surface_key)
        if collapsed {
            panel()
        } else {
            text(size="sm", color=if active is surface_key { "white/90" } else { "zinc.500" }) { label }
        }
    }
}
"#;

const STATUS_SEGMENT_SRC: &str = r#"
component StatusSegment(label: str, value: str, dot_color: str, highlighted: bool) {
    view: row(items="center", gap=1, raw_class=if highlighted { "opacity-100 cursor-pointer hover:opacity-90" } else { "opacity-60 cursor-pointer hover:opacity-80" }) {
        panel(w=1, h=1, radius="full", bg=dot_color, raw_class="shrink-0")
        text(size="xs", font_family="mono", color="zinc.400") { label }
        text(size="xs", font_family="mono", color="zinc.500") { value }
    }
}
"#;

// ── TopBar ────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn top_bar_emits_workspace_and_status_chip() {
    let source = format!(
        "{}{}{}",
        STATE_CHIP_SRC,
        NAV_ICON_SRC,
        r#"
component TopBar(workspace: str, run_status: str, run_label: str) {
    view: row(h=12, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0, bg="zinc.950") {
        button(raw_class="flex items-center gap-1.5 hover:bg-white/5 rounded-lg px-2 py-1 transition-colors", bg="transparent", color="zinc.300", border=false) {
            text(size="xs", weight="bold", color="zinc.400", tracking="widest", case="upper") { "Vox" }
            text(size="xs", color="zinc.700") { "/" }
            text(size="sm", color="zinc.300", font_family="mono") { workspace }
            text(size="xs", color="zinc.600", raw_class="ml-0.5") { "‹" }
        }
        button(raw_class="flex items-center gap-2 w-64", bg="zinc.900", border=true, border_color="white/10", radius="lg", pad_x=3, pad_y=1, color="zinc.500") {
            NavIcon(name="search")
            text(size="xs", color="zinc.600", raw_class="flex-1 text-left") { "Search or jump to…" }
            panel(border=true, border_color="white/15", radius="sm", pad_x=1, bg="white/5", raw_class="h-5 inline-flex items-center") {
                text(size="xs", font_family="mono", color="zinc.500") { "⌘K" }
            }
        }
        row(items="center", gap=3) {
            StateChip(status=run_status, label=run_label)
            panel(w=7, h=7, radius="full", bg="blue.600/20", border=true, border_color="blue.500/30", raw_class="flex items-center justify-center") {
                text(size="xs", weight="bold", color="blue.400", font_family="mono") { "AU" }
            }
        }
    }
}
"#
    );

    let ts = compile_component(&source, "TopBar");
    println!("TopBar.tsx:\n{ts}");

    // Must emit JSX elements, not JS function calls.
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");
    assert!(!ts.contains("button("), "button must not emit as JS call");

    // TopBar must render <button> elements.
    assert!(ts.contains("<button"), "TopBar must emit <button> element");

    // StateChip JSX call must be present.
    assert!(
        ts.contains("StateChip"),
        "TopBar must include StateChip component"
    );

    // ⌘K hint must appear.
    assert!(ts.contains("⌘K"), "TopBar must include ⌘K shortcut hint");

    insta::assert_snapshot!("top_bar_tsx_dashboard_chrome", ts);
}

// ── LeftRail (expanded) ───────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn left_rail_expanded_emits_all_nav_items() {
    let source = format!(
        "{}{}{}",
        NAV_ICON_SRC,
        NAV_ITEM_SRC,
        r#"
component LeftRail(active: str, collapsed: bool) {
    view: column(raw_class=if collapsed { "w-14 shrink-0" } else { "w-[200px] shrink-0" }, border_r=true, border_color="zinc.800", pad=2, gap=1, bg="zinc.950") {
        row(h=12, items="center", shrink=0, raw_class=if collapsed { "justify-center px-1" } else { "gap-2 px-2" }) {
            panel(w=6, h=6, radius="md", bg="blue.600", raw_class="flex items-center justify-center shrink-0") {
                text(size="xs", weight="bold", color="white", font_family="mono") { "V" }
            }
            if collapsed {
                panel()
            } else {
                text(size="sm", weight="bold", color="white/90", tracking="tight") { "Vox" }
            }
        }
        NavItem(label="Speak",    surface_key="speak",    active=active, collapsed=collapsed)
        NavItem(label="Mesh",     surface_key="mesh",     active=active, collapsed=collapsed)
        NavItem(label="Forge",    surface_key="forge",    active=active, collapsed=collapsed)
        NavItem(label="Code",     surface_key="code",     active=active, collapsed=collapsed)
        NavItem(label="Models",   surface_key="models",   active=active, collapsed=collapsed)
        NavItem(label="Runs",     surface_key="runs",     active=active, collapsed=collapsed)
        panel(flex=1)
        NavItem(label="Settings", surface_key="settings", active=active, collapsed=collapsed)
        if collapsed {
            panel()
        } else {
            row(pad_x=2, pad_y=2, shrink=0) {
                text(size="xs", font_family="mono", color="zinc.700") { "v0.14.2 · main" }
            }
        }
    }
}
"#
    );

    let ts = compile_component(&source, "LeftRail");
    println!("LeftRail expanded.tsx:\n{ts}");

    // Must emit JSX elements, not JS function calls.
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");

    // NavItem invocations must appear.
    assert!(
        ts.contains("NavItem"),
        "LeftRail must include NavItem component"
    );

    // All 7 English nav labels must be present as prop values.
    assert!(ts.contains("Speak"), "LeftRail must show Speak label");
    assert!(ts.contains("Mesh"), "LeftRail must show Mesh label");
    assert!(ts.contains("Forge"), "LeftRail must show Forge label");
    assert!(ts.contains("Code"), "LeftRail must show Code label");
    assert!(ts.contains("Models"), "LeftRail must show Models label");
    assert!(ts.contains("Runs"), "LeftRail must show Runs label");
    assert!(ts.contains("Settings"), "LeftRail must show Settings label");

    // No Latin labels — the principal IA fix this batch ships.
    assert!(
        !ts.contains("LOQUELA"),
        "LeftRail must NOT contain Latin label LOQUELA"
    );
    assert!(
        !ts.contains("RETE"),
        "LeftRail must NOT contain Latin label RETE"
    );
    assert!(
        !ts.contains("FABRICA"),
        "LeftRail must NOT contain Latin label FABRICA"
    );
    assert!(
        !ts.contains("IMPERIUM"),
        "LeftRail must NOT contain Latin label IMPERIUM"
    );

    // Version label present in expanded mode source.
    assert!(
        ts.contains("v0.14.2"),
        "LeftRail must include version label"
    );

    insta::assert_snapshot!("left_rail_expanded_tsx_dashboard_chrome", ts);
}

// ── LeftRail (collapsed) ──────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn left_rail_collapsed_emits_narrow_class() {
    // Identical source to the expanded test — we verify that BOTH width branches
    // compile into the conditional expression (the collapsed=true runtime path
    // is exercised at runtime; here we confirm the compiler emits both branches).
    let source = format!(
        "{}{}{}",
        NAV_ICON_SRC,
        NAV_ITEM_SRC,
        r#"
component LeftRail(active: str, collapsed: bool) {
    view: column(raw_class=if collapsed { "w-14 shrink-0" } else { "w-[200px] shrink-0" }, border_r=true, border_color="zinc.800", pad=2, gap=1, bg="zinc.950") {
        row(h=12, items="center", shrink=0, raw_class=if collapsed { "justify-center px-1" } else { "gap-2 px-2" }) {
            panel(w=6, h=6, radius="md", bg="blue.600", raw_class="flex items-center justify-center shrink-0") {
                text(size="xs", weight="bold", color="white", font_family="mono") { "V" }
            }
            if collapsed {
                panel()
            } else {
                text(size="sm", weight="bold", color="white/90", tracking="tight") { "Vox" }
            }
        }
        NavItem(label="Speak",    surface_key="speak",    active=active, collapsed=collapsed)
        NavItem(label="Mesh",     surface_key="mesh",     active=active, collapsed=collapsed)
        NavItem(label="Forge",    surface_key="forge",    active=active, collapsed=collapsed)
        NavItem(label="Code",     surface_key="code",     active=active, collapsed=collapsed)
        NavItem(label="Models",   surface_key="models",   active=active, collapsed=collapsed)
        NavItem(label="Runs",     surface_key="runs",     active=active, collapsed=collapsed)
        panel(flex=1)
        NavItem(label="Settings", surface_key="settings", active=active, collapsed=collapsed)
        if collapsed {
            panel()
        } else {
            row(pad_x=2, pad_y=2, shrink=0) {
                text(size="xs", font_family="mono", color="zinc.700") { "v0.14.2 · main" }
            }
        }
    }
}
"#
    );

    let ts = compile_component(&source, "LeftRail");
    println!("LeftRail collapsed branch.tsx:\n{ts}");

    // Both conditional width classes must appear in the emitted TSX.
    assert!(
        ts.contains("w-14"),
        "collapsed LeftRail must have w-14 narrow class"
    );
    assert!(
        ts.contains("w-[200px]"),
        "expanded LeftRail must have w-[200px] class"
    );

    // The collapsed layout class for the header row must appear.
    assert!(
        ts.contains("justify-center"),
        "collapsed LeftRail header row must emit justify-center"
    );

    insta::assert_snapshot!("left_rail_collapsed_tsx_dashboard_chrome", ts);
}

// ── StatusBar ─────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn status_bar_active_mesh_emits_segments() {
    let source = format!(
        "{}{}",
        STATUS_SEGMENT_SRC,
        r#"
component StatusBar(active: str) {
    let mesh_nodes  = "12"
    let queue_count = "3"
    let error_count = "2"
    let model_name  = "sonnet-4.6"
    let build_state = "idle"
    let cost_used   = "$4.82"
    let cost_cap    = "$50.00"
    let utc_time    = "UTC 14:32"

    view: row(raw_class="h-[26px] shrink-0", border_t=true, border_color="zinc.800", pad_x=4, items="center", justify="between", bg="zinc.950") {
        row(items="center", gap=3) {
            StatusSegment(label="mesh",   value=mesh_nodes,  dot_color="emerald.400", highlighted=if active is "mesh"   { true } else { false })
            text(size="xs", color="zinc.700") { "·" }
            StatusSegment(label="queue",  value=queue_count, dot_color="emerald.400", highlighted=if active is "runs"   { true } else { false })
            text(size="xs", color="zinc.700") { "·" }
            StatusSegment(label="errors", value=error_count, dot_color="rose.500",    highlighted=if active is "code"   { true } else { false })
            text(size="xs", color="zinc.700") { "·" }
            StatusSegment(label="model",  value=model_name,  dot_color="emerald.400", highlighted=if active is "models" { true } else { false })
            text(size="xs", color="zinc.700") { "·" }
            StatusSegment(label="build",  value=build_state, dot_color="zinc.600",    highlighted=if active is "forge"  { true } else { false })
        }
        row(items="center", gap=2) {
            text(size="xs", font_family="mono", color="zinc.600") { "cost" }
            text(size="xs", color="zinc.700") { "·" }
            text(size="xs", font_family="mono", color="zinc.500") { cost_used }
            text(size="xs", color="zinc.700") { "/" }
            text(size="xs", font_family="mono", color="zinc.600") { cost_cap }
            text(size="xs", color="zinc.700") { "·" }
            text(size="xs", font_family="mono", color="zinc.600") { "24h" }
            text(size="xs", color="zinc.700") { "·" }
            text(size="xs", font_family="mono", color="zinc.600") { utc_time }
        }
    }
}
"#
    );

    let ts = compile_component(&source, "StatusBar");
    println!("StatusBar.tsx:\n{ts}");

    // Must emit JSX elements, not JS function calls.
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");

    // StatusSegment component call must appear.
    assert!(
        ts.contains("StatusSegment"),
        "StatusBar must include StatusSegment"
    );

    // Static placeholder values must be present.
    assert!(
        ts.contains("sonnet-4.6"),
        "StatusBar must show default model name"
    );
    assert!(ts.contains("$4.82"), "StatusBar must show cost placeholder");
    assert!(
        ts.contains("UTC 14:32"),
        "StatusBar must show UTC time placeholder"
    );

    // rose.500 dot for errors must appear.
    assert!(
        ts.contains("rose"),
        "StatusBar must have rose color for error segment"
    );

    insta::assert_snapshot!("status_bar_tsx_dashboard_chrome", ts);
}

// ── Shell ─────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn shell_wraps_top_bar_left_rail_status_bar() {
    // Minimal inline versions of the chrome sub-components so Shell can resolve
    // its component calls without needing cross-file import resolution.
    let source = format!(
        "{}{}{}{}{}{}",
        STATE_CHIP_SRC,
        NAV_ICON_SRC,
        NAV_ITEM_SRC,
        STATUS_SEGMENT_SRC,
        r#"
component TopBar(workspace: str, run_status: str, run_label: str) {
    view: row(h=12, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0, bg="zinc.950") {
        text(size="sm", color="zinc.300", font_family="mono") { workspace }
        StateChip(status=run_status, label=run_label)
    }
}

component LeftRail(active: str, collapsed: bool) {
    view: column(raw_class=if collapsed { "w-14 shrink-0" } else { "w-[200px] shrink-0" }, border_r=true, border_color="zinc.800", pad=2, gap=1, bg="zinc.950") {
        NavItem(label="Mesh", surface_key="mesh", active=active, collapsed=collapsed)
    }
}

component StatusBar(active: str) {
    let model_name = "sonnet-4.6"
    view: row(raw_class="h-[26px] shrink-0", border_t=true, border_color="zinc.800", pad_x=4, items="center", bg="zinc.950") {
        StatusSegment(label="model", value=model_name, dot_color="emerald.400", highlighted=if active is "models" { true } else { false })
    }
}
"#,
        r#"
component Shell(active: str, rail_collapsed: bool, workspace: str, run_status: str, run_label: str) {
    view: column(min_h="screen", overflow="hidden", bg="zinc.950", color="zinc.100") {
        TopBar(workspace=workspace, run_status=run_status, run_label=run_label)
        row(flex=1, min_h=0, overflow="hidden") {
            LeftRail(active=active, collapsed=rail_collapsed)
            panel(flex=1, min_w=0, overflow="hidden", bg="zinc.950")
        }
        StatusBar(active=active)
    }
}
"#
    );

    let ts = compile_component(&source, "Shell");
    println!("Shell.tsx:\n{ts}");

    // Must emit JSX elements, not JS function calls.
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");

    // Three chrome component calls must appear as JSX.
    assert!(ts.contains("TopBar"), "Shell must include TopBar component");
    assert!(
        ts.contains("LeftRail"),
        "Shell must include LeftRail component"
    );
    assert!(
        ts.contains("StatusBar"),
        "Shell must include StatusBar component"
    );

    // Shell is a column layout — min-h-screen must be present.
    assert!(
        ts.contains("screen") || ts.contains("min-h"),
        "Shell must have min-h-screen layout class"
    );

    insta::assert_snapshot!("shell_tsx_dashboard_chrome", ts);
}
