//! Golden tests for Phase 1 dashboard surface stubs.
//!
//! Each surface file (`surfaces/*.vox`) is a self-contained stub that renders
//! a header row + `SurfaceStub` empty state.  Tests here verify:
//!   1. The component compiles to TSX without errors.
//!   2. Key structural assertions: no JS function-call emission, correct title
//!      string present, no Latin labels.
//!   3. A snapshot captures the full emitted TSX for regression detection as
//!      surfaces gain real implementations in Phases 2–8.
//!
//! Sources are inlined (no cross-file import resolution in the test harness).
//! Deps (`StateChip`, `SurfaceStub`) are prepended as shared constants.

// ── Compile helper ────────────────────────────────────────────────────────────

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

// ── Shared dependency sources ─────────────────────────────────────────────────
// Inlined so each test is self-contained. Uses `else if` chains throughout
// (parser fix landed in Phase 1 Batch 3).

const STATE_CHIP_SRC: &str = r#"
component StateChip(status: str, label: str = "", dim: bool = false) {
    view: row(items="center", gap=1, pad_x=2, pad_y=0, radius="full", bg=if dim { "white/5" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400/15" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400/15" } else if (status is "error" or status is "failed" or status is "errored") { "rose.500/15" } else { "white/5" }, raw_class="h-5 inline-flex") {
        panel(w=1, h=1, radius="full", bg=if dim { "zinc.600" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" } else if (status is "error" or status is "failed" or status is "errored") { "rose.500" } else { "zinc.600" })
        text(size="xs", font_family="mono", tracking="widest", case="upper", color=if dim { "zinc.500" } else if (status is "running" or status is "ok" or status is "ready") { "emerald.400" } else if (status is "warn" or status is "blocked" or status is "pending") { "amber.400" } else if (status is "error" or status is "failed" or status is "errored") { "rose.400" } else { "zinc.500" }) { if label is "" { status } else { label } }
    }
}
"#;

const SURFACE_STUB_SRC: &str = r#"
component SurfaceStub(title: str, desc: str, action: str) {
    view: column(flex=1, items="center", justify="center", gap=4, color="zinc.500", pad=8) {
        column(w="full", gap=2, raw_class="max-w-md opacity-20 pointer-events-none mb-4") {
            row(gap=2) {
                panel(flex=1, h=8, bg="zinc.800", radius="lg")
                panel(w=20, h=8, bg="zinc.800", radius="lg")
            }
            panel(w="full", h=24, bg="zinc.800", radius="lg")
            row(gap=2) {
                panel(flex=1, h=6, bg="zinc.800", radius="lg")
                panel(flex=1, h=6, bg="zinc.800", radius="lg")
                panel(flex=1, h=6, bg="zinc.800", radius="lg")
            }
        }
        text(size="sm", weight="bold", case="upper", tracking="widest", color="zinc.400") { title }
        text(size="xs", color="zinc.600", raw_class="text-center max-w-xs") { desc }
        row(gap=3, raw_class="mt-2") {
            button(raw_class="px-4 py-2 rounded-lg bg-blue-600 text-white text-xs font-bold hover:bg-blue-500 transition-colors") { action }
            button(raw_class="px-4 py-2 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-400 hover:bg-zinc-800 transition-colors") { "Load example" }
        }
    }
}
"#;

// ── SurfaceStub standalone ────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn surface_stub_emits_silhouette_and_cta() {
    let ts = compile_component(SURFACE_STUB_SRC, "SurfaceStub");
    println!("SurfaceStub.tsx:\n{ts}");

    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("panel("), "panel must not emit as JS call");
    assert!(!ts.contains("button("), "button must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");

    // Both CTA buttons must appear (primary action + "Load example").
    assert!(
        ts.contains("Load example"),
        "SurfaceStub must include 'Load example' button"
    );

    insta::assert_snapshot!("surface_stub_tsx_dashboard_surface", ts);
}

// ── MeshSurface ───────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn mesh_surface_stub_emits_header_and_stub() {
    let source = format!(
        "{}{}{}",
        STATE_CHIP_SRC,
        SURFACE_STUB_SRC,
        r#"
component MeshSurface() {
    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Mesh" }
            row(gap=2, items="center") {
                StateChip(status="idle", label="0 nodes")
                button(raw_class="px-3 py-1 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-400 hover:bg-zinc-800") { "Refresh" }
            }
        }
        SurfaceStub(title="Mesh", desc="Force-directed agent topology. Nodes show orchestrators; edges show active channels.", action="Start an orchestrator")
    }
}
"#
    );

    let ts = compile_component(&source, "MeshSurface");
    println!("MeshSurface.tsx:\n{ts}");

    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("row("), "row must not emit as JS call");
    assert!(!ts.contains("text("), "text must not emit as JS call");

    assert!(
        ts.contains("Mesh"),
        "MeshSurface header must include 'Mesh' title"
    );
    assert!(
        ts.contains("StateChip"),
        "MeshSurface must include StateChip"
    );
    assert!(
        ts.contains("SurfaceStub"),
        "MeshSurface must include SurfaceStub"
    );
    assert!(
        ts.contains("Refresh"),
        "MeshSurface header must include Refresh button"
    );

    insta::assert_snapshot!("mesh_surface_stub_tsx_dashboard_surface", ts);
}

// ── SpeakSurface ──────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn speak_surface_stub_emits_header_and_stub() {
    let source = format!(
        "{}{}{}",
        STATE_CHIP_SRC,
        SURFACE_STUB_SRC,
        r#"
component SpeakSurface() {
    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Speak" }
            StateChip(status="idle", label="no session")
        }
        SurfaceStub(title="Speak", desc="Chat with the mesh. Tool calls render as collapsible cards.", action="Start a conversation")
    }
}
"#
    );

    let ts = compile_component(&source, "SpeakSurface");
    println!("SpeakSurface.tsx:\n{ts}");

    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(
        ts.contains("Speak"),
        "SpeakSurface must include 'Speak' title"
    );
    assert!(
        ts.contains("StateChip"),
        "SpeakSurface must include StateChip"
    );

    insta::assert_snapshot!("speak_surface_stub_tsx_dashboard_surface", ts);
}

// ── ForgeSurface ──────────────────────────────────────────────────────────────
// Forge has internal state + segmented toggle — tests that state + else if
// in button raw_class compiles correctly.

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn forge_surface_stub_emits_panel_toggle() {
    let source = format!(
        "{}{}",
        SURFACE_STUB_SRC,
        r#"
component ForgeSurface() {
    state panel: str = "pipeline"

    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", gap=4, shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Forge" }
            row(gap=1) {
                button(raw_class=if panel is "pipeline" { "px-3 py-1 rounded-lg bg-white/8 text-xs text-white font-medium" } else { "px-3 py-1 rounded-lg text-xs text-zinc-500 hover:text-zinc-300" }, on_click=fn() { panel = "pipeline" }) { "Pipeline" }
                button(raw_class=if panel is "timeline" { "px-3 py-1 rounded-lg bg-white/8 text-xs text-white font-medium" } else { "px-3 py-1 rounded-lg text-xs text-zinc-500 hover:text-zinc-300" }, on_click=fn() { panel = "timeline" }) { "Time Travel" }
            }
        }
        if panel is "pipeline" {
            SurfaceStub(title="Pipeline", desc="Lex to Codegen. Each stage shows duration, status, and diagnostics.", action="Run build")
        } else {
            SurfaceStub(title="Time Travel", desc="Scrub the workflow event timeline to inspect durable state at any past instant.", action="Load a workflow")
        }
    }
}
"#
    );

    let ts = compile_component(&source, "ForgeSurface");
    println!("ForgeSurface.tsx:\n{ts}");

    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(!ts.contains("button("), "button must not emit as JS call");

    assert!(
        ts.contains("Pipeline"),
        "ForgeSurface must include Pipeline tab label"
    );
    assert!(
        ts.contains("Time Travel"),
        "ForgeSurface must include Time Travel tab label"
    );
    // Both SurfaceStub calls must appear (one per branch).
    assert!(
        ts.contains("SurfaceStub"),
        "ForgeSurface must include SurfaceStub"
    );
    // onClick wiring from on_click lambdas.
    assert!(
        ts.contains("onClick"),
        "ForgeSurface buttons must emit onClick"
    );

    insta::assert_snapshot!("forge_surface_stub_tsx_dashboard_surface", ts);
}

// ── RunsSurface ───────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn runs_surface_stub_compiles() {
    let source = format!(
        "{}{}",
        SURFACE_STUB_SRC,
        r#"
component RunsSurface() {
    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Runs" }
            button(raw_class="px-3 py-1 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-400 hover:bg-zinc-800") { "Live tail: off" }
        }
        SurfaceStub(title="Runs", desc="Persistent log of every orchestrator run. Click a row to see the full event tree.", action="Start a run")
    }
}
"#
    );

    let ts = compile_component(&source, "RunsSurface");
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(ts.contains("Runs"), "RunsSurface must include 'Runs' title");
    assert!(
        ts.contains("Live tail"),
        "RunsSurface must include live tail button"
    );

    insta::assert_snapshot!("runs_surface_stub_tsx_dashboard_surface", ts);
}

// ── ModelsSurface ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn models_surface_stub_compiles() {
    let source = format!(
        "{}{}",
        SURFACE_STUB_SRC,
        r#"
component ModelsSurface() {
    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Models" }
            button(raw_class="px-3 py-1 rounded-lg bg-zinc-900 border border-white/10 text-xs text-zinc-400 hover:bg-zinc-800") { "Auto-route: on" }
        }
        SurfaceStub(title="Models", desc="Hosted and local model registry. Cards show provider, context window, cost, and latency.", action="Add a model")
    }
}
"#
    );

    let ts = compile_component(&source, "ModelsSurface");
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(
        ts.contains("Models"),
        "ModelsSurface must include 'Models' title"
    );
    assert!(
        ts.contains("Auto-route"),
        "ModelsSurface must include auto-route button"
    );

    insta::assert_snapshot!("models_surface_stub_tsx_dashboard_surface", ts);
}

// ── CodeSurface ───────────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn code_surface_stub_compiles() {
    let source = format!(
        "{}{}{}",
        STATE_CHIP_SRC,
        SURFACE_STUB_SRC,
        r#"
component CodeSurface() {
    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", justify="between", shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Code" }
            StateChip(status="idle", label="no file")
        }
        SurfaceStub(title="Code", desc="Scoped file editor for .vox, Rust, and TypeScript. File tree on the left; diagnostics and agent activity on the right.", action="Open a file")
    }
}
"#
    );

    let ts = compile_component(&source, "CodeSurface");
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(ts.contains("Code"), "CodeSurface must include 'Code' title");
    assert!(
        ts.contains("StateChip"),
        "CodeSurface must include StateChip"
    );

    insta::assert_snapshot!("code_surface_stub_tsx_dashboard_surface", ts);
}

// ── SettingsSurface ───────────────────────────────────────────────────────────

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn settings_surface_stub_compiles() {
    let source = format!(
        "{}{}",
        SURFACE_STUB_SRC,
        r#"
component SettingsSurface() {
    view: column(flex=1, overflow="hidden", bg="zinc.950") {
        row(h=10, border_b=true, border_color="zinc.800", pad_x=4, items="center", shrink=0) {
            text(size="sm", weight="black", tracking="tight", color="white") { "Settings" }
        }
        SurfaceStub(title="Settings", desc="Identity and API tokens, workspace paths, budget caps, telemetry, and appearance.", action="Configure identity")
    }
}
"#
    );

    let ts = compile_component(&source, "SettingsSurface");
    assert!(!ts.contains("column("), "column must not emit as JS call");
    assert!(
        ts.contains("Settings"),
        "SettingsSurface must include 'Settings' title"
    );

    insta::assert_snapshot!("settings_surface_stub_tsx_dashboard_surface", ts);
}

// ── No-Latin-labels cross-surface assertion ───────────────────────────────────
// Compiles each surface and asserts no Latin nav label appears in any of them.

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn no_latin_labels_in_any_surface_stub() {
    let latin_labels = ["LOQUELA", "RETE", "FABRICA", "IMPERIUM"];

    let surfaces: &[(&str, &str)] = &[
        (
            &format!(
                "{}{}{}",
                STATE_CHIP_SRC,
                SURFACE_STUB_SRC,
                r#"component MeshSurface() { view: column(flex=1, bg="zinc.950") { text(size="sm", color="white") { "Mesh" } SurfaceStub(title="Mesh", desc="topology", action="Start") } }"#
            ),
            "MeshSurface",
        ),
        (
            &format!(
                "{}{}{}",
                STATE_CHIP_SRC,
                SURFACE_STUB_SRC,
                r#"component SpeakSurface() { view: column(flex=1, bg="zinc.950") { text(size="sm", color="white") { "Speak" } SurfaceStub(title="Speak", desc="chat", action="Start") } }"#
            ),
            "SpeakSurface",
        ),
        (
            &format!(
                "{}{}",
                SURFACE_STUB_SRC,
                r#"component RunsSurface() { view: column(flex=1, bg="zinc.950") { text(size="sm", color="white") { "Runs" } SurfaceStub(title="Runs", desc="logs", action="Start") } }"#
            ),
            "RunsSurface",
        ),
        (
            &format!(
                "{}{}",
                SURFACE_STUB_SRC,
                r#"component SettingsSurface() { view: column(flex=1, bg="zinc.950") { text(size="sm", color="white") { "Settings" } SurfaceStub(title="Settings", desc="config", action="Configure") } }"#
            ),
            "SettingsSurface",
        ),
    ];

    for (source, name) in surfaces {
        let ts = compile_component(source, name);
        for label in latin_labels {
            assert!(
                !ts.contains(label),
                "{name} must not contain Latin label {label}"
            );
        }
    }
}
