//! Thread-local typedef registry for `@ai(structured_output = T)` schema emission.
//!
//! Mirrors the `json_as_ctx` precedent: `emit_lib` registers the module's
//! typedefs for the duration of the emit; `emit_llm_function_body` (which only
//! receives the `HirFn`, not the module) resolves the structured-output type
//! name to a full JSON Schema here. Nested user structs recurse with a depth
//! cap so self-referential types cannot hang codegen.

use std::cell::RefCell;

use vox_compiler::hir::{HirType, HirTypeDef};

thread_local! {
    static MODULE_TYPES: RefCell<Vec<HirTypeDef>> = const { RefCell::new(Vec::new()) };
}

/// RAII guard restoring the previously-registered typedefs on drop.
pub(super) struct AiSchemaGuard(Vec<HirTypeDef>);

impl Drop for AiSchemaGuard {
    fn drop(&mut self) {
        MODULE_TYPES.with(|c| *c.borrow_mut() = std::mem::take(&mut self.0));
    }
}

/// Register `types` as the current module's typedefs for schema lookup.
pub(super) fn enter_module_types(types: &[HirTypeDef]) -> AiSchemaGuard {
    let prev = MODULE_TYPES.with(|c| std::mem::replace(&mut *c.borrow_mut(), types.to_vec()));
    AiSchemaGuard(prev)
}

/// Full JSON Schema for the struct typedef named `type_name`, or `None` when
/// the name is unregistered, a sum type, or fieldless (callers fall back to
/// the legacy name-only `response_format`).
pub(super) fn schema_for(type_name: &str) -> Option<serde_json::Value> {
    MODULE_TYPES.with(|c| {
        let types = c.borrow();
        let td = find_struct_typedef(&types, type_name)?;
        Some(schema_for_typedef(td, &types, 0))
    })
}

/// Recursion cap for nested user structs (self-referential types terminate).
const MAX_SCHEMA_DEPTH: u8 = 4;

fn find_struct_typedef<'a>(types: &'a [HirTypeDef], name: &str) -> Option<&'a HirTypeDef> {
    types
        .iter()
        .find(|t| t.name == name && t.variants.is_empty() && !t.fields.is_empty())
}

fn schema_for_typedef(td: &HirTypeDef, types: &[HirTypeDef], depth: u8) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::with_capacity(td.fields.len());
    for (fname, fty) in &td.fields {
        properties.insert(fname.clone(), schema_for_hir_type(fty, types, depth));
        required.push(serde_json::Value::String(fname.clone()));
    }
    serde_json::json!({
        "type": "object",
        "properties": serde_json::Value::Object(properties),
        "required": required,
        "additionalProperties": false
    })
}

fn schema_for_hir_type(ty: &HirType, types: &[HirTypeDef], depth: u8) -> serde_json::Value {
    use serde_json::json;
    if depth >= MAX_SCHEMA_DEPTH {
        return json!({});
    }
    match ty {
        HirType::Named(n) => match n.as_str() {
            "int" => json!({"type": "integer"}),
            "float" => json!({"type": "number"}),
            "bool" => json!({"type": "boolean"}),
            "str" => json!({"type": "string"}),
            other => match find_struct_typedef(types, other) {
                Some(td) => schema_for_typedef(td, types, depth + 1),
                None => json!({}),
            },
        },
        HirType::Generic(outer, args) if outer == "list" || outer == "List" => {
            let items = args
                .first()
                .map(|a| schema_for_hir_type(a, types, depth + 1))
                .unwrap_or_else(|| json!({}));
            json!({"type": "array", "items": items})
        }
        HirType::Generic(outer, args) if outer == "Option" => {
            let inner = args
                .first()
                .map(|a| schema_for_hir_type(a, types, depth + 1))
                .unwrap_or_else(|| json!({}));
            json!({"anyOf": [inner, {"type": "null"}]})
        }
        HirType::Decimal => json!({"type": "string"}),
        _ => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::DefId;

    fn stub_typedef() -> HirTypeDef {
        HirTypeDef {
            id: DefId(1),
            name: "StubDto".into(),
            variants: vec![],
            fields: vec![
                ("ok".into(), HirType::Named("bool".into())),
                ("score".into(), HirType::Named("int".into())),
                (
                    "tags".into(),
                    HirType::Generic("list".into(), vec![HirType::Named("str".into())]),
                ),
            ],
            is_pub: true,
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn schema_for_struct_maps_scalars_lists_and_closes_object() {
        let td = stub_typedef();
        let _guard = enter_module_types(std::slice::from_ref(&td));
        let schema = schema_for("StubDto").expect("registered struct yields a schema");
        assert_eq!(
            schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "ok": {"type": "boolean"},
                    "score": {"type": "integer"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                },
                "required": ["ok", "score", "tags"],
                "additionalProperties": false
            })
        );
    }

    #[test]
    fn schema_for_unknown_type_is_none() {
        let td = stub_typedef();
        let _guard = enter_module_types(std::slice::from_ref(&td));
        assert!(schema_for("NotRegistered").is_none());
    }

    #[test]
    fn schema_for_is_none_outside_guard() {
        assert!(schema_for("StubDto").is_none());
    }

    #[test]
    fn nested_user_struct_recurses_and_option_is_nullable() {
        let inner = HirTypeDef {
            id: DefId(2),
            name: "Inner".into(),
            variants: vec![],
            fields: vec![("n".into(), HirType::Named("int".into()))],
            is_pub: true,
            span: Span::new(0, 0),
        };
        let outer = HirTypeDef {
            id: DefId(3),
            name: "Outer".into(),
            variants: vec![],
            fields: vec![
                ("inner".into(), HirType::Named("Inner".into())),
                (
                    "maybe".into(),
                    HirType::Generic("Option".into(), vec![HirType::Named("str".into())]),
                ),
            ],
            is_pub: true,
            span: Span::new(0, 0),
        };
        let types = vec![inner, outer];
        let _guard = enter_module_types(&types);
        let schema = schema_for("Outer").expect("schema");
        assert_eq!(
            schema["properties"]["inner"],
            serde_json::json!({
                "type": "object",
                "properties": {"n": {"type": "integer"}},
                "required": ["n"],
                "additionalProperties": false
            })
        );
        assert_eq!(
            schema["properties"]["maybe"],
            serde_json::json!({"anyOf": [{"type": "string"}, {"type": "null"}]})
        );
    }

    #[test]
    fn decimal_scalar_maps_to_string() {
        let td = HirTypeDef {
            id: DefId(4),
            name: "PriceDto".into(),
            variants: vec![],
            fields: vec![("amount".into(), HirType::Decimal)],
            is_pub: true,
            span: Span::new(0, 0),
        };
        let _guard = enter_module_types(std::slice::from_ref(&td));
        let schema = schema_for("PriceDto").expect("schema");
        assert_eq!(
            schema["properties"]["amount"],
            serde_json::json!({"type": "string"})
        );
    }

    #[test]
    fn self_referential_type_terminates_at_depth_cap() {
        // `Node` has a field of its own type name, so naive recursion would
        // never terminate. `MAX_SCHEMA_DEPTH` must cut it off.
        let node = HirTypeDef {
            id: DefId(5),
            name: "Node".into(),
            variants: vec![],
            fields: vec![
                ("value".into(), HirType::Named("int".into())),
                ("next".into(), HirType::Named("Node".into())),
            ],
            is_pub: true,
            span: Span::new(0, 0),
        };
        let types = vec![node];
        let _guard = enter_module_types(&types);

        // Must return promptly (not hang) and produce a schema.
        let schema = schema_for("Node").expect("schema");

        // Walk down the `next` chain from the top-level object (built at
        // depth 0). Each hop recurses into `schema_for_typedef` at depth+1;
        // the hop that would recurse at `depth == MAX_SCHEMA_DEPTH` must hit
        // the catch-all `{}` instead. That is the (MAX_SCHEMA_DEPTH + 1)-th hop.
        let mut current = &schema;
        for _ in 0..=MAX_SCHEMA_DEPTH {
            assert_eq!(current["type"], serde_json::json!("object"));
            current = &current["properties"]["next"];
        }
        assert_eq!(*current, serde_json::json!({}));
    }
}
