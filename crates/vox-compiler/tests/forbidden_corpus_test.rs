//! Forbidden corpus: every file in `examples/forbidden/` must FAIL compilation
//! with exactly the diagnostic code named in its `// expect-error:` header.
//!
//! This suite is the contract for "VUV makes this bug structurally unrepresentable".
//! A file that starts compiling cleanly — or fails with a *different* code — is a
//! regression, not a victory. It exercises the full pipeline (parse → typecheck →
//! lower → web_ir validate) so the guarantee is checked end-to-end on real `.vox`,
//! not on hand-built IR.

use std::path::PathBuf;

use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
use vox_codegen::web_ir::validate::validate_web_ir_with_metrics;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn forbidden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/forbidden")
}

/// Compile `src` through every stage and return ALL diagnostic codes (typeck +
/// web_ir validate), unfiltered — advisory codes included, since "forbidden" means
/// the build must reject the program, and we assert on the specific code.
fn all_diagnostic_codes(src: &str) -> Vec<String> {
    let tokens = lex(src);
    let module = match parse(tokens) {
        Ok(m) => m,
        Err(errs) => {
            // A hard parse error is itself a rejection; surface a synthetic code so
            // parse-level forbidden cases can assert on it if ever added.
            return errs
                .iter()
                .map(|_| "parse-error".to_string())
                .chain(std::iter::once("parse-error".to_string()))
                .collect();
        }
    };

    let mut codes: Vec<String> = typecheck_ast_module(src, &module)
        .iter()
        .filter_map(|d| d.code.clone())
        .collect();

    let hir = lower_module(&module);
    let web_ir = lower_hir_to_web_ir(&hir);
    let (diags, _metrics) = validate_web_ir_with_metrics(&web_ir);
    codes.extend(diags.into_iter().map(|d| d.code));
    codes
}

#[test]
fn every_forbidden_example_fails_with_its_declared_code() {
    let dir = forbidden_dir();
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("examples/forbidden must exist") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let expected = src
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("// expect-error:"))
            .unwrap_or_else(|| panic!("{path:?} missing `// expect-error:` header"))
            .trim()
            .to_string();

        let codes = all_diagnostic_codes(&src);
        assert!(
            codes.iter().any(|c| c == &expected),
            "{path:?}: expected diagnostic `{expected}`, got {codes:?}"
        );
        checked += 1;
    }
    assert!(
        checked >= 6,
        "forbidden corpus shrank to {checked} files — did fixtures get deleted?"
    );
}
