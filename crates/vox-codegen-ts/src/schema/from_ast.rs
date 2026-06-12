use vox_compiler::ast::decl::{Decl, Module};

use super::type_maps::{to_camel_case, type_to_ts, type_to_voxdb_validator};

/// Generate a VoxDB `schema.ts` from all @table, @index, and @vector_index declarations.
///
/// Emits `defineSchema({ tableName: defineTable({ ... }) })` with proper VoxDB validators.
pub fn generate_voxdb_schema(module: &Module) -> String {
    let mut tables = Vec::new();
    let mut collections = Vec::new();
    let mut indexes: Vec<(String, String, Vec<String>)> = Vec::new(); // (table, name, cols)
    let mut vector_indexes: Vec<(String, String, String, u32, Vec<String>)> = Vec::new(); // (table, name, field, dims, filters)
    let mut search_indexes: Vec<(String, String, String, Vec<String>)> = Vec::new(); // (table, name, field, filters)

    for decl in &module.declarations {
        match decl {
            Decl::Table(t) => tables.push(t),
            Decl::Collection(c) => collections.push(c),
            Decl::Index(idx) => {
                indexes.push((
                    idx.table_name.clone(),
                    idx.index_name.clone(),
                    idx.columns.clone(),
                ));
            }
            Decl::VectorIndex(vidx) => {
                vector_indexes.push((
                    vidx.table_name.clone(),
                    vidx.index_name.clone(),
                    vidx.column.clone(),
                    vidx.dimensions,
                    vidx.filter_fields.clone(),
                ));
            }
            Decl::SearchIndex(sidx) => {
                search_indexes.push((
                    sidx.table_name.clone(),
                    sidx.index_name.clone(),
                    sidx.search_field.clone(),
                    sidx.filter_fields.clone(),
                ));
            }
            _ => {}
        }
    }

    if tables.is_empty() && collections.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("import { defineSchema, defineTable } from \"voxdb/server\";\n");
    out.push_str("import { v } from \"voxdb/values\";\n\n");
    out.push_str("export default defineSchema({\n");

    for table in &tables {
        let table_key = to_camel_case(&table.name);
        out.push_str(&format!("  {}: defineTable({{\n", table_key));
        for field in &table.fields {
            let validator = type_to_voxdb_validator(&field.type_ann);
            out.push_str(&format!("    {}: {},\n", field.name, validator));
        }
        out.push_str("  })");

        // Append indexes for this table
        for (tbl, idx_name, cols) in &indexes {
            if tbl == &table.name {
                let cols_str: Vec<String> = cols.iter().map(|c| format!("\"{}\"", c)).collect();
                out.push_str(&format!(
                    "\n    .index(\"{}\", [{}])",
                    idx_name,
                    cols_str.join(", ")
                ));
            }
        }

        // Append vector indexes for this table
        for (tbl, idx_name, field, dims, filters) in &vector_indexes {
            if tbl == &table.name {
                let filters_str = if filters.is_empty() {
                    "".to_string()
                } else {
                    format!(
                        ", filterFields: [{}]",
                        filters
                            .iter()
                            .map(|f| format!("\"{}\"", f))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                out.push_str(&format!(
                    "\n    .vectorIndex(\"{}\", {{ vectorField: \"{}\", dimensions: {}{} }})",
                    idx_name, field, dims, filters_str
                ));
            }
        }

        // Append search indexes for this table
        for (tbl, idx_name, field, filters) in &search_indexes {
            if tbl == &table.name {
                let filters_str = if filters.is_empty() {
                    "".to_string()
                } else {
                    format!(
                        ", filterFields: [{}]",
                        filters
                            .iter()
                            .map(|f| format!("\"{}\"", f))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                out.push_str(&format!(
                    "\n    .searchIndex(\"{}\", {{ searchField: \"{}\"{} }})",
                    idx_name, field, filters_str
                ));
            }
        }

        out.push_str(",\n");
    }

    for coll in &collections {
        let coll_key = to_camel_case(&coll.name);
        out.push_str(&format!("  {}: defineTable({{\n", coll_key));

        // Output validators for explicitly typed fields, if any
        for field in &coll.fields {
            let validator = type_to_voxdb_validator(&field.type_ann);
            out.push_str(&format!("    {}: {},\n", field.name, validator));
        }

        // Schemaless store: allow any other fields by default
        out.push_str("  })");

        // Append indexes for this collection
        for (tbl, idx_name, cols) in &indexes {
            if tbl == &coll.name {
                let cols_str: Vec<String> = cols.iter().map(|c| format!("\"{}\"", c)).collect();
                out.push_str(&format!(
                    "\n    .index(\"{}\", [{}])",
                    idx_name,
                    cols_str.join(", ")
                ));
            }
        }

        out.push_str(",\n");
    }

    out.push_str("});\n\n");

    // Also emit TypeScript interfaces alongside the schema for client-side typing
    out.push_str("// TypeScript interfaces for client-side type safety\n");
    for table in &tables {
        out.push_str(&format!("export interface {} {{\n", table.name));
        out.push_str("  _id: string; // VoxDB ID\n");
        out.push_str("  _creationTime: number;\n");
        for field in &table.fields {
            let ts_type = type_to_ts(&field.type_ann);
            out.push_str(&format!("  {}: {};\n", field.name, ts_type));
        }
        out.push_str("}\n\n");
    }

    for coll in &collections {
        out.push_str(&format!("export interface {} {{\n", coll.name));
        out.push_str("  _id: string; // VoxDB ID\n");
        out.push_str("  _creationTime: number;\n");
        for field in &coll.fields {
            let ts_type = type_to_ts(&field.type_ann);
            out.push_str(&format!("  {}: {};\n", field.name, ts_type));
        }
        out.push_str("  [key: string]: any; // Schemaless fields\n");
        out.push_str("}\n\n");
    }

    out
}
