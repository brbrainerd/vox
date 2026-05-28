use vox_compiler::ast::decl::{Decl, Module};

use super::digest_types::*;
use super::helpers::*;

// ── Core API ────────────────────────────────────────────

/// Generate a schema digest from a parsed Vox module.
///
/// This is the primary entry point. Given an AST `Module`, it extracts
/// all database-related declarations and produces a structured digest
/// that can be:
/// 1. Serialized to JSON and served via MCP tools
/// 2. Embedded as comments in codegen output
/// 3. Used for error enrichment
///
/// `vcs_snapshot_id` is copied into [`SchemaDigest::vcs_snapshot_id`] for lineage (commit SHA,
/// export id, etc.); pass `None` when unknown.
pub fn generate_schema_digest(module: &Module, vcs_snapshot_id: Option<String>) -> SchemaDigest {
    let mut tables = Vec::new();
    let mut collections = Vec::new();
    let mut indexes = Vec::new();
    let mut queries = Vec::new();
    let mut mutations = Vec::new();

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
            Decl::Endpoint(e) => match e.kind {
                vox_compiler::ast::decl::EndpointKind::Query => {
                    queries.push(extract_function_info(&e.func, &table_names));
                }
                vox_compiler::ast::decl::EndpointKind::Mutation => {
                    mutations.push(extract_function_info(&e.func, &table_names));
                }
                vox_compiler::ast::decl::EndpointKind::Server => {}
            },

            _ => {}
        }
    }

    // Auto-detect relationships from Id<X> references
    let relationships = detect_relationships(&tables);

    // Generate human-readable summary
    let summary = generate_summary(&tables, &indexes, &queries, &mutations);

    SchemaDigest {
        tables,
        collections,
        relationships,
        indexes,
        queries,
        mutations,
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

    out
}

/// Generate a JSON representation of the schema digest.
pub fn digest_to_json(digest: &SchemaDigest) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(digest)
}
