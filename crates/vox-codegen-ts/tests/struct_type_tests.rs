/// Tests for struct type declarations (product types).
/// Verifies parser, HIR lowering, typeck registration, and TS/Rust codegen.
use vox_ast::span::Span;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;

fn parse_src(src: &str) -> vox_ast::decl::Module {
    let tokens = lex(src);
    parse(tokens).expect(&format!("Parse failed for: {}", src))
}

fn try_parse(src: &str) -> Result<vox_ast::decl::Module, Vec<vox_parser::ParseError>> {
    let tokens = lex(src);
    parse(tokens)
}

#[test]
fn struct_type_parses_colon_syntax() {
    let module = parse_src("type Config:\n    host: str\n    port: int");
    let decl = &module.declarations[0];
    if let vox_ast::decl::Decl::TypeDef(td) = decl {
        assert_eq!(td.name, "Config");
        assert!(td.variants.is_empty(), "struct should have no variants");
        assert_eq!(td.fields.len(), 2, "should have 2 fields");
        assert_eq!(td.fields[0].name, "host");
        assert_eq!(td.fields[1].name, "port");
    } else {
        panic!("expected TypeDef, got {:?}", decl);
    }
}

#[test]
fn adt_type_parses_equals_pipe_syntax() {
    let module = parse_src("type Color =\n    | Red\n    | Blue");
    let decl = &module.declarations[0];
    if let vox_ast::decl::Decl::TypeDef(td) = decl {
        assert_eq!(td.name, "Color");
        assert_eq!(td.variants.len(), 2);
        assert!(td.fields.is_empty(), "ADT should have no fields");
    } else {
        panic!("expected TypeDef");
    }
}

#[test]
fn struct_type_ts_codegen_emits_interface() {
    let module = parse_src("pub type Point:\n    x: float\n    y: float");
    let out = vox_codegen_ts::adt::generate_types(&module);
    assert!(out.contains("export interface Point {"), "got:\n{}", out);
    assert!(out.contains("readonly x: number;"), "got:\n{}", out);
    assert!(out.contains("readonly y: number;"), "got:\n{}", out);
    assert!(
        !out.contains("_tag"),
        "struct must not have _tag discriminant"
    );
}

#[test]
fn adt_type_ts_codegen_emits_discriminated_union() {
    let module = parse_src("pub type Color =\n    | Red\n    | Blue");
    let out = vox_codegen_ts::adt::generate_types(&module);
    assert!(out.contains("export type Color ="), "got:\n{}", out);
    assert!(out.contains("\"Red\""), "got:\n{}", out);
    assert!(out.contains("\"Blue\""), "got:\n{}", out);
}

#[test]
fn struct_type_rust_codegen_emits_struct() {
    use vox_hir::{DefId, HirModule, HirType, HirTypeDef};

    let module = HirModule {
        types: vec![HirTypeDef {
            id: DefId(1),
            name: "Config".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![
                ("host".to_string(), HirType::Named("str".to_string())),
                ("port".to_string(), HirType::Named("int".to_string())),
            ],
            type_alias: None,
            is_pub: true,
            is_deprecated: false,
            span: Span { start: 0, end: 0 },
        }],
        consts: vec![],
        imports: vec![],
        functions: vec![],
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
        contexts: vec![],
        hooks: vec![],
        providers: vec![],
        ..Default::default()
    };
    let lib_rs = vox_codegen_rust::emit::emit_lib(&module);
    assert!(
        lib_rs.contains("pub struct Config {"),
        "should emit pub struct, got:\n{}",
        &lib_rs[..lib_rs.len().min(1000)]
    );
    assert!(
        lib_rs.contains("pub host: String,"),
        "got:\n{}",
        &lib_rs[..1000]
    );
    assert!(
        lib_rs.contains("pub port: i64,"),
        "got:\n{}",
        &lib_rs[..1000]
    );
    assert!(!lib_rs.contains("pub enum Config"), "should not be an enum");
}

#[test]
fn struct_type_optional_field_maps_correctly() {
    let module = parse_src("type User:\n    name: str\n    bio: Option[str]");
    let out = vox_codegen_ts::adt::generate_types(&module);
    assert!(
        out.contains("readonly bio: string | null;"),
        "got:\n{}",
        out
    );
}

#[test]
fn struct_type_duplicate_field_is_parse_error() {
    let result = try_parse("type Bad:\n    x: str\n    x: int");
    assert!(
        result.is_err(),
        "duplicate fields in struct should produce a parse error"
    );
}

#[test]
fn type_alias_codegen_emits_alias() {
    // 1. Parsing
    let module = parse_src("pub type MyStr = str\npub type OptionalInt = Option[int]");

    // 2. TS Codegen
    let ts_out = vox_codegen_ts::adt::generate_types(&module);
    assert!(
        ts_out.contains("export type MyStr = string;"),
        "got:\n{}",
        ts_out
    );
    assert!(
        ts_out.contains("export type OptionalInt = number | null;"),
        "got:\n{}",
        ts_out
    );

    // 3. Rust Codegen (after lowering)
    let hir = vox_hir::lower_module(&module);
    let rust_out = vox_codegen_rust::emit::emit_lib(&hir);
    assert!(
        rust_out.contains("pub type MyStr = String;"),
        "got:\n{}",
        rust_out
    );
    assert!(
        rust_out.contains("pub type OptionalInt = Option<i64>;"),
        "got:\n{}",
        rust_out
    );
}
