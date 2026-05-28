//! Golden tests for the Phase 2 Mesh surface components.
//!
//! Verifies that:
//!   1. MeshSummaryBar compiles to JSX with 6 KPI chips (no JS function calls).
//!   2. AgentNode and OrchNode emit SVG elements with conditional stroke.
//!   3. MeshTopology SVG canvas contains the expected edges and all 7 nodes.
//!   4. ActivityStrip emits SVG rect elements (bar chart).
//!
//! Sources are inlined (no cross-file import resolution in the test harness).

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

// ── Shared sources ────────────────────────────────────────────────────────────

const STATE_CHIP_SRC: &str = r#"
component StateChip(status: str, label: str = "", dim: bool = false) {
    view: row(items="center", gap=1, pad_x=2, pad_y=0, radius="full", bg=if dim { "white/5" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400/15" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400/15" } else if (status is "error" or status is "failed" or status is "errored") { "rose.500/15" } else { "white/5" }, raw_class="h-5 inline-flex") {
        panel(w=1, h=1, radius="full", bg=if dim { "zinc.600" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" } else if (status is "error" or status is "failed" or status is "errored") { "rose.500" } else { "zinc.600" })
        text(size="xs", font_family="mono", tracking="widest", case="upper", color=if dim { "zinc.500" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" } else if (status is "error" or status is "failed" or status is "errored") { "rose.400" } else { "zinc.500" }) { if label is "" { status } else { label } }
    }
}
"#;

const MESH_KPI_SRC: &str = r#"
component MeshKpi(key: str, value: str, sub: str = "") {
    view: column(pad_x=4, pad_y=2, gap=0, shrink=0) {
        text(size="xs", weight="bold", case="upper", tracking="widest", color="zinc.500") { key }
        text(size="xl", weight="bold", color="white/90", font_family="mono") { value }
        if sub is "" { panel(h=4) } else { text(size="xs", color="zinc.600") { sub } }
    }
}
"#;

const AGENT_NODE_SRC: &str = r##"
component AgentNode(cx: int, cy: int, id: str, selected: str, on_click: fn() = fn() {}) {
    view: circle(cx=cx, cy=cy, r=20, fill="#27272a", stroke=if selected is id { "#3b82f6" } else { "#52525b" }, stroke_width=if selected is id { 2 } else { 1 }, raw_class="cursor-pointer", on_click=on_click)
}
"##;

const ORCH_NODE_SRC: &str = r##"
component OrchNode(points: str, id: str, selected: str, orch_stroke: str, on_click: fn() = fn() {}) {
    view: polygon(points=points, fill="#0c1631", stroke=if selected is id { "#60a5fa" } else { orch_stroke }, stroke_width=if selected is id { 2 } else { 1.5 }, raw_class="cursor-pointer", on_click=on_click)
}
"##;

// ── MeshSummaryBar ────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn mesh_summary_bar_emits_six_kpi_chips() {
    let source = format!(
        "{}{}",
        MESH_KPI_SRC,
        r#"
component MeshSummaryBar(nodes: str, active: str, blocked: str, errors: str, tok_s: str, cost_h: str) {
    view: row(pad_y=1, border_b=true, border_color="zinc.800", shrink=0, bg="zinc.950") {
        MeshKpi(key="NODES",   value=nodes,   sub="total")
        panel(w=1, bg="zinc.800", raw_class="self-stretch my-2")
        MeshKpi(key="ACTIVE",  value=active,  sub="running")
        panel(w=1, bg="zinc.800", raw_class="self-stretch my-2")
        MeshKpi(key="BLOCKED", value=blocked, sub="waiting")
        panel(w=1, bg="zinc.800", raw_class="self-stretch my-2")
        MeshKpi(key="ERRORS",  value=errors)
        panel(w=1, bg="zinc.800", raw_class="self-stretch my-2")
        MeshKpi(key="TOK/S",   value=tok_s,   sub="avg")
        panel(w=1, bg="zinc.800", raw_class="self-stretch my-2")
        MeshKpi(key="$/HR",    value=cost_h,  sub="rate")
    }
}
"#
    );

    let ts = compile_component(&source, "MeshSummaryBar");
    println!("MeshSummaryBar.tsx:\n{ts}");

    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("column("), "column must not emit as JS call");

    // All 6 KPI keys must appear.
    assert!(ts.contains("NODES"), "must have NODES chip");
    assert!(ts.contains("ACTIVE"), "must have ACTIVE chip");
    assert!(ts.contains("BLOCKED"), "must have BLOCKED chip");
    assert!(ts.contains("ERRORS"), "must have ERRORS chip");
    // TOK/S and $/HR contain special chars — count JSX uses (exclude import line).
    assert_eq!(
        ts.matches("<MeshKpi").count(),
        6,
        "MeshSummaryBar must include exactly 6 MeshKpi calls"
    );

    insta::assert_snapshot!("mesh_summary_bar_tsx_mesh_surface", ts);
}

// ── AgentNode ─────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn agent_node_emits_circle_with_conditional_stroke() {
    let ts = compile_component(AGENT_NODE_SRC, "AgentNode");
    println!("AgentNode.tsx:\n{ts}");

    // Must emit a <circle> SVG element (passthrough).
    assert!(
        ts.contains("<circle"),
        "AgentNode must emit <circle> SVG element"
    );
    assert!(!ts.contains("circle("), "circle must not emit as JS call");

    // Conditional stroke colors must appear.
    assert!(ts.contains("#3b82f6"), "selected stroke blue must appear");
    assert!(ts.contains("#52525b"), "idle stroke zinc must appear");

    // onClick handler must be present.
    assert!(ts.contains("onClick"), "AgentNode must emit onClick");

    insta::assert_snapshot!("agent_node_tsx_mesh_surface", ts);
}

// ── OrchNode ──────────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn orch_node_emits_polygon_with_conditional_stroke() {
    let ts = compile_component(ORCH_NODE_SRC, "OrchNode");
    println!("OrchNode.tsx:\n{ts}");

    assert!(
        ts.contains("<polygon"),
        "OrchNode must emit <polygon> SVG element"
    );
    assert!(!ts.contains("polygon("), "polygon must not emit as JS call");

    // Conditional stroke: selected (blue) vs default (caller-supplied color).
    assert!(
        ts.contains("#60a5fa"),
        "selected stroke light-blue must appear"
    );

    // points prop must be forwarded.
    assert!(ts.contains("points"), "OrchNode must forward points prop");

    insta::assert_snapshot!("orch_node_tsx_mesh_surface", ts);
}

// ── MeshTopology ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn mesh_topology_emits_svg_with_all_nodes_and_edges() {
    let source = format!(
        "{}{}{}{}",
        STATE_CHIP_SRC,
        AGENT_NODE_SRC,
        ORCH_NODE_SRC,
        r##"
component MeshTopology() {
    state selected: str = ""

    view: row(flex=1, min_h=0, overflow="hidden") {
        panel(flex=1, overflow="hidden", bg="zinc.950") {
            svg(raw_class="w-full h-full", view_box="0 0 800 480") {
                line(x1=230, y1=260, x2=570, y2=260, stroke="#1d4ed8", stroke_width=1, stroke_dasharray="6 3")
                line(x1=230, y1=260, x2=130, y2=120, stroke="#3f3f46", stroke_width=1)
                line(x1=230, y1=260, x2=230, y2=80,  stroke="#3f3f46", stroke_width=1)
                line(x1=230, y1=260, x2=330, y2=120, stroke="#3f3f46", stroke_width=1)
                line(x1=570, y1=260, x2=470, y2=120, stroke="#3f3f46", stroke_width=1)
                line(x1=570, y1=260, x2=670, y2=120, stroke="#3f3f46", stroke_width=1)
                OrchNode(points="262,260 246,288 214,288 198,260 214,232 246,232", id="orchestrator-7c2a", selected=selected, orch_stroke="#2563eb", on_click=fn() { selected = "orchestrator-7c2a" })
                OrchNode(points="602,260 586,288 554,288 538,260 554,232 586,232", id="orchestrator-3f1b", selected=selected, orch_stroke="#7c3aed", on_click=fn() { selected = "orchestrator-3f1b" })
                AgentNode(cx=130, cy=120, id="lex-2",       selected=selected, on_click=fn() { selected = "lex-2" })
                AgentNode(cx=230, cy=80,  id="parse-1",     selected=selected, on_click=fn() { selected = "parse-1" })
                AgentNode(cx=330, cy=120, id="hir-3",       selected=selected, on_click=fn() { selected = "hir-3" })
                AgentNode(cx=470, cy=120, id="typecheck-1", selected=selected, on_click=fn() { selected = "typecheck-1" })
                AgentNode(cx=670, cy=120, id="codegen-2",   selected=selected, on_click=fn() { selected = "codegen-2" })
            }
        }
        if selected is "" {
            panel()
        } else {
            column(w=72, border_l=true, border_color="zinc.800", bg="zinc.950", shrink=0) {
                row(h=10, pad_x=4, items="center", justify="between", border_b=true, border_color="zinc.800") {
                    text(size="xs", weight="bold", color="white/80", font_family="mono") { selected }
                    button(raw_class="p-1 rounded text-zinc-600 hover:text-zinc-300 hover:bg-zinc-800", on_click=fn() { selected = "" }) { "x" }
                }
                column(pad=4, gap=3) {
                    StateChip(status="idle", label="idle")
                }
            }
        }
    }
}
"##
    );

    let ts = compile_component(&source, "MeshTopology");
    println!("MeshTopology.tsx:\n{ts}");

    assert!(!ts.contains("row("), "row must not emit as JS call");

    // SVG canvas must be present.
    assert!(ts.contains("<svg"), "MeshTopology must include SVG canvas");
    assert!(ts.contains("viewBox"), "SVG must have viewBox attribute");

    // Delegation edge (dashed blue).
    assert!(
        ts.contains("#1d4ed8"),
        "delegation edge must use blue color"
    );
    assert!(
        ts.contains("strokeDasharray"),
        "delegation edge must be dashed"
    );

    // All 5 AgentNode calls must appear (count JSX uses, exclude import line).
    assert_eq!(
        ts.matches("<AgentNode").count(),
        5,
        "MeshTopology must include 5 AgentNode calls"
    );

    // Both OrchNode calls must appear (count JSX uses, exclude import line).
    assert_eq!(
        ts.matches("<OrchNode").count(),
        2,
        "MeshTopology must include 2 OrchNode calls"
    );

    // State for selection.
    assert!(
        ts.contains("selected"),
        "MeshTopology must manage selected state"
    );
    assert!(
        ts.contains("useState"),
        "MeshTopology must use useState for selection"
    );

    // Inspector rail (conditional).
    assert!(
        ts.contains("StateChip"),
        "MeshTopology inspector must include StateChip"
    );

    insta::assert_snapshot!("mesh_topology_tsx_mesh_surface", ts);
}

// ── ActivityStrip ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn activity_strip_emits_svg_rect_bars() {
    let source = r##"
component ActivityStrip() {
    view: row(h=20, border_t=true, border_color="zinc.800", pad_x=4, items="center", gap=3, shrink=0, bg="zinc.950") {
        text(size="xs", font_family="mono", color="zinc.600", raw_class="w-10 shrink-0 text-right") { "tok/s" }
        panel(flex=1, raw_class="h-10 relative overflow-hidden") {
            svg(raw_class="w-full h-full", view_box="0 0 600 40", preserve_aspect_ratio="none") {
                rect(x=0,  y=36, width=8, height=4,  fill="#22c55e28")
                rect(x=10, y=33, width=8, height=7,  fill="#22c55e38")
                rect(x=20, y=29, width=8, height=11, fill="#22c55e48")
                rect(x=30, y=26, width=8, height=14, fill="#22c55e58")
                rect(x=40, y=22, width=8, height=18, fill="#22c55e68")
            }
        }
        text(size="xs", font_family="mono", color="zinc.700", raw_class="w-10 shrink-0") { "-/s" }
    }
}
"##;

    let ts = compile_component(source, "ActivityStrip");
    println!("ActivityStrip.tsx:\n{ts}");

    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(ts.contains("<svg"), "ActivityStrip must include SVG canvas");
    assert!(
        ts.contains("<rect"),
        "ActivityStrip must include rect bar elements"
    );

    // preserveAspectRatio must be camelCased correctly.
    assert!(
        ts.contains("preserveAspectRatio"),
        "preserve_aspect_ratio must camelCase to preserveAspectRatio"
    );

    insta::assert_snapshot!("activity_strip_tsx_mesh_surface", ts);
}
