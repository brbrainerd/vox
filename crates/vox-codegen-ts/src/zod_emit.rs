//! Zod schema emit driven by Contract IR.
//!
//! This module reads [`vox_compiler::contract_ir::ContractIr`] rather than walking HIR
//! directly. The wire-format-v1 rules (Decimal/BigInt → string, Option →
//! optional, sum types → `_tag`-discriminated unions) live in
//! `vox_compiler::contract_ir::project` (the function) — Zod, OpenAPI, JSON Schema, and the TS
//! client SDK all share that single projection.

use vox_compiler::contract_ir::{
    ContractIr, ContractType, ContractTypeKind, ContractVariant, WireType,
};
use vox_compiler::hir::{HirModule, HirType};

/// Generate TypeScript Zod schema definitions from a Vox HIR module.
pub fn generate_zod_schemas(hir: &HirModule) -> String {
    if hir.types.is_empty() && hir.tables.is_empty() {
        return String::new();
    }
    let ir = vox_compiler::contract_ir::project(hir);
    emit_from_contract(&ir)
}

fn emit_from_contract(ir: &ContractIr) -> String {
    let mut out = String::new();
    if ir.types.is_empty() {
        return out;
    }
    out.push_str("import { z } from \"zod\";\n\n");
    for t in &ir.types {
        out.push_str(&emit_type(t));
        out.push('\n');
    }
    out
}

fn emit_type(t: &ContractType) -> String {
    let mut out = String::new();
    match &t.kind {
        ContractTypeKind::Struct { fields } if fields.is_empty() => {
            // Distinguish a declared empty struct from a sum type.
            out.push_str(&format!(
                "export const {}Schema = z.object({{}});\n",
                t.name
            ));
        }
        ContractTypeKind::Struct { fields } => {
            out.push_str(&format!("export const {}Schema = z.object({{\n", t.name));
            for f in fields {
                out.push_str(&format!(
                    "  {}: {},\n",
                    f.name,
                    field_zod(&f.ty, f.optional)
                ));
            }
            out.push_str("});\n");
        }
        ContractTypeKind::Sum { variants } => {
            out.push_str(&emit_sum(&t.name, variants));
        }
    }
    out
}

fn emit_sum(name: &str, variants: &[ContractVariant]) -> String {
    let mut out = String::new();
    if variants.len() == 1 {
        let v = &variants[0];
        out.push_str(&format!("export const {}Schema = z.object({{\n", name));
        out.push_str(&format!("  _tag: z.literal(\"{}\"),\n", v.tag));
        for f in &v.fields {
            out.push_str(&format!(
                "  {}: {},\n",
                f.name,
                field_zod(&f.ty, f.optional)
            ));
        }
        out.push_str("});\n");
        return out;
    }
    out.push_str(&format!(
        "export const {}Schema = z.discriminatedUnion(\"_tag\", [\n",
        name
    ));
    for v in variants {
        out.push_str(&format!(
            "  z.object({{\n    _tag: z.literal(\"{}\"),\n",
            v.tag
        ));
        for f in &v.fields {
            out.push_str(&format!(
                "    {}: {},\n",
                f.name,
                field_zod(&f.ty, f.optional)
            ));
        }
        out.push_str("  }),\n");
    }
    out.push_str("]);\n");
    out
}

fn field_zod(ty: &WireType, optional: bool) -> String {
    let base = wire_zod(ty);
    if optional {
        format!("{}.optional()", base)
    } else {
        base
    }
}

/// Delegates to [`vox_compiler::contract_ir::wire_type_to_zod`] — the single
/// authoritative `WireType → Zod` mapping.
fn wire_zod(ty: &WireType) -> String {
    vox_compiler::contract_ir::wire_type_to_zod(ty)
}

/// Legacy helper kept for callers that still hand-build Zod from `HirType`.
/// Prefer routing through Contract IR.
pub fn map_type_to_zod(ty: &HirType) -> String {
    wire_zod(&vox_compiler::contract_ir::project_type(ty))
}
