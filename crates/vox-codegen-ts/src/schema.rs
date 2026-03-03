use vox_ast::{
    decl::{Decl, Module},
    types::TypeExpr,
};

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

/// Map a Vox TypeExpr to a Convex validator expression (e.g. `v.string()`).
pub fn type_to_voxdb_validator(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, .. } => match name.as_str() {
            "str" => "v.string()".to_string(),
            "int" | "float" | "float64" => "v.number()".to_string(),
            "bool" => "v.boolean()".to_string(),
            "bytes" | "Bytes" => "v.bytes()".to_string(),
            // Custom named types → v.any() with a comment (user should refine)
            other => format!("v.any() /* {} */", other),
        },
        TypeExpr::Generic { name, args, .. } => match name.as_str() {
            "Option" => {
                let inner = args
                    .first()
                    .map(type_to_voxdb_validator)
                    .unwrap_or_else(|| "v.any()".to_string());
                format!("v.optional({})", inner)
            }
            "List" | "list" => {
                let inner = args
                    .first()
                    .map(type_to_voxdb_validator)
                    .unwrap_or_else(|| "v.any()".to_string());
                format!("v.array({})", inner)
            }
            "Id" => {
                let table = args
                    .first()
                    .and_then(|a| {
                        if let TypeExpr::Named { name, .. } = a {
                            Some(name.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or("unknown");
                format!("v.id(\"{}\")", to_camel_case(&table.to_lowercase()))
            }
            "Map" | "map" => "v.any() /* Map */".to_string(),
            "Set" | "set" => "v.any() /* Set */".to_string(),
            _ => format!("v.any() /* {}<...> */", name),
        },
        TypeExpr::Union { variants, .. } => {
            let vs: Vec<String> = variants.iter().map(type_to_voxdb_validator).collect();
            format!("v.union({})", vs.join(", "))
        }
        TypeExpr::Tuple { elements, .. } => {
            let els: Vec<String> = elements.iter().map(type_to_voxdb_validator).collect();
            format!("v.array(v.union({}))", els.join(", "))
        }
        TypeExpr::Map { .. } => "v.any() /* Map */".to_string(),
        TypeExpr::Set { element, .. } => {
            let inner = type_to_voxdb_validator(element);
            format!("v.array({})", inner)
        }
        TypeExpr::Intersection { .. } => "v.any() /* Intersection */".to_string(),
        TypeExpr::Function { .. } => "v.any() /* Function */".to_string(),
        TypeExpr::Unit { .. } => "v.null()".to_string(),
    }
}

/// Map a Vox TypeExpr to a TypeScript type string (for the interface declarations).
fn type_to_ts(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named { name, .. } => match name.as_str() {
            "str" => "string".to_string(),
            "int" | "float" | "float64" => "number".to_string(),
            "bool" => "boolean".to_string(),
            "bytes" | "Bytes" => "ArrayBuffer".to_string(),
            "Unit" => "void".to_string(),
            "Id" => "string".to_string(),
            other => other.to_string(),
        },
        TypeExpr::Generic { name, args, .. } => {
            let args_str: Vec<String> = args.iter().map(type_to_ts).collect();
            match name.as_str() {
                "Option" => format!("{} | null", args_str.join(", ")),
                "List" | "list" => format!(
                    "readonly {}[]",
                    args_str.first().map(|s| s.as_str()).unwrap_or("unknown")
                ),
                "Map" | "map" if args_str.len() == 2 => {
                    format!("Record<{}, {}>", args_str[0], args_str[1])
                }
                "Set" | "set" if !args_str.is_empty() => {
                    format!("Set<{}>", args_str[0])
                }
                "Result" => format!("Result<{}>", args_str.join(", ")),
                "Id" => "string".to_string(),
                _ => format!("{}<{}>", name, args_str.join(", ")),
            }
        }
        TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let params_str: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{i}: {}", type_to_ts(p)))
                .collect();
            format!("({}) => {}", params_str.join(", "), type_to_ts(return_type))
        }
        TypeExpr::Tuple { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(type_to_ts).collect();
            format!("[{}]", elems.join(", "))
        }
        TypeExpr::Unit { .. } => "void".to_string(),
        TypeExpr::Union { variants, .. } => {
            let vars: Vec<String> = variants.iter().map(type_to_ts).collect();
            vars.join(" | ")
        }
        TypeExpr::Intersection { left, right, .. } => {
            format!("{} & {}", type_to_ts(left), type_to_ts(right))
        }
        TypeExpr::Map { key, value, .. } => {
            format!("Record<{}, {}>", type_to_ts(key), type_to_ts(value))
        }
        TypeExpr::Set { element, .. } => {
            format!("Set<{}>", type_to_ts(element))
        }
    }
}

/// Convert a PascalCase or snake_case name to camelCase for VoxDB table keys.
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
