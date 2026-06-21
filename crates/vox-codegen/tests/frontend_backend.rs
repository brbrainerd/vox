//! The `frontend_backend` seam must (a) route `Target::TypeScript` to the exact
//! same output as calling `codegen_ts::generate_with_options` directly (no
//! behavior change — this is the Model 3 seam, not a rewrite), and (b) reject
//! non-frontend targets with a typed error rather than a silent fallthrough.

use vox_codegen::codegen_ts::CodegenOptions;
use vox_codegen::frontend_backend::emit_frontend;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::target::Target;

// Minimal component with state + controlled input (avoiding `type` keyword, which
// is reserved in the Vox parser). `placeholder` is a plain identifier attribute.
const SRC: &str = r#"
component Hello() {
    state name: str = ""
    view: input(placeholder="name", bind={name})
}
"#;

fn hir_for(src: &str) -> vox_compiler::hir::HirModule {
    lower_module(&parse(lex(src)).expect("parse"))
}

#[test]
fn typescript_target_matches_direct_emitter_output() {
    let hir = hir_for(SRC);
    let opts = CodegenOptions::default();

    let direct =
        vox_codegen::codegen_ts::generate_with_options(&hir, opts.clone()).expect("direct emit ok");
    let via_seam = emit_frontend(Target::TypeScript, &hir, opts).expect("seam emit ok");

    // Same emitted file set, byte-for-byte — proves zero behavior change.
    assert_eq!(
        via_seam.files, direct.files,
        "seam output must equal direct generate_with_options output"
    );
}

#[test]
fn backend_targets_are_rejected_with_typed_error() {
    let hir = hir_for(SRC);
    for t in [Target::RustAxum, Target::Interpreter, Target::RustTauri] {
        let err = emit_frontend(t, &hir, CodegenOptions::default())
            .expect_err("non-frontend target must error");
        assert!(
            err.contains("not a frontend emission target"),
            "expected typed seam error for {t:?}, got: {err}"
        );
    }
}
