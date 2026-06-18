//! JSON Schema derivation for workspace @tool parameters (mirrors codegen client emitter).

use vox_compiler::hir::HirParam;
use vox_compiler::hir::HirType;

/// True when a parameter must appear in JSON Schema `required` (no default in HIR).
pub fn param_required_in_schema(param: &HirParam) -> bool {
    param.default.is_none()
}

/// JSON Schema object for one inputSchema.properties entry.
pub fn hir_type_json_schema_property(type_ann: Option<&HirType>) -> serde_json::Value {
    match type_ann {
        Some(HirType::Named(t)) if t == "String" || t == "str" => {
            serde_json::json!({ "type": "string" })
        }
        Some(HirType::Named(t)) if t == "i64" || t == "int" => {
            serde_json::json!({ "type": "integer" })
        }
        Some(HirType::Named(t)) if t == "f64" || t == "float" => {
            serde_json::json!({ "type": "number" })
        }
        Some(HirType::Named(t)) if t == "bool" => serde_json::json!({ "type": "boolean" }),
        Some(HirType::Generic(name, args)) if name == "list" || name == "List" => {
            let item = args
                .first()
                .map(|a| hir_type_json_schema_property(Some(a)))
                .unwrap_or_else(|| serde_json::json!({ "type": "string" }));
            serde_json::json!({ "type": "array", "items": item })
        }
        Some(HirType::Tuple(elems)) => {
            if elems.is_empty() {
                return serde_json::json!({ "type": "array", "maxItems": 0 });
            }
            let items: Vec<_> = elems
                .iter()
                .map(|e| hir_type_json_schema_property(Some(e)))
                .collect();
            let n = elems.len();
            serde_json::json!({
                "type": "array",
                "prefixItems": items,
                "minItems": n,
                "maxItems": n,
            })
        }
        Some(HirType::Unit) => serde_json::json!({ "type": "null" }),
        Some(HirType::Decimal) => serde_json::json!({ "type": "string" }),
        Some(HirType::Function(_, _)) => serde_json::json!({ "type": "string" }),
        _ => serde_json::json!({ "type": "string" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{DefId, HirExpr, HirParam, HirType};

    fn zspan() -> Span {
        Span { start: 0, end: 0 }
    }

    #[test]
    fn str_maps_to_string_schema() {
        let schema = hir_type_json_schema_property(Some(&HirType::Named("str".to_string())));
        assert_eq!(schema["type"], "string");
    }

    #[test]
    fn param_with_default_not_required() {
        let p = HirParam {
            id: DefId(0),
            name: "limit".to_string(),
            type_ann: Some(HirType::Named("int".to_string())),
            default: Some(HirExpr::IntLit(10, zspan())),
            span: zspan(),
        };
        assert!(!param_required_in_schema(&p));
    }

    #[test]
    fn param_without_default_is_required() {
        let p = HirParam {
            id: DefId(0),
            name: "path".to_string(),
            type_ann: Some(HirType::Named("str".to_string())),
            default: None,
            span: zspan(),
        };
        assert!(param_required_in_schema(&p));
    }
}
