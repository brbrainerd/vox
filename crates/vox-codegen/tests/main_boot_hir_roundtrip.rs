//! ADR-041 §6(b) / P8.b: verify the `emit_main_boot` output for HIR embedding
//! produces JSON that deserializes back to an equivalent `HirModule`.
//!
//! This guards the codegen-to-compiled-binary path: at codegen time we
//! serialize the HirModule into a raw-string `const &str`, and the generated
//! `main()` decodes it via `::serde_json::from_str(...)` before calling
//! `set_current_hir_module()`. If the round-trip fails the generated binary
//! would panic at startup, so we lock the invariant here.

use vox_codegen::codegen_rust::emit::emit_main_boot;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn embedded_hir_roundtrips_through_json() {
    let src = r#"
        @scheduled("1m")
        fn tick() to int { return 1 }

        @server
        fn hello() to str { return "hi" }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);

    let main_rs = emit_main_boot(&hir);

    // Extract the embedded JSON between EMBEDDED_HIR's raw-string delimiters.
    // The shape we emit is:
    //   const EMBEDDED_HIR: &str = r#"...JSON..."#;
    // (with a dynamic `#` count for raw-string delimiter escalation if the
    // JSON contains `"#`; for this fixture a single `#` always suffices).
    let start_marker = "const EMBEDDED_HIR: &str = r#\"";
    let end_marker = "\"#;\n";
    let start = main_rs
        .find(start_marker)
        .expect("emit contains the EMBEDDED_HIR raw-string opener");
    let json_start = start + start_marker.len();
    let json_end_rel = main_rs[json_start..]
        .find(end_marker)
        .expect("emit contains the EMBEDDED_HIR raw-string closer");
    let json = &main_rs[json_start..json_start + json_end_rel];

    // Round-trip: JSON → HirModule. Must succeed.
    let roundtripped: vox_compiler::hir::HirModule =
        serde_json::from_str(json).expect("embedded JSON deserializes back to HirModule");

    // Verify key invariants survive the round-trip.
    assert_eq!(
        roundtripped.functions.len(),
        hir.functions.len(),
        "function count preserved"
    );
    assert_eq!(
        roundtripped.endpoint_fns.len(),
        hir.endpoint_fns.len(),
        "endpoint count preserved"
    );

    // Spot-check that the @scheduled interval survives — this is what the
    // scheduler runner reads at boot.
    let scheduled_intervals: Vec<&str> = roundtripped
        .functions
        .iter()
        .filter_map(|f| f.schedule_interval.as_deref())
        .collect();
    let original_intervals: Vec<&str> = hir
        .functions
        .iter()
        .filter_map(|f| f.schedule_interval.as_deref())
        .collect();
    assert_eq!(
        scheduled_intervals, original_intervals,
        "@scheduled intervals preserved through round-trip"
    );
}

#[test]
fn embedded_hir_for_empty_module_roundtrips() {
    // The pathological-empty case: just a trivial fn, no @scheduled, no
    // endpoints, no actors. Still must serialize and deserialize cleanly.
    let src = r#"
        fn plain() to int { return 1 }
    "#;
    let module = parse(lex(src)).expect("parse");
    let hir = lower_module(&module);

    let main_rs = emit_main_boot(&hir);

    let start_marker = "const EMBEDDED_HIR: &str = r#\"";
    let end_marker = "\"#;\n";
    let start = main_rs.find(start_marker).expect("EMBEDDED_HIR opener");
    let json_start = start + start_marker.len();
    let json_end_rel = main_rs[json_start..]
        .find(end_marker)
        .expect("EMBEDDED_HIR closer");
    let json = &main_rs[json_start..json_start + json_end_rel];

    let roundtripped: vox_compiler::hir::HirModule =
        serde_json::from_str(json).expect("empty-module JSON deserializes");
    assert_eq!(
        roundtripped.functions.len(),
        1,
        "single trivial fn survives"
    );
    assert!(
        roundtripped.endpoint_fns.is_empty(),
        "no endpoints in trivial module"
    );
}
