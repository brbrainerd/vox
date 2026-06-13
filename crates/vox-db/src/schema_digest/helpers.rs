use vox_ast::types::TypeExpr;

use super::digest_types::*;

pub(crate) fn extract_table_info(
    table: &vox_ast::decl::TableDecl,
    all_table_names: &[String],
) -> TableInfo {
    let fields: Vec<FieldInfo> = table
        .fields
        .iter()
        .map(|f| extract_field_info(f, all_table_names))
        .collect();

    let example_insert = generate_example_insert(&table.name, &fields);
    let example_query = generate_example_query(&table.name);

    TableInfo {
        name: table.name.clone(),
        fields,
        description: table.description.clone(),
        example_insert,
        example_query,
        is_public: table.is_pub,
        auth_provider: table.auth_provider.clone(),
        sample_data: Vec::new(),
    }
}

pub(crate) fn extract_collection_info(
    collection: &vox_ast::decl::CollectionDecl,
    all_table_names: &[String],
) -> CollectionInfo {
    let fields: Vec<FieldInfo> = collection
        .fields
        .iter()
        .map(|f| extract_field_info(f, all_table_names))
        .collect();

    CollectionInfo {
        name: collection.name.clone(),
        fields,
        description: collection.description.clone(),
        is_public: collection.is_pub,
        sample_data: Vec::new(),
    }
}

fn extract_field_info(field: &vox_ast::decl::TableField, all_table_names: &[String]) -> FieldInfo {
    let type_str = type_expr_to_string(&field.type_ann);
    let is_optional = matches!(&field.type_ann, TypeExpr::Generic { name, .. } if name == "Option");
    let references_table = detect_table_reference(&field.type_ann, all_table_names);

    FieldInfo {
        name: field.name.clone(),
        type_str,
        is_optional,
        references_table,
    }
}

pub(crate) fn extract_function_info(
    func: &vox_ast::decl::FnDecl,
    all_table_names: &[String],
) -> FunctionInfo {
    let params: Vec<ParamInfo> = func
        .params
        .iter()
        .map(|p| ParamInfo {
            name: p.name.clone(),
            type_str: p
                .type_ann
                .as_ref()
                .map(|t| type_expr_to_string(t))
                .unwrap_or_else(|| "unknown".to_string()),
        })
        .collect();

    let return_type = func.return_type.as_ref().map(|t| type_expr_to_string(t));

    // Heuristic: detect which tables this function might affect
    // by looking at type references in params and return type
    let mut affected_tables = Vec::new();
    for p in &func.params {
        if let Some(ref ty) = p.type_ann {
            if let Some(table) = detect_table_reference(ty, all_table_names) {
                if !affected_tables.contains(&table) {
                    affected_tables.push(table);
                }
            }
        }
    }
    if let Some(ref ret) = func.return_type {
        if let Some(table) = detect_table_reference(ret, all_table_names) {
            if !affected_tables.contains(&table) {
                affected_tables.push(table);
            }
        }
    }

    FunctionInfo {
        name: func.name.clone(),
        params,
        return_type,
        affected_tables,
    }
}

/// Convert a TypeExpr to a human-readable string.
fn type_expr_to_string(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, .. } => name.clone(),
        TypeExpr::Generic { name, args, .. } => {
            let arg_strs: Vec<String> = args.iter().map(type_expr_to_string).collect();
            format!("{}[{}]", name, arg_strs.join(", "))
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let param_strs: Vec<String> = params.iter().map(type_expr_to_string).collect();
            format!(
                "fn({}) -> {}",
                param_strs.join(", "),
                type_expr_to_string(return_type)
            )
        }
        TypeExpr::Tuple { elements, .. } => {
            let el_strs: Vec<String> = elements.iter().map(type_expr_to_string).collect();
            format!("({})", el_strs.join(", "))
        }
        TypeExpr::Unit { .. } => "Unit".to_string(),
        TypeExpr::Infer { .. } => "_".to_string(),
        TypeExpr::Decimal { .. } => "dec".to_string(),
    }
}

/// Detect if a type references another table via `Id[TableName]`.
fn detect_table_reference(ty: &TypeExpr, all_table_names: &[String]) -> Option<String> {
    match ty {
        TypeExpr::Generic { name, args, .. } if name == "Id" => {
            if let Some(TypeExpr::Named { name: table, .. }) = args.first() {
                if all_table_names.contains(table) {
                    return Some(table.clone());
                }
            }
            None
        }
        TypeExpr::Generic { name, args, .. } if name == "List" || name == "list" => {
            // Check for List[Id[X]]
            args.first()
                .and_then(|inner| detect_table_reference(inner, all_table_names))
        }
        TypeExpr::Generic { name, args, .. } if name == "Option" => args
            .first()
            .and_then(|inner| detect_table_reference(inner, all_table_names)),
        _ => None,
    }
}

/// Auto-detect relationships from field types.
pub(crate) fn detect_relationships(tables: &[TableInfo]) -> Vec<Relationship> {
    let mut rels = Vec::new();
    let all_names: Vec<String> = tables.iter().map(|t| t.name.clone()).collect();

    for table in tables {
        for field in &table.fields {
            if let Some(ref target) = field.references_table {
                let kind = if field.type_str.starts_with("List[") {
                    RelationshipKind::OneToMany
                } else {
                    RelationshipKind::OneToOne
                };
                rels.push(Relationship {
                    from_table: table.name.clone(),
                    from_field: field.name.clone(),
                    to_table: target.clone(),
                    kind,
                });
            } else {
                // Heuristic: if field name matches a table name (e.g., "owner" ↔ "User"),
                // infer a possible relationship
                let field_lower = field.name.to_lowercase();
                for name in &all_names {
                    if name != &table.name && name.to_lowercase() == field_lower {
                        rels.push(Relationship {
                            from_table: table.name.clone(),
                            from_field: field.name.clone(),
                            to_table: name.clone(),
                            kind: RelationshipKind::Inferred,
                        });
                    }
                }
            }
        }
    }

    rels
}

/// Generate an example insert statement.
fn generate_example_insert(table_name: &str, fields: &[FieldInfo]) -> String {
    let field_examples: Vec<String> = fields
        .iter()
        .filter(|f| !f.is_optional)
        .map(|f| {
            let example_val = example_value_for_type(&f.type_str);
            format!("{}: {}", f.name, example_val)
        })
        .collect();
    format!(
        "db.insert({}, {{ {} }})",
        table_name,
        field_examples.join(", ")
    )
}

/// Generate an example query statement.
fn generate_example_query(table_name: &str) -> String {
    format!("db.query({}).collect()", table_name)
}

/// Generate an example value for a type.
fn example_value_for_type(type_str: &str) -> String {
    match type_str {
        "str" => "\"example\"".to_string(),
        "int" => "0".to_string(),
        "float" | "float64" => "0.0".to_string(),
        "bool" => "false".to_string(),
        "bytes" | "Bytes" => "b\"\"".to_string(),
        _ if type_str.starts_with("Id[") => "\"<id>\"".to_string(),
        _ if type_str.starts_with("List[") => "[]".to_string(),
        _ if type_str.starts_with("Map[") => "{}".to_string(),
        _ => "null".to_string(),
    }
}

/// Generate a human-readable summary string.
pub(crate) fn generate_summary(
    tables: &[TableInfo],
    indexes: &[IndexInfo],
    queries: &[FunctionInfo],
    mutations: &[FunctionInfo],
) -> String {
    let mut parts = Vec::new();
    if !tables.is_empty() {
        let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
        parts.push(format!("{} table(s): {}", tables.len(), names.join(", ")));
    }
    if !indexes.is_empty() {
        parts.push(format!("{} index(es)", indexes.len()));
    }
    if !queries.is_empty() {
        parts.push(format!("{} query/queries", queries.len()));
    }
    if !mutations.is_empty() {
        parts.push(format!("{} mutation(s)", mutations.len()));
    }

    if parts.is_empty() {
        "No database declarations found.".to_string()
    } else {
        format!("VoxDB schema: {}", parts.join(", "))
    }
}

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{digest_to_json, format_llm_context, generate_schema_digest};
    use vox_ast::decl::*;
    use vox_ast::types::TypeExpr;
    use vox_test_harness::spans::dummy_span;

    fn make_field(name: &str, type_name: &str) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: TypeExpr::Named {
                name: type_name.to_string(),
                span: dummy_span(),
            },
            description: None,
            span: dummy_span(),
        }
    }

    fn make_id_field(name: &str, target_table: &str) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: TypeExpr::Generic {
                name: "Id".to_string(),
                args: vec![TypeExpr::Named {
                    name: target_table.to_string(),
                    span: dummy_span(),
                }],
                span: dummy_span(),
            },
            description: None,
            span: dummy_span(),
        }
    }

    fn make_table(name: &str, fields: Vec<TableField>) -> Decl {
        Decl::Table(TableDecl {
            name: name.to_string(),
            fields,
            primary_key: None,
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: Vec::new(),
            cors: None,
            is_pub: false,
            is_deprecated: false,
            is_extern: false,
            source: None,
            span: dummy_span(),
        })
    }

    fn make_index(table: &str, index_name: &str, cols: Vec<&str>) -> Decl {
        Decl::Index(IndexDecl {
            table_name: table.to_string(),
            index_name: index_name.to_string(),
            columns: cols.into_iter().map(String::from).collect(),
            span: dummy_span(),
        })
    }

    #[test]
    fn test_basic_schema_digest() {
        let module = Module {
            declarations: vec![
                make_table(
                    "Task",
                    vec![
                        make_field("title", "str"),
                        make_field("done", "bool"),
                        make_field("priority", "int"),
                    ],
                ),
                make_index("Task", "by_done", vec!["done", "priority"]),
            ],
            span: dummy_span(),
        };

        let digest = generate_schema_digest(&module, None);
        assert_eq!(digest.tables.len(), 1);
        assert_eq!(digest.tables[0].name, "Task");
        assert_eq!(digest.tables[0].fields.len(), 3);
        assert_eq!(digest.indexes.len(), 1);
        assert_eq!(digest.indexes[0].index_name, "by_done");
        assert!(digest.summary.contains("1 table(s)"));
    }

    #[test]
    fn test_relationship_detection() {
        let module = Module {
            declarations: vec![
                make_table("User", vec![make_field("name", "str")]),
                make_table(
                    "Task",
                    vec![make_field("title", "str"), make_id_field("owner", "User")],
                ),
            ],
            span: dummy_span(),
        };

        let digest = generate_schema_digest(&module, None);
        assert_eq!(digest.relationships.len(), 1);
        assert_eq!(digest.relationships[0].from_table, "Task");
        assert_eq!(digest.relationships[0].from_field, "owner");
        assert_eq!(digest.relationships[0].to_table, "User");
    }

    #[test]
    fn test_llm_context_format() {
        let module = Module {
            declarations: vec![make_table(
                "Task",
                vec![make_field("title", "str"), make_field("done", "bool")],
            )],
            span: dummy_span(),
        };

        let digest = generate_schema_digest(&module, None);
        let ctx = format_llm_context(&digest);
        assert!(ctx.contains("## Database Context (VoxDB)"));
        assert!(ctx.contains("**Task**"));
        assert!(ctx.contains("title:str"));
        assert!(ctx.contains("done:bool"));
    }

    #[test]
    fn test_json_serialization() {
        let module = Module {
            declarations: vec![make_table("Note", vec![make_field("body", "str")])],
            span: dummy_span(),
        };

        let digest = generate_schema_digest(&module, None);
        let json = digest_to_json(&digest).unwrap();
        assert!(json.contains("\"name\": \"Note\""));
        assert!(json.contains("\"body\""));
    }

    #[test]
    fn test_empty_module() {
        let module = Module {
            declarations: vec![],
            span: dummy_span(),
        };

        let digest = generate_schema_digest(&module, None);
        assert!(digest.tables.is_empty());
        assert_eq!(digest.summary, "No database declarations found.");
    }

    #[test]
    fn test_type_expr_to_string() {
        assert_eq!(
            type_expr_to_string(&TypeExpr::Named {
                name: "str".to_string(),
                span: dummy_span(),
            }),
            "str"
        );

        assert_eq!(
            type_expr_to_string(&TypeExpr::Generic {
                name: "List".to_string(),
                args: vec![TypeExpr::Named {
                    name: "int".to_string(),
                    span: dummy_span(),
                }],
                span: dummy_span(),
            }),
            "List[int]"
        );

        assert_eq!(
            type_expr_to_string(&TypeExpr::Generic {
                name: "Option".to_string(),
                args: vec![TypeExpr::Named {
                    name: "str".to_string(),
                    span: dummy_span(),
                }],
                span: dummy_span(),
            }),
            "Option[str]"
        );
    }
}
