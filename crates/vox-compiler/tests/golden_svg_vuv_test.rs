//! Smoke test: SVG via VUV view-call passthrough (dashboard-vuv-port salvage).
//!
//! Tests whether raw SVG elements (`svg`, `polygon`, `defs`, `stop`, `rect`) survive the
//! full compile pipeline and whether:
//!   - snake_case attributes (`view_box`, `stroke_width`, `preserve_aspect_ratio`,
//!     `stop_color`, `stop_opacity`) lower to camelCase via `map_jsx_attr_name`
//!   - `radial_gradient` lowers to `<radialGradient>` via `map_jsx_tag`
//!   - Nested view-calls inside an unknown-tag (passthrough) parent emit as JSX children,
//!     NOT as JS function calls (fix for dashboard-vuv-port bug)

/// Step 1 control: verify that nested **known** primitives (column/row/text) compose
/// correctly before checking the SVG passthrough path.  If this fails, the bug is bigger
/// than the SVG passthrough — escalate before proceeding.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn nested_known_primitives_compose() {
    let source = r#"
component Layout() {
    view: column(pad=4) {
        row(gap=2) {
            text() { "left" }
            text() { "right" }
        }
    }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse Layout");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen Layout");
    let ts = out
        .files
        .iter()
        .find(|(n, _)| n == "Layout.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Layout.tsx not in output");

    println!("Layout.tsx emit:\n{ts}");

    // column and row must be proper JSX elements with children, not JS calls.
    assert!(
        !ts.contains("column("),
        "column must NOT emit as a JS function call; got:\n{ts}"
    );
    assert!(
        !ts.contains("row("),
        "row must NOT emit as a JS function call; got:\n{ts}"
    );
    // text children must appear inside the row, not as bare statements.
    assert!(
        ts.contains("left"),
        "text child 'left' should appear in output; got:\n{ts}"
    );
    assert!(
        ts.contains("right"),
        "text child 'right' should appear in output; got:\n{ts}"
    );
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn play_icon_svg_passthrough() {
    let source = r#"
component PlayIcon() {
    view: svg(view_box="0 0 24 24", fill="none", stroke="currentColor", stroke_width=1.5) {
        polygon(points="5 3 19 12 5 21 5 3")
    }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse PlayIcon");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen PlayIcon");
    let ts = out
        .files
        .iter()
        .find(|(n, _)| n == "PlayIcon.tsx")
        .map(|(_, c)| c.as_str())
        .expect("PlayIcon.tsx not in output");

    println!("PlayIcon.tsx emit:\n{ts}");

    // The outer svg tag emits with correct camelCase attribute mapping:
    assert!(
        ts.contains("viewBox"),
        "view_box should lower to viewBox; got:\n{ts}"
    );
    assert!(
        ts.contains("strokeWidth"),
        "stroke_width should lower to strokeWidth; got:\n{ts}"
    );

    // polygon must emit as a JSX child element, not a JS function call.
    assert!(
        ts.contains("<polygon"),
        "polygon should emit as JSX child <polygon .../>; got:\n{ts}"
    );
    assert!(
        !ts.contains("polygon("),
        "polygon must NOT emit as a JS function call; got:\n{ts}"
    );

    insta::assert_snapshot!("play_icon_tsx_svg_passthrough", ts);
}

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn halo_rect_svg_with_radial_gradient() {
    let source = r##"
component HaloRect() {
    view: svg(view_box="0 0 100 60", preserve_aspect_ratio="xMidYMid meet") {
        defs() {
            radial_gradient(id="halo", cx=0.5, cy=0.5, r=0.5) {
                stop(offset="0%", stop_color="#34d399", stop_opacity=0.4)
                stop(offset="100%", stop_color="#34d399", stop_opacity=0)
            }
        }
        rect(width=100, height=60, fill="url(#halo)")
    }
}
"##;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse HaloRect");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen HaloRect");
    let ts = out
        .files
        .iter()
        .find(|(n, _)| n == "HaloRect.tsx")
        .map(|(_, c)| c.as_str())
        .expect("HaloRect.tsx not in output");

    println!("HaloRect.tsx emit:\n{ts}");

    // The outer svg tag emits with correct camelCase attribute mapping:
    assert!(
        ts.contains("viewBox"),
        "view_box should lower to viewBox; got:\n{ts}"
    );
    assert!(
        ts.contains("preserveAspectRatio"),
        "preserve_aspect_ratio should lower to preserveAspectRatio; got:\n{ts}"
    );

    // All nested SVG elements must emit as JSX children, not JS function calls.
    assert!(
        ts.contains("<defs"),
        "defs should emit as JSX child; got:\n{ts}"
    );
    assert!(
        ts.contains("<radialGradient"),
        "radial_gradient should lower to <radialGradient>; got:\n{ts}"
    );
    assert!(
        ts.contains("<stop"),
        "stop should emit as JSX child; got:\n{ts}"
    );
    assert!(
        ts.contains("<rect"),
        "rect should emit as JSX child; got:\n{ts}"
    );
    // Named kwargs (stop_color, stop_opacity) must survive as JSX attributes.
    assert!(
        ts.contains("stopColor"),
        "stop_color should lower to stopColor attr; got:\n{ts}"
    );
    assert!(
        ts.contains("stopOpacity"),
        "stop_opacity should lower to stopOpacity attr; got:\n{ts}"
    );

    insta::assert_snapshot!("halo_rect_tsx_svg_passthrough", ts);
}

/// Step 5: deeper SVG smoke test — multi-level nesting, mixed kwargs,
/// self-closing and container children, camelCase alias coverage.
///
/// Exercises:
///   - `<svg><defs><radialGradient><stop/></radialGradient></defs><circle/></svg>`
///   - Mixed positional-value and named kwargs
///   - snake_case → camelCase aliases: `radial_gradient`, `stop_color`, `stop_opacity`
///   - `g(transform=...)` container with multiple circle children
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn mesh_node_topology_svg() {
    let source = r##"
component MeshNode() {
    view: svg(view_box="0 0 100 100") {
        defs() {
            radial_gradient(id="halo", cx=0.5, cy=0.5, r=0.5) {
                stop(offset="0%", stop_color="#34d399", stop_opacity=0.4)
                stop(offset="100%", stop_color="#34d399", stop_opacity=0)
            }
        }
        g(transform="translate(50, 50)") {
            circle(r=8, fill="url(#halo)")
            circle(r=3, fill="#34d399")
        }
    }
}
"##;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse MeshNode");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen MeshNode");
    let ts = out
        .files
        .iter()
        .find(|(n, _)| n == "MeshNode.tsx")
        .map(|(_, c)| c.as_str())
        .expect("MeshNode.tsx not in output");

    println!("MeshNode.tsx emit:\n{ts}");

    // Outer svg with correct viewBox alias.
    assert!(ts.contains("viewBox"), "view_box → viewBox; got:\n{ts}");

    // defs container must be a JSX element.
    assert!(ts.contains("<defs"), "defs as JSX; got:\n{ts}");

    // radial_gradient → <radialGradient> (camelCase alias).
    assert!(
        ts.contains("<radialGradient"),
        "radial_gradient → <radialGradient>; got:\n{ts}"
    );

    // stop children with camelCase attr aliases.
    assert!(ts.contains("<stop"), "stop as JSX child; got:\n{ts}");
    assert!(
        ts.contains("stopColor"),
        "stop_color → stopColor; got:\n{ts}"
    );
    assert!(
        ts.contains("stopOpacity"),
        "stop_opacity → stopOpacity; got:\n{ts}"
    );

    // g container with two circle children.
    assert!(ts.contains("<g"), "<g> group element; got:\n{ts}");
    assert!(ts.contains("<circle"), "<circle> child; got:\n{ts}");

    // Nothing should appear as a plain JS function call.
    for banned in &["defs(", "radial_gradient(", "stop(", "circle(", "g("] {
        assert!(
            !ts.contains(banned),
            "{banned} must not appear as a JS function call; got:\n{ts}"
        );
    }

    insta::assert_snapshot!("mesh_node_tsx_svg_topology", ts);
}
