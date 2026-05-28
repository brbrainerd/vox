use vox_compiler::hir::HirModule;

use super::type_maps::{hir_type_to_ts, hir_type_to_voxdb_validator, to_camel_case};

/// Same as `generate_voxdb_schema` but reads canonical HIR vectors (tables, collections, indexes).
///
/// Use this on `HirModule` after lowering so schema emission stays aligned with `legacy_ast_nodes` removal.
pub fn generate_voxdb_schema_from_hir(module: &HirModule) -> String {
    let mut indexes: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut vector_indexes: Vec<(String, String, String, u32, Vec<String>)> = Vec::new();
    let mut search_indexes: Vec<(String, String, String, Vec<String>)> = Vec::new();

    for idx in &module.indexes {
        indexes.push((
            idx.table_name.clone(),
            idx.index_name.clone(),
            idx.columns.clone(),
        ));
    }
    for vidx in &module.vector_indexes {
        vector_indexes.push((
            vidx.table_name.clone(),
            vidx.index_name.clone(),
            vidx.column.clone(),
            vidx.dimensions,
            vidx.filter_fields.clone(),
        ));
    }
    for sidx in &module.search_indexes {
        search_indexes.push((
            sidx.table_name.clone(),
            sidx.index_name.clone(),
            sidx.search_field.clone(),
            sidx.filter_fields.clone(),
        ));
    }

    if module.tables.is_empty() && module.collections.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("import { defineSchema, defineTable } from \"voxdb/server\";\n");
    out.push_str("import { v } from \"voxdb/values\";\n\n");
    out.push_str("export default defineSchema({\n");

    for table in &module.tables {
        let table_key = to_camel_case(&table.name);
        out.push_str(&format!("  {}: defineTable({{\n", table_key));
        for field in &table.fields {
            let validator = hir_type_to_voxdb_validator(&field.type_ann);
            out.push_str(&format!("    {}: {},\n", field.name, validator));
        }
        out.push_str("  })");

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

    for coll in &module.collections {
        let coll_key = to_camel_case(&coll.name);
        out.push_str(&format!("  {}: defineTable({{\n", coll_key));

        for field in &coll.fields {
            let validator = hir_type_to_voxdb_validator(&field.type_ann);
            out.push_str(&format!("    {}: {},\n", field.name, validator));
        }

        out.push_str("  })");

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

    out.push_str("// TypeScript interfaces for client-side type safety\n");
    for table in &module.tables {
        out.push_str(&format!("export interface {} {{\n", table.name));
        out.push_str("  _id: string; // VoxDB ID\n");
        out.push_str("  _creationTime: number;\n");
        for field in &table.fields {
            let ts_type = hir_type_to_ts(&field.type_ann);
            out.push_str(&format!("  {}: {};\n", field.name, ts_type));
        }
        out.push_str("}\n\n");
    }

    for coll in &module.collections {
        out.push_str(&format!("export interface {} {{\n", coll.name));
        out.push_str("  _id: string; // VoxDB ID\n");
        out.push_str("  _creationTime: number;\n");
        for field in &coll.fields {
            let ts_type = hir_type_to_ts(&field.type_ann);
            out.push_str(&format!("  {}: {};\n", field.name, ts_type));
        }
        out.push_str("  [key: string]: any; // Schemaless fields\n");
        out.push_str("}\n\n");
    }

    out
}
