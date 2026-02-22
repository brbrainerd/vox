use vox_ast::decl::{Decl, Module, TypeDefDecl};

/// Generate TypeScript type definitions from Vox ADTs.
pub fn generate_types(module: &Module) -> String {
    let mut out = String::new();

    for decl in &module.declarations {
        if let Decl::TypeDef(typedef) = decl {
            out.push_str(&generate_adt(typedef));
            out.push('\n');
        }
    }

    out
}

/// Generate a TypeScript discriminated union from a Vox ADT.
fn generate_adt(typedef: &TypeDefDecl) -> String {
    let mut out = String::new();
    let name = &typedef.name;

    // Generate the union type
    out.push_str(&format!("export type {name} =\n"));
    for (i, variant) in typedef.variants.iter().enumerate() {
        let separator = if i < typedef.variants.len() - 1 {
            ""
        } else {
            ";"
        };
        if variant.fields.is_empty() {
            out.push_str(&format!(
                "  | {{ readonly _tag: \"{}\" }}{separator}\n",
                variant.name
            ));
        } else {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| {
                    let ts_type = map_type_to_ts(&f.type_ann);
                    format!("readonly {}: {ts_type}", f.name)
                })
                .collect();
            out.push_str(&format!(
                "  | {{ readonly _tag: \"{}\"; {} }}{separator}\n",
                variant.name,
                fields.join("; ")
            ));
        }
    }
    out.push('\n');

    // Generate constructor functions
    for variant in &typedef.variants {
        if variant.fields.is_empty() {
            out.push_str(&format!(
                "export const {}: {name} = {{ _tag: \"{}\" }};\n",
                variant.name, variant.name
            ));
        } else {
            let params: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, map_type_to_ts(&f.type_ann)))
                .collect();
            let fields: Vec<String> = variant.fields.iter().map(|f| f.name.clone()).collect();
            out.push_str(&format!(
                "export const {} = ({}): {name} => ({{ _tag: \"{}\", {} }});\n",
                variant.name,
                params.join(", "),
                variant.name,
                fields.join(", ")
            ));
        }
    }

    out
}

fn map_type_to_ts(ty: &vox_ast::types::TypeExpr) -> String {
    match ty {
        vox_ast::types::TypeExpr::Named { name, .. } => match name.as_str() {
            "int" | "float" => "number".to_string(),
            "str" => "string".to_string(),
            "bool" => "boolean".to_string(),
            other => other.to_string(),
        },
        vox_ast::types::TypeExpr::Generic { name, args, .. } => {
            let args_str: Vec<String> = args.iter().map(map_type_to_ts).collect();
            format!("{}<{}>", name, args_str.join(", "))
        }
        _ => "any".to_string(),
    }
}
