const VOX_IR_SCHEMA: &str = include_str!("../../../docs/src/reference/vox-ir.schema.json");

#[test]
fn test_ir_emission_with_hashing_and_inference() {
    let source = r#"
        fn add(a: int, b: int) {
            a + b
        }

        @endpoint(kind: server)
        fn greet(name: str) {
            "Hello " + name
        }
    "#;

    // Run frontend
    let res =
        vox_compiler::pipeline::run_frontend_str(source, "test.vox").expect("frontend failed");

    // Inferred return types should match
    let add_fn = res
        .hir
        .functions
        .iter()
        .find(|f| f.name == "add")
        .expect("add not found");
    assert!(add_fn.return_type.is_some());
    // "int" is the expected name for Ty::Int
    if let Some(vox_compiler::hir::HirType::Named(name)) = &add_fn.return_type {
        assert_eq!(name, "int");
    } else {
        panic!("Expected Named(int), got {:?}", add_fn.return_type);
    }

    let greet_fn = res
        .hir
        .endpoint_fns
        .iter()
        .find(|f| f.name == "greet")
        .expect("greet not found");
    assert!(greet_fn.return_type.is_some());

    // Generate IR
    let vox_ir = vox_codegen::vox_ir::lower::lower_hir_to_vox_ir(&res.hir, Some(source));

    // Check hash
    assert!(!vox_ir.metadata.source_hash.is_empty());

    // Check content
    assert!(vox_ir.module.functions.iter().any(|f| f.name == "add"));
    assert!(vox_ir.module.endpoint_fns.iter().any(|f| f.name == "greet"));

    let ir: serde_json::Value = serde_json::to_value(&vox_ir).expect("VoxIrModule as JSON Value");
    let schema_val: serde_json::Value =
        serde_json::from_str(VOX_IR_SCHEMA).expect("parse vox-ir.schema.json");
    let validator = vox_jsonschema_util::compile_validator(&schema_val, "docs vox-ir.schema.json")
        .expect("compile schema");
    vox_jsonschema_util::validate(&ir, &validator, "lower_hir_to_vox_ir").expect("schema validate");
}

/// Per ADR-041 (durable functions completion), `@scheduled` is back in the public grammar
/// and the scheduler runtime ships — the pipeline no longer rejects it at the front-end.
/// However, the `docs vox-ir.schema.json` validator currently treats `routes` as a required
/// top-level property, so a `@scheduled`-only module without an explicit `routes {}` block
/// fails schema validation. Re-enable after the schema is relaxed (or after this test gains
/// a no-op `routes {}` block once `@scheduled` integrates with the routing layer).
#[test]
#[ignore = "ADR-041 lifted @scheduled; vox-ir.schema still requires `routes` — re-enable when the schema accepts @scheduled-only modules"]
fn test_ir_emission_includes_scheduled_jobs_in_web_ir() {
    let source = r#"
@scheduled("1h")
fn tick() to Unit {
    return Unit
}
"#;
    let res =
        vox_compiler::pipeline::run_frontend_str(source, "sched.vox").expect("frontend failed");
    let vox_ir = vox_codegen::vox_ir::lower::lower_hir_to_vox_ir(&res.hir, Some(source));
    let ir: serde_json::Value = serde_json::to_value(&vox_ir).expect("VoxIrModule as JSON Value");
    let jobs = ir
        .pointer("/module/web_ir/scheduled_jobs")
        .and_then(|v| v.as_array())
        .expect("scheduled_jobs array");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["name"], "tick");
    assert_eq!(jobs[0]["interval"], "1h");

    let schema_val: serde_json::Value =
        serde_json::from_str(VOX_IR_SCHEMA).expect("parse vox-ir.schema.json");
    let validator = vox_jsonschema_util::compile_validator(&schema_val, "docs vox-ir.schema.json")
        .expect("compile schema");
    vox_jsonschema_util::validate(&ir, &validator, "scheduled web_ir").expect("schema validate");
}
