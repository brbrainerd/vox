//! `@json_as` codegen must map Vox `Json` to a Rust type that exists in the
//! generated script crate (`serde_json::Value` or a `type Json = …` alias).

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_codegen::codegen_rust::generate_script;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

fn emit_lib_rs(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let runtime = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vox-actor-runtime");
    let out = generate_script(&hir, "vox-script", Some(&runtime)).expect("generate_script");
    out.files.get("src/lib.rs").cloned().expect("lib.rs")
}

fn lib_defines_or_imports_json(lib: &str) -> bool {
    lib.contains("type Json = vox_actor_runtime::builtins::VoxJson")
        && lib.contains("use vox_actor_runtime::builtins::VoxJson")
}

/// Minimal `@json_as` surface: one struct + synthesized from/to helpers.
#[test]
fn json_as_from_json_fn_uses_json_type_alias() {
    let src = r#"
        @json_as(Widget)
        type Widget { name: str }
    "#;
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "Widget_from_json")
        .expect("Widget_from_json");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]);
    assert!(
        emitted.contains("Json") || emitted.contains("VoxJson"),
        "from_json must reference Json/VoxJson in signature; got:\n{emitted}"
    );
    assert!(
        emitted.contains("j: Json"),
        "@json_as from_json param must use Json alias; got:\n{emitted}"
    );
}

#[test]
fn json_as_to_json_fn_wraps_object_literal_as_json() {
    let src = r#"
        @json_as(Widget)
        type Widget { name: str }
    "#;
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "Widget_to_json")
        .expect("Widget_to_json");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]);
    assert!(
        emitted.contains("-> Json"),
        "to_json must return Json; got:\n{emitted}"
    );
    assert!(
        emitted.contains("return VoxJson(serde_json::json!"),
        "to_json must wrap object literal as VoxJson(...); got:\n{emitted}"
    );
}

#[test]
fn json_as_minimal_emits_json_type_or_alias() {
    let src = r#"
        @json_as(Widget)
        type Widget {
            name: str,
        }

        fn main() to str {
            let r = json.parse("{\"name\":\"x\"}")
            match r {
                Error(_) => return "err"
                Ok(j) => {
                    let res = Widget_from_json(j)
                    match res {
                        Error(_) => return "decode_err"
                        Ok(w) => return w.name
                    }
                }
            }
        }
    "#;
    let lib = emit_lib_rs(src);

    assert!(
        lib.contains("Widget_from_json"),
        "expected synthesized from_json helper; sample:\n{}",
        &lib[..lib.len().min(1200)]
    );
    assert!(
        lib_defines_or_imports_json(&lib),
        "generated lib.rs must define Json alias; sample:\n{}",
        &lib[..lib.len().min(1200)]
    );
    assert!(
        lib.contains("Widget_from_json(j: Json"),
        "from_json must take Json param; sample:\n{}",
        &lib[..lib.len().min(2000)]
    );
}

#[test]
fn json_as_typed_golden_defines_json_alias() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/golden/json_as_typed.vox");
    let src = std::fs::read_to_string(&path).expect("read golden");
    let lib = emit_lib_rs(&src);
    assert!(
        lib_defines_or_imports_json(&lib),
        "json_as_typed golden must emit Json type alias; sample:\n{}",
        &lib[..lib.len().min(2500)]
    );
}

#[test]
fn json_as_match_error_pattern_emits_err() {
    let src = r#"
        fn f(r: Result[str]) to str {
            match r {
                Error(_) => return "err"
                Ok(v) => return v
            }
        }
    "#;
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir.functions.first().expect("f");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]);
    assert!(
        emitted.contains("Err(_)"),
        "match arm Error(_) must lower to Err(_); got:\n{emitted}"
    );
}

#[test]
fn json_as_from_json_return_type_is_std_result() {
    let src = r#"
        @json_as(Widget)
        type Widget { name: str }
    "#;
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "Widget_from_json")
        .expect("Widget_from_json");
    let emitted = emit_fn(f, Some(&hir.inferred_types), &[]);
    assert!(
        emitted.contains("-> Result<Widget, String>"),
        "Result[T] must map to Result<T, String>; got:\n{emitted}"
    );
    assert!(
        emitted.contains("Ok(Widget {"),
        "from_json must construct Widget struct literal; got:\n{emitted}"
    );
}

#[test]
fn json_as_lib_may_declare_json_alias_when_needed() {
    let src = r#"
        @json_as(Widget)
        type Widget { name: str }
        fn main() { }
    "#;
    let lib = emit_lib_rs(src);
    // Either fully-qualified serde_json::Value in signatures, or an explicit alias.
    assert!(
        lib.contains("serde_json::Value") || lib_defines_or_imports_json(&lib),
        "generated lib must define or import Json; sample:\n{}",
        &lib[..lib.len().min(1200)]
    );
}
