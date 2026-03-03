/// Codegen tests A-105 and A-106.
///
/// A-105: emit_vector_index_ddl generates correct libsql_vector_idx DDL
/// A-106: generated Rust code for ADT produces valid enum with struct variants
use vox_ast::span::Span;
use vox_hir::*;

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

fn minimal_module() -> HirModule {
    HirModule {
        consts: vec![],
        imports: vec![],
        functions: vec![],
        types: vec![],
        routes: vec![],
        actors: vec![],
        workflows: vec![],
        activities: vec![],
        tests: vec![],
        server_fns: vec![],
        tables: vec![],
        indexes: vec![],
        vector_indexes: vec![],
        search_indexes: vec![],
        mcp_tools: vec![],
        traits: vec![],
        impls: vec![],
        queries: vec![],
        mutations: vec![],
        actions: vec![],
        skills: vec![],
        agents: vec![],
        native_agents: vec![],
        scheduled: vec![],
        messages: vec![],
        config_blocks: vec![],
        collections: vec![],
        contexts: vec![],
        hooks: vec![],
        providers: vec![],
        ..Default::default()
    }
}

#[test]
fn a105_vector_index_ddl_generates_float32_column() {
    let mut module = minimal_module();

    // Add a table with a vector column
    module.tables.push(HirTable {
        id: DefId(1),
        name: "Document".to_string(),
        fields: vec![
            HirTableField {
                name: "text".to_string(),
                type_ann: HirType::Named("str".to_string()),
                description: None,
                span: dummy_span(),
            },
            HirTableField {
                name: "embedding".to_string(),
                type_ann: HirType::Named("vec384".to_string()),
                description: None,
                span: dummy_span(),
            },
        ],
        description: None,
        is_pub: false,
        is_deprecated: false,
        span: dummy_span(),
    });

    // Add a vector index on the embedding column
    module.vector_indexes.push(HirVectorIndex {
        table_name: "Document".to_string(),
        index_name: "idx_document_embedding".to_string(),
        column: "embedding".to_string(),
        dimensions: 384,
        filter_fields: vec![],
        span: dummy_span(),
    });

    // Need a route to trigger DB setup
    module.routes.push(HirRoute {
        method: vox_hir::HirHttpMethod::Post,
        path: "/upload".to_string(),
        params: vec![],
        return_type: None,
        body: vec![],
        is_deprecated: false,
        is_traced: false,
        span: dummy_span(),
    });

    let main_rs = vox_codegen_rust::emit::emit_main(&module, "test_app");

    assert!(
        main_rs.contains("\"kind\":\"Vector\""),
        "Should contain Vector index kind in digest, got:\n{}",
        &main_rs[..main_rs.len().min(3000)]
    );
    assert!(
        main_rs.contains("\"dimensions\":384"),
        "Should contain 384 dimensions in digest, got:\n{}",
        &main_rs[..main_rs.len().min(3000)]
    );
    assert!(
        main_rs.contains("embedding"),
        "Should reference the embedding column"
    );
}

#[test]
fn a106_adt_generates_valid_rust_enum_with_struct_variants() {
    let mut module = minimal_module();

    module.types.push(HirTypeDef {
        id: DefId(1),
        name: "Shape".to_string(),
        generics: vec![],
        variants: vec![
            HirVariant {
                name: "Circle".to_string(),
                fields: vec![("radius".to_string(), HirType::Named("f64".to_string()))],
                span: dummy_span(),
            },
            HirVariant {
                name: "Rect".to_string(),
                fields: vec![
                    ("width".to_string(), HirType::Named("f64".to_string())),
                    ("height".to_string(), HirType::Named("f64".to_string())),
                ],
                span: dummy_span(),
            },
            HirVariant {
                name: "Point".to_string(),
                fields: vec![],
                span: dummy_span(),
            },
        ],
        fields: vec![],
        type_alias: None,
        is_pub: true,
        is_deprecated: false,

        span: dummy_span(),
    });

    let lib_rs = vox_codegen_rust::emit::emit_lib(&module);

    assert!(
        lib_rs.contains("pub enum Shape {"),
        "Should generate pub enum Shape, got:\n{}",
        &lib_rs[..lib_rs.len().min(2000)]
    );
    assert!(
        lib_rs.contains("Circle {"),
        "Should have Circle as struct variant, got:\n{}",
        &lib_rs[..lib_rs.len().min(2000)]
    );
    assert!(
        lib_rs.contains("Rect {"),
        "Should have Rect as struct variant, got:\n{}",
        &lib_rs[..lib_rs.len().min(2000)]
    );
    assert!(
        lib_rs.contains("Point,"),
        "Should have Point as unit variant"
    );
    assert!(
        lib_rs.contains("#[serde(tag = \"_tag\")]"),
        "Should have serde internally-tagged annotation"
    );
    assert!(
        lib_rs.contains("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]"),
        "Should have derive macros"
    );
}
