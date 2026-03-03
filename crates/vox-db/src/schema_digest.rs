//! Schema Digest Generator — the LLM Context Engine for VoxDB.
//!
//! Walks `Module` AST declarations and produces a structured `SchemaDigest`
//! that makes the database fully self-describing for AI models.
//!
//! This is the **core differentiator** that makes VoxDB "LLM-first":
//! AI coding assistants using VoxDB always know the exact database shape,
//! field types, relationships, indexes, and can generate accurate queries
//! without guessing.

use serde::{Deserialize, Serialize};
use vox_ast::decl::{Decl, Module};
use vox_ast::types::TypeExpr;

// ── Public Types ────────────────────────────────────────

/// Complete schema digest — the single source of truth for LLM context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDigest {
    /// All declared tables with fields and metadata.
    pub tables: Vec<TableInfo>,
    /// All declared schemaless document collections.
    pub collections: Vec<CollectionInfo>,
    /// Auto-detected relationships from `Id<X>` references.
    pub relationships: Vec<Relationship>,
    /// All declared indexes (standard, vector, search).
    pub indexes: Vec<IndexInfo>,
    /// All declared queries.
    pub queries: Vec<FunctionInfo>,
    /// All declared mutations.
    pub mutations: Vec<FunctionInfo>,
    /// All declared actions.
    pub actions: Vec<FunctionInfo>,
    /// Human-readable summary for LLM prompts.
    pub summary: String,
    /// The exact VCS context snapshot ID this schema belongs to.
    pub vcs_snapshot_id: Option<String>,
}

/// Information about a single database table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    /// User-provided description (from `@describe` decorator, if present).
    pub description: Option<String>,
    /// Auto-generated example insert statement.
    pub example_insert: String,
    /// Auto-generated example query statement.
    pub example_query: String,
    /// Whether public (`is_pub`).
    pub is_public: bool,
    /// Auth provider if configured.
    pub auth_provider: Option<String>,
    /// Sample data (first 3 rows) if populated by the context engine.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sample_data: Vec<serde_json::Value>,
}

/// Information about a single document collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub fields: Vec<FieldInfo>,
    /// User-provided description (from `@describe` decorator, if present).
    pub description: Option<String>,
    pub is_public: bool,
    /// Sample data (first 3 rows) if populated by the context engine.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sample_data: Vec<serde_json::Value>,
}

/// Information about a single table field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    /// Human-readable type string (e.g. "str", "int", "Option[str]").
    pub type_str: String,
    /// Whether this field is optional.
    pub is_optional: bool,
    /// Whether this field references another table (via `Id<X>`).
    pub references_table: Option<String>,
}

/// A detected relationship between tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Table that holds the foreign key.
    pub from_table: String,
    /// Field that contains the reference.
    pub from_field: String,
    /// Table being referenced.
    pub to_table: String,
    /// Kind of relationship.
    pub kind: RelationshipKind,
}

/// Relationship kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipKind {
    /// `Id[X]` → references one record in table X.
    OneToOne,
    /// `List[Id[X]]` → references many records in table X.
    OneToMany,
    /// Inferred from field name matching a table name.
    Inferred,
}

/// Information about an index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub table_name: String,
    pub index_name: String,
    pub kind: IndexKind,
    pub columns: Vec<String>,
    /// For vector indexes: dimensionality.
    pub dimensions: Option<u32>,
    /// For vector/search indexes: filter fields.
    pub filter_fields: Vec<String>,
}

/// The kind of index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexKind {
    Standard,
    Vector,
    Search,
}

/// Information about a query, mutation, or action function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<String>,
    /// Tables this function likely reads from (for queries) or writes to (for mutations).
    pub affected_tables: Vec<String>,
}

/// A function parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub type_str: String,
}

// ── Core API ────────────────────────────────────────────

/// Generate a schema digest from a parsed Vox module.
///
/// This is the primary entry point. Given an AST `Module`, it extracts
/// all database-related declarations and produces a structured digest
/// that can be:
/// 1. Serialized to JSON and served via MCP tools
/// 2. Embedded as comments in codegen output
/// 3. Used for error enrichment
pub fn generate_schema_digest(module: &Module, vcs_snapshot_id: Option<String>) -> SchemaDigest {
    let mut tables = Vec::new();
    let mut collections = Vec::new();
    let mut indexes = Vec::new();
    let mut queries = Vec::new();
    let mut mutations = Vec::new();
    let mut actions = Vec::new();

    // Collect all table names for relationship detection
    let table_names: Vec<String> = module
        .declarations
        .iter()
        .filter_map(|d| match d {
            Decl::Table(t) => Some(t.name.clone()),
            _ => None,
        })
        .collect();

    for decl in &module.declarations {
        match decl {
            Decl::Table(t) => {
                tables.push(extract_table_info(t, &table_names));
            }
            Decl::Collection(c) => {
                collections.push(extract_collection_info(c, &table_names));
            }
            Decl::Index(idx) => {
                indexes.push(IndexInfo {
                    table_name: idx.table_name.clone(),
                    index_name: idx.index_name.clone(),
                    kind: IndexKind::Standard,
                    columns: idx.columns.clone(),
                    dimensions: None,
                    filter_fields: Vec::new(),
                });
            }
            Decl::VectorIndex(vidx) => {
                indexes.push(IndexInfo {
                    table_name: vidx.table_name.clone(),
                    index_name: vidx.index_name.clone(),
                    kind: IndexKind::Vector,
                    columns: vec![vidx.column.clone()],
                    dimensions: Some(vidx.dimensions),
                    filter_fields: vidx.filter_fields.clone(),
                });
            }
            Decl::SearchIndex(sidx) => {
                indexes.push(IndexInfo {
                    table_name: sidx.table_name.clone(),
                    index_name: sidx.index_name.clone(),
                    kind: IndexKind::Search,
                    columns: vec![sidx.search_field.clone()],
                    dimensions: None,
                    filter_fields: sidx.filter_fields.clone(),
                });
            }
            Decl::Query(q) => {
                queries.push(extract_function_info(&q.func, &table_names));
            }
            Decl::Mutation(m) => {
                mutations.push(extract_function_info(&m.func, &table_names));
            }
            Decl::Action(a) => {
                actions.push(extract_function_info(&a.func, &table_names));
            }
            _ => {}
        }
    }

    // Auto-detect relationships from Id<X> references
    let relationships = detect_relationships(&tables);

    // Generate human-readable summary
    let summary = generate_summary(&tables, &indexes, &queries, &mutations, &actions);

    SchemaDigest {
        tables,
        collections,
        relationships,
        indexes,
        queries,
        mutations,
        actions,
        summary,
        vcs_snapshot_id,
    }
}

/// Generate a formatted context block suitable for LLM prompts.
///
/// This is the text that gets injected into AI assistant context so they
/// understand the database without reading source code.
pub fn format_llm_context(digest: &SchemaDigest) -> String {
    let mut out = String::new();
    out.push_str("## Database Context (VoxDB)\n");
    if let Some(snapshot) = &digest.vcs_snapshot_id {
        out.push_str(&format!("*Snapshot: {}*\n\n", snapshot));
    } else {
        out.push_str("\n");
    }

    if !digest.tables.is_empty() {
        out.push_str("### Tables\n\n");
        for table in &digest.tables {
            if let Some(desc) = &table.description {
                out.push_str(&format!("*{}*\n", desc));
            }
            let fields_str: Vec<String> = table
                .fields
                .iter()
                .map(|f| {
                    let opt = if f.is_optional { "?" } else { "" };
                    format!("{}{}:{}", f.name, opt, f.type_str)
                })
                .collect();
            out.push_str(&format!(
                "- **{}** ({} fields): {}\n",
                table.name,
                table.fields.len(),
                fields_str.join(", ")
            ));

            // Show indexes for this table
            let table_indexes: Vec<&IndexInfo> = digest
                .indexes
                .iter()
                .filter(|i| i.table_name == table.name)
                .collect();
            if !table_indexes.is_empty() {
                let idx_str: Vec<String> = table_indexes
                    .iter()
                    .map(|i| {
                        let kind = match i.kind {
                            IndexKind::Standard => "index",
                            IndexKind::Vector => "vector_index",
                            IndexKind::Search => "search_index",
                        };
                        format!("{}({})", kind, i.columns.join(","))
                    })
                    .collect();
                out.push_str(&format!("  - Indexes: {}\n", idx_str.join(", ")));
            }

            // Example queries
            out.push_str(&format!("  - e.g. Insert: `{}`\n", table.example_insert));
            out.push_str(&format!("  - e.g. Query: `{}`\n", table.example_query));

            if !table.sample_data.is_empty() {
                out.push_str("  - Sample Data (first 3 rows):\n");
                if let Ok(json) = serde_json::to_string_pretty(&table.sample_data) {
                    out.push_str(&format!(
                        "    ```json\n    {}\n    ```\n",
                        json.replace('\n', "\n    ")
                    ));
                }
            }
            out.push_str("\n");
        }
    }

    if !digest.collections.is_empty() {
        out.push_str("### Collections\n\n");
        for coll in &digest.collections {
            if let Some(desc) = &coll.description {
                out.push_str(&format!("*{}*\n", desc));
            }
            let fields_str: Vec<String> = coll
                .fields
                .iter()
                .map(|f| {
                    let opt = if f.is_optional { "?" } else { "" };
                    format!("{}{}:{}", f.name, opt, f.type_str)
                })
                .collect();
            out.push_str(&format!(
                "- **{}** (schemaless document store{})\n",
                coll.name,
                if fields_str.is_empty() {
                    String::new()
                } else {
                    format!(", typed fields: {}", fields_str.join(", "))
                }
            ));

            // Show indexes for this collection
            let coll_indexes: Vec<&IndexInfo> = digest
                .indexes
                .iter()
                .filter(|i| i.table_name == coll.name)
                .collect();

            if !coll_indexes.is_empty() {
                let idx_str: Vec<String> = coll_indexes
                    .iter()
                    .map(|i| format!("index({})", i.columns.join(",")))
                    .collect();
                out.push_str(&format!("  - Indexes: {}\n", idx_str.join(", ")));
            }
            out.push_str(&format!(
                "  - e.g. Insert: `db.collection(\"{}\").insert(doc)`\n",
                coll.name
            ));
            out.push_str(&format!(
                "  - e.g. Query: `db.collection(\"{}\").find({{\"field\": \"value\"}})`\n",
                coll.name
            ));

            if !coll.sample_data.is_empty() {
                out.push_str("  - Sample Data (first 3 rows):\n");
                if let Ok(json) = serde_json::to_string_pretty(&coll.sample_data) {
                    out.push_str(&format!(
                        "    ```json\n    {}\n    ```\n",
                        json.replace('\n', "\n    ")
                    ));
                }
            }
            out.push_str("\n");
        }
    }

    if !digest.relationships.is_empty() {
        out.push_str("### Relationships\n\n");
        for rel in &digest.relationships {
            let kind_str = match rel.kind {
                RelationshipKind::OneToOne => "→",
                RelationshipKind::OneToMany => "→*",
                RelationshipKind::Inferred => "~>",
            };
            out.push_str(&format!(
                "- {}.{} {} {}\n",
                rel.from_table, rel.from_field, kind_str, rel.to_table
            ));
        }
        out.push('\n');
    }

    if !digest.queries.is_empty() {
        out.push_str("### Queries (read-only)\n\n");
        for q in &digest.queries {
            let params: Vec<String> = q
                .params
                .iter()
                .map(|p| format!("{}:{}", p.name, p.type_str))
                .collect();
            let ret = q.return_type.as_deref().unwrap_or("void");
            out.push_str(&format!(
                "- `{}({})` → {}\n",
                q.name,
                params.join(", "),
                ret
            ));
        }
        out.push('\n');
    }

    if !digest.mutations.is_empty() {
        out.push_str("### Mutations (read-write)\n\n");
        for m in &digest.mutations {
            let params: Vec<String> = m
                .params
                .iter()
                .map(|p| format!("{}:{}", p.name, p.type_str))
                .collect();
            let ret = m.return_type.as_deref().unwrap_or("void");
            out.push_str(&format!(
                "- `{}({})` → {}\n",
                m.name,
                params.join(", "),
                ret
            ));
        }
        out.push('\n');
    }

    if !digest.actions.is_empty() {
        out.push_str("### Actions (side-effects)\n\n");
        for a in &digest.actions {
            let params: Vec<String> = a
                .params
                .iter()
                .map(|p| format!("{}:{}", p.name, p.type_str))
                .collect();
            let ret = a.return_type.as_deref().unwrap_or("void");
            out.push_str(&format!(
                "- `{}({})` → {}\n",
                a.name,
                params.join(", "),
                ret
            ));
        }
        out.push('\n');
    }

    out
}

/// Generate a JSON representation of the schema digest.
pub fn digest_to_json(digest: &SchemaDigest) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(digest)
}

// ── Private Helpers ─────────────────────────────────────

fn extract_table_info(table: &vox_ast::decl::TableDecl, all_table_names: &[String]) -> TableInfo {
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

fn extract_collection_info(
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

fn extract_function_info(func: &vox_ast::decl::FnDecl, all_table_names: &[String]) -> FunctionInfo {
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
        TypeExpr::Union { variants, .. } => {
            let v_strs: Vec<String> = variants.iter().map(type_expr_to_string).collect();
            v_strs.join(" | ")
        }
        TypeExpr::Map { key, value, .. } => {
            format!(
                "Map[{}, {}]",
                type_expr_to_string(key),
                type_expr_to_string(value)
            )
        }
        TypeExpr::Set { element, .. } => {
            format!("Set[{}]", type_expr_to_string(element))
        }
        TypeExpr::Intersection { left, right, .. } => {
            format!(
                "{} & {}",
                type_expr_to_string(left),
                type_expr_to_string(right)
            )
        }
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
fn detect_relationships(tables: &[TableInfo]) -> Vec<Relationship> {
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
fn generate_summary(
    tables: &[TableInfo],
    indexes: &[IndexInfo],
    queries: &[FunctionInfo],
    mutations: &[FunctionInfo],
    actions: &[FunctionInfo],
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
    if !actions.is_empty() {
        parts.push(format!("{} action(s)", actions.len()));
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
    use vox_ast::decl::*;
    use vox_ast::span::Span;
    use vox_ast::types::TypeExpr;

    fn dummy_span() -> Span {
        Span { start: 0, end: 0 }
    }

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
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: Vec::new(),
            cors: None,
            is_pub: false,
            is_deprecated: false,
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

        let digest = generate_schema_digest(&module);
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

        let digest = generate_schema_digest(&module);
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

        let digest = generate_schema_digest(&module);
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

        let digest = generate_schema_digest(&module);
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

        let digest = generate_schema_digest(&module);
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
