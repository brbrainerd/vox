use std::path::PathBuf;
use vox_codegen::codegen_ts::emitter::generate;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_hir_module_with_path;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

/// Goldens that fail typeck due to known bugs in typeck itself (not the example).
/// Each entry must point to a tracking issue; remove from this list when fixed.
const TYPECK_SKIP: &[&str] = &[
    // Struct-literal expressions in fn bodies aren't resolved
    // ("Undefined variable: <TypeName>"). Surfaced during the
    // mobile-target-evaluation-2026 harness audit on 2026-05-27.
    "wire_format_round_trip",
];

#[test]
fn golden_ts_emit() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden-ts");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("vox"))
        .collect();
    entries.sort();
    let mut typeck_failures: Vec<(String, Vec<String>)> = Vec::new();
    for p in entries {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let m = parse(lex(&src)).expect(&stem);
        let mut hir = lower_module(&m);

        // Typeck gate: golden examples must build through the full CLI pipeline,
        // not just codegen. Catches drift where a lint blocks `vox build` on an
        // example we still claim is "golden". The TYPECK_SKIP list above carves
        // out examples blocked by known typeck bugs (tracked separately).
        if !TYPECK_SKIP.contains(&stem.as_str()) {
            let diags = typecheck_hir_module_with_path(&src, &mut hir, Some(&p));
            let errors: Vec<String> = diags
                .iter()
                .filter(|d| matches!(d.severity, TypeckSeverity::Error))
                .map(|d| d.message.clone())
                .collect();
            if !errors.is_empty() {
                typeck_failures.push((stem.clone(), errors));
            }
        }

        let out = generate(&hir).unwrap();
        let combined = out
            .files
            .iter()
            .map(|(name, content)| format!("=== {} ===\n{}", name, content))
            .collect::<Vec<_>>()
            .join("\n\n");
        insta::with_settings!({ snapshot_suffix => stem.clone() }, {
            insta::assert_snapshot!(combined);
        });
    }
    if !typeck_failures.is_empty() {
        let report = typeck_failures
            .iter()
            .map(|(s, es)| {
                format!(
                    "  {}:\n{}",
                    s,
                    es.iter()
                        .map(|m| format!("    - {}", m))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "{} golden example(s) failed typeck. They would NOT build through `vox build`:\n{}",
            typeck_failures.len(),
            report
        );
    }
}
