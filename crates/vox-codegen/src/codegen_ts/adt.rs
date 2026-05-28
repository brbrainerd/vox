use vox_compiler::hir::{HirModule, HirType, HirTypeDef};

/// Generate TypeScript type definitions from Vox ADTs.
pub fn generate_types(hir: &HirModule) -> String {
    let mut out = String::new();

    for typedef in &hir.types {
        out.push_str(&generate_adt(typedef));
        out.push('\n');
    }

    out
}

/// Generate a TypeScript discriminated union from a Vox ADT,
/// or a plain type alias for a struct typedef.
fn generate_adt(typedef: &HirTypeDef) -> String {
    let mut out = String::new();
    let name = &typedef.name;

    // Struct typedef (`type Foo { f: T, ... }`) → `export type Foo = { f: T, ... }`.
    if typedef.variants.is_empty() && !typedef.fields.is_empty() {
        let fields: Vec<String> = typedef
            .fields
            .iter()
            .map(|(fname, ftype)| format!("readonly {}: {}", fname, map_type_to_ts(ftype)))
            .collect();
        out.push_str(&format!(
            "export type {name} = {{ {} }};\n",
            fields.join("; ")
        ));
        return out;
    }

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
                .map(|(fname, ftype)| {
                    let ts_type = map_type_to_ts(ftype);
                    format!("readonly {}: {ts_type}", fname)
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
                .map(|(fname, ftype)| format!("{}: {}", fname, map_type_to_ts(ftype)))
                .collect();
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|(fname, _)| fname.clone())
                .collect();
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

fn map_type_to_ts(ty: &HirType) -> String {
    vox_compiler::contract_ir::wire_type_to_ts(&vox_compiler::contract_ir::project_type(ty))
}
