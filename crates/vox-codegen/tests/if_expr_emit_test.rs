//! If/else used as expressions must emit Rust tail values (no trailing `;` on arms).

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

#[test]
fn if_else_in_let_binding_emits_expression_arms() {
    let src = r#"
        fn shipping_cost(weight: int, express: bool) to int {
            let base = if weight > 10 { 20 } else { 5 }
            let surcharge = if express { 15 } else { 0 }
            return base + surcharge
        }
    "#;
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "shipping_cost")
        .expect("shipping_cost");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]).replace("\r\n", "\n");
    assert!(
        emitted.contains("if (weight > 10) {\n    20\n    } else {\n    5\n    }"),
        "if/else arms must be tail expressions; got:\n{emitted}"
    );
    assert!(
        !emitted.contains("    20;\n") && !emitted.contains("    5;\n"),
        "expression if arms must not end with semicolon; got:\n{emitted}"
    );
    assert!(
        emitted.contains("if express {\n    15\n    } else {\n    0\n    }"),
        "bool if/else must also emit tail ints; got:\n{emitted}"
    );
}
