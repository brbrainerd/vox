//! Contract-level check: golden `crud_api.vox` produces OpenAPI with `ErrorEnvelope` on operations.

use serde_json::Value;
use vox_codegen::codegen_ts::emitter::{BuildMode, CodegenOptions, generate_with_options};
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

/// True if any `$ref` under `paths` targets `ErrorEnvelope` (response layout may vary).
fn paths_reference_error_envelope(paths_val: &Value) -> bool {
    fn walk(v: &Value) -> bool {
        match v {
            Value::Object(m) => {
                if m.get("$ref").and_then(|x| x.as_str())
                    == Some("#/components/schemas/ErrorEnvelope")
                {
                    return true;
                }
                m.values().any(walk)
            }
            Value::Array(a) => a.iter().any(walk),
            _ => false,
        }
    }
    walk(paths_val)
}

#[test]
fn crud_api_openapi_lists_error_envelope_and_default_response() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir).join("../../examples/golden/crud_api.vox");
    let src =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let module = parse(lex(&src)).expect("parse crud_api.vox");
    let hir = lower_module(&module);
    let opts = CodegenOptions {
        tanstack_start: false,
        target: None,
        mode: BuildMode::Library,
        ..Default::default()
    };
    let out = generate_with_options(&hir, opts).expect("codegen");
    let openapi_str = out
        .files
        .iter()
        .find(|(n, _)| n == "openapi.json")
        .map(|(_, c)| c.as_str())
        .expect("openapi.json in Library output");
    let spec: Value = serde_json::from_str(openapi_str).expect("openapi JSON");

    assert!(
        spec.pointer("/components/schemas/ErrorEnvelope/properties/ok/const")
            == Some(&Value::Bool(false)),
        "ErrorEnvelope.ok const false"
    );

    let paths = &spec["paths"];
    assert!(
        paths.as_object().is_some_and(|p| !p.is_empty()),
        "expected at least one path from crud_api endpoints"
    );
    assert!(
        paths_reference_error_envelope(paths),
        "expected paths to reference #/components/schemas/ErrorEnvelope under responses"
    );

    let pkg = out
        .files
        .iter()
        .find(|(n, _)| n == "package.json")
        .map(|(_, c)| c.as_str())
        .expect("package.json emitted in Library mode");
    let pkg_val: Value = serde_json::from_str(pkg).expect("package.json");
    assert_eq!(pkg_val["name"], "vox-generated-client");
    assert_eq!(pkg_val["private"], true);
    assert!(pkg_val["exports"]["./openapi.json"].is_string());
}
