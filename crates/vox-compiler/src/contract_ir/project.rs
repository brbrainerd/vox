//! HIR → Contract IR projection.
//!
//! All wire-format-v1 mapping rules live here. If a Vox source type is not
//! representable on the wire, this layer either widens to a string-encoded
//! form (Decimal, BigInt, DateTime) or falls back to [`WireType::Unknown`] —
//! never silently lossy.

use super::{
    ContractEndpoint, ContractField, ContractType, ContractTypeKind, ContractVariant, WireType,
};
use crate::hir::{HirEndpointFn, HirType, HirTypeDef};

/// Lower a `HirTypeDef` into a `ContractType`.
pub(super) fn type_def(t: &HirTypeDef) -> ContractType {
    let kind = if !t.variants.is_empty() {
        ContractTypeKind::Sum {
            variants: t
                .variants
                .iter()
                .map(|v| ContractVariant {
                    tag: v.name.clone(),
                    fields: v.fields.iter().map(field).collect(),
                })
                .collect(),
        }
    } else {
        ContractTypeKind::Struct {
            fields: t.fields.iter().map(field).collect(),
        }
    };
    ContractType {
        name: t.name.clone(),
        kind,
    }
}

/// Lower a `HirEndpointFn` into a `ContractEndpoint`.
pub(super) fn endpoint(e: &HirEndpointFn) -> ContractEndpoint {
    let kind = super::ContractEndpointKind::from(e.kind);
    let method = kind.default_method();
    let response = match &e.return_type {
        Some(t) => ty(t),
        None => WireType::Unit,
    };
    let params = e
        .params
        .iter()
        .map(|p| {
            let inner_ty = p
                .type_ann
                .as_ref()
                .map_or(WireType::Unknown, |t| unwrap_optional(t).1);
            let optional = p.type_ann.as_ref().is_some_and(|t| unwrap_optional(t).0);
            ContractField {
                name: p.name.clone(),
                ty: inner_ty,
                optional,
            }
        })
        .collect();
    ContractEndpoint {
        kind,
        name: e.name.clone(),
        method,
        path: e.route_path.clone(),
        params,
        response,
        is_pure: e.is_pure,
    }
}

fn field((name, ty_): &(String, HirType)) -> ContractField {
    let (optional, inner) = unwrap_optional(ty_);
    ContractField {
        name: name.clone(),
        ty: inner,
        optional,
    }
}

/// `Option<T>` lowers to a non-optional `T` flagged as `optional` per
/// wire-format-v1's "absent key" rule. Returns `(was_option, projected_inner)`.
fn unwrap_optional(t: &HirType) -> (bool, WireType) {
    if let HirType::Generic(name, args) = t
        && name == "Option"
        && args.len() == 1
    {
        return (true, ty(&args[0]));
    }
    (false, ty(t))
}

/// Map a HIR type to its wire-format-v1 representation.
pub(super) fn ty(t: &HirType) -> WireType {
    match t {
        HirType::Named(name) => named(name),
        HirType::Generic(name, args) => generic(name, args),
        HirType::Function(_, _) => WireType::Unknown,
        HirType::Tuple(elems) => WireType::Tuple(elems.iter().map(ty).collect()),
        HirType::Unit => WireType::Unit,
        HirType::Decimal => WireType::DecimalString,
    }
}

fn named(name: &str) -> WireType {
    match name {
        "int" | "i32" | "i64" | "u32" | "u64" | "f32" | "f64" | "float" | "number" => {
            WireType::Number
        }
        "str" | "string" | "String" => WireType::String,
        "bool" | "boolean" => WireType::Bool,
        "Decimal" | "decimal" => WireType::DecimalString,
        // Per wire-format-v1: integers wider than safe-JSON range encode as
        // string. Vox `bigint` aliases or 128-bit named types fall here.
        "bigint" | "BigInt" | "i128" | "u128" => WireType::BigIntString,
        // Vox stdlib date/time aliases. Anything called by these names should
        // round-trip as RFC 3339 UTC strings.
        "Date" | "DateTime" | "Instant" | "Timestamp" => WireType::DateTimeString,
        "Unit" | "()" => WireType::Unit,
        // Anything else is a reference to a user-declared type.
        other => WireType::Ref(other.to_string()),
    }
}

fn generic(name: &str, args: &[HirType]) -> WireType {
    match name {
        "list" | "List" | "Vec" | "Array" if args.len() == 1 => {
            WireType::Array(Box::new(ty(&args[0])))
        }
        "Option" if args.len() == 1 => {
            // Bare `Option<T>` outside an option-aware position widens to its
            // inner type — the absent-key encoding lives on the *field*, not
            // on the wire alphabet itself.
            ty(&args[0])
        }
        // Result<T, E>, Map<K, V>, etc. — emitters handle these case-by-case
        // through `Ref` if the user has declared an alias, otherwise opaque.
        _ => WireType::Unknown,
    }
}
