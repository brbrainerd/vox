/// Tests for the Convex schema generator and updated type mappings.
use vox_ast::{
    decl::{Decl, IndexDecl, Module, TableDecl, TableField, VectorIndexDecl},
    span::Span,
    types::TypeExpr,
};
use vox_codegen_ts::generate_voxdb_schema;

fn span() -> Span {
    Span { start: 0, end: 0 }
}

fn named(name: &str) -> TypeExpr {
    TypeExpr::Named {
        name: name.to_string(),
        span: span(),
    }
}

fn generic(name: &str, arg: TypeExpr) -> TypeExpr {
    TypeExpr::Generic {
        name: name.to_string(),
        args: vec![arg],
        span: span(),
    }
}

fn table_module(table_name: &str, fields: Vec<(&str, TypeExpr)>) -> Module {
    Module {
        declarations: vec![Decl::Table(TableDecl {
            name: table_name.to_string(),
            fields: fields
                .into_iter()
                .map(|(fname, ftype)| TableField {
                    name: fname.to_string(),
                    type_ann: ftype,
                    description: None,
                    span: span(),
                })
                .collect(),
            description: None,
            is_pub: true,
            is_deprecated: false,
            json_layout: None,
            auth_provider: None,
            roles: vec![],
            cors: None,
            span: span(),
        })],
        span: span(),
    }
}

#[test]
fn schema_empty_module_produces_no_output() {
    let module = Module {
        declarations: vec![],
        span: span(),
    };
    assert_eq!(generate_voxdb_schema(&module), "");
}

#[test]
fn schema_str_field_maps_to_v_string() {
    let module = table_module("Post", vec![("title", named("str"))]);
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("title: v.string()"), "got:\n{}", out);
    assert!(out.contains("import { defineSchema, defineTable } from \"voxdb/server\""));
}

#[test]
fn schema_int_and_float_map_to_v_number() {
    let module = table_module(
        "Stats",
        vec![
            ("count", named("int")),
            ("score", named("float")),
            ("precision", named("float64")),
        ],
    );
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("count: v.number()"), "got:\n{}", out);
    assert!(out.contains("score: v.number()"), "got:\n{}", out);
    assert!(out.contains("precision: v.number()"), "got:\n{}", out);
}

#[test]
fn schema_bool_maps_to_v_boolean() {
    let module = table_module("User", vec![("active", named("bool"))]);
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("active: v.boolean()"), "got:\n{}", out);
}

#[test]
fn schema_option_str_maps_to_v_optional_v_string() {
    let module = table_module("Post", vec![("subtitle", generic("Option", named("str")))]);
    let out = generate_voxdb_schema(&module);
    assert!(
        out.contains("subtitle: v.optional(v.string())"),
        "got:\n{}",
        out
    );
}

#[test]
fn schema_list_maps_to_v_array() {
    let module = table_module("Task", vec![("tags", generic("List", named("str")))]);
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("tags: v.array(v.string())"), "got:\n{}", out);
}

#[test]
fn schema_id_maps_to_v_id_with_table_name() {
    let module = table_module(
        "Comment",
        vec![(
            "postId",
            TypeExpr::Generic {
                name: "Id".to_string(),
                args: vec![named("Post")],
                span: span(),
            },
        )],
    );
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("postId: v.id(\"post\")"), "got:\n{}", out);
}

#[test]
fn schema_index_appended_to_table() {
    let module = Module {
        declarations: vec![
            Decl::Table(TableDecl {
                name: "Post".to_string(),
                fields: vec![TableField {
                    name: "author".to_string(),
                    type_ann: named("str"),
                    description: None,
                    span: span(),
                }],
                description: None,
                is_pub: true,
                is_deprecated: false,
                json_layout: None,
                auth_provider: None,
                roles: vec![],
                cors: None,
                span: span(),
            }),
            Decl::Index(IndexDecl {
                table_name: "Post".to_string(),
                index_name: "by_author".to_string(),
                columns: vec!["author".to_string()],
                span: span(),
            }),
        ],
        span: span(),
    };
    let out = generate_voxdb_schema(&module);
    assert!(
        out.contains(".index(\"by_author\", [\"author\"])"),
        "got:\n{}",
        out
    );
}

#[test]
fn schema_vector_index_appended_to_table() {
    let module = Module {
        declarations: vec![
            Decl::Table(TableDecl {
                name: "Document".to_string(),
                fields: vec![TableField {
                    name: "embedding".to_string(),
                    type_ann: named("str"),
                    description: None,
                    span: span(),
                }],
                description: None,
                is_pub: true,
                is_deprecated: false,
                json_layout: None,
                auth_provider: None,
                roles: vec![],
                cors: None,
                span: span(),
            }),
            Decl::VectorIndex(VectorIndexDecl {
                table_name: "Document".to_string(),
                index_name: "by_embedding".to_string(),
                column: "embedding".to_string(),
                dimensions: 1536,
                filter_fields: vec!["authorId".to_string()],
                span: span(),
            }),
        ],
        span: span(),
    };
    let out = generate_voxdb_schema(&module);
    assert!(
        out.contains(".vectorIndex(\"by_embedding\", { vectorField: \"embedding\", dimensions: 1536, filterFields: [\"authorId\"] })"),
        "got:\n{}",
        out
    );
}

#[test]
fn schema_search_index_appended_to_table() {
    let module = Module {
        declarations: vec![
            Decl::Table(TableDecl {
                name: "Post".to_string(),
                fields: vec![
                    TableField {
                        name: "title".to_string(),
                        type_ann: named("str"),
                        description: None,
                        span: span(),
                    },
                    TableField {
                        name: "status".to_string(),
                        type_ann: named("str"),
                        description: None,
                        span: span(),
                    },
                ],
                description: None,
                is_pub: true,
                is_deprecated: false,
                json_layout: None,
                auth_provider: None,
                roles: vec![],
                cors: None,
                span: span(),
            }),
            Decl::SearchIndex(vox_ast::decl::SearchIndexDecl {
                table_name: "Post".to_string(),
                index_name: "search_title".to_string(),
                search_field: "title".to_string(),
                filter_fields: vec!["status".to_string()],
                span: span(),
            }),
        ],
        span: span(),
    };
    let out = generate_voxdb_schema(&module);
    assert!(
        out.contains(".searchIndex(\"search_title\", { searchField: \"title\", filterFields: [\"status\"] })"),
        "got:\n{}",
        out
    );
}

#[test]
fn schema_emits_typescript_interface() {
    let module = table_module("User", vec![("name", named("str")), ("age", named("int"))]);
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("export interface User {"), "got:\n{}", out);
    assert!(out.contains("_id: string; // VoxDB ID"), "got:\n{}", out);
    assert!(out.contains("name: string;"), "got:\n{}", out);
    assert!(out.contains("age: number;"), "got:\n{}", out);
}

#[test]
fn schema_table_key_is_camel_case() {
    let module = table_module("BlogPost", vec![("title", named("str"))]);
    let out = generate_voxdb_schema(&module);
    assert!(out.contains("blogPost: defineTable("), "got:\n{}", out);
}
