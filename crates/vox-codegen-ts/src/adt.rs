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

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::nodes::{DefId, HirType};
    use vox_compiler::hir::{HirModule, HirTypeDef, HirVariant};

    fn span() -> Span {
        Span::new(0, 0)
    }
    fn id() -> DefId {
        DefId(0)
    }

    // Helper: build a minimal HirModule with no fields set except types.
    fn module_with_types(types: Vec<HirTypeDef>) -> HirModule {
        HirModule {
            types,
            ..Default::default()
        }
    }

    #[test]
    fn generate_types_struct_typedef_emits_export_type_alias() {
        // A HirTypeDef with no variants and non-empty fields → struct path.
        let typedef = HirTypeDef {
            id: id(),
            name: "Point".to_string(),
            variants: vec![],
            fields: vec![
                ("x".to_string(), HirType::Named("float".to_string())),
                ("y".to_string(), HirType::Named("float".to_string())),
            ],
            is_pub: true,
            span: span(),
        };
        let hir = module_with_types(vec![typedef]);
        let out = generate_types(&hir);
        // Must start with `export type Point =`
        assert!(out.contains("export type Point ="), "got: {out}");
        // Must contain both field names
        assert!(out.contains("readonly x:"), "got: {out}");
        assert!(out.contains("readonly y:"), "got: {out}");
        // Must NOT contain union syntax for a struct typedef
        assert!(
            !out.contains("_tag"),
            "struct typedef should not emit _tag: {out}"
        );
    }

    #[test]
    fn generate_types_adt_unit_variant_emits_discriminated_union_and_constructor() {
        // ADT with one unit variant → discriminated union + const constructor
        let typedef = HirTypeDef {
            id: id(),
            name: "Color".to_string(),
            variants: vec![
                HirVariant {
                    name: "Red".to_string(),
                    fields: vec![],
                    span: span(),
                },
                HirVariant {
                    name: "Blue".to_string(),
                    fields: vec![],
                    span: span(),
                },
            ],
            fields: vec![],
            is_pub: true,
            span: span(),
        };
        let hir = module_with_types(vec![typedef]);
        let out = generate_types(&hir);
        // Union head
        assert!(out.contains("export type Color ="), "got: {out}");
        // Unit variants use _tag with their name
        assert!(out.contains("\"Red\""), "got: {out}");
        assert!(out.contains("\"Blue\""), "got: {out}");
        // Const constructor for the final variant has no semicolon after the union arm
        assert!(out.contains("export const Red:"), "got: {out}");
        assert!(out.contains("export const Blue:"), "got: {out}");
    }

    #[test]
    fn generate_types_adt_variant_with_fields_emits_arrow_constructor() {
        // Variant with fields → arrow function constructor
        let typedef = HirTypeDef {
            id: id(),
            name: "Shape".to_string(),
            variants: vec![HirVariant {
                name: "Circle".to_string(),
                fields: vec![("radius".to_string(), HirType::Named("float".to_string()))],
                span: span(),
            }],
            fields: vec![],
            is_pub: true,
            span: span(),
        };
        let hir = module_with_types(vec![typedef]);
        let out = generate_types(&hir);
        // Arrow constructor signature present
        assert!(out.contains("export const Circle = ("), "got: {out}");
        assert!(out.contains("): Shape =>"), "got: {out}");
        assert!(out.contains("_tag: \"Circle\""), "got: {out}");
    }
}
