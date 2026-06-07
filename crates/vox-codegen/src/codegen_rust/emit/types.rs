use vox_compiler::hir::HirType;

pub(crate) fn emit_type(ty: &HirType) -> String {
    match ty {
        HirType::Named(n) => match n.as_str() {
            "int" => "i64".into(),
            "float" => "f64".into(),
            "bool" => "bool".into(),
            "str" => "String".into(),
            "Element" | "Result" | "Any" | "Json" => "serde_json::Value".into(),
            other => other.to_string(),
        },
        HirType::Generic(n, args) => {
            let args_str: Vec<_> = args.iter().map(emit_type).collect();
            match n.as_str() {
                // Id[Task] → i64 (SQLite rowid)
                "Id" => "i64".into(),
                "List" | "list" => format!(
                    "Vec<{}>",
                    args_str.first().unwrap_or(&"serde_json::Value".to_string())
                ),
                "Option" => format!(
                    "Option<{}>",
                    args_str.first().unwrap_or(&"serde_json::Value".to_string())
                ),
                // Deprecated aliases — emit DurablePromise during the v0.6→v0.7 migration window.
                "Future" | "Promise" => format!(
                    "DurablePromise<{}>",
                    args_str.first().unwrap_or(&"serde_json::Value".to_string())
                ),
                _ => format!("{}<{}>", n, args_str.join(", ")),
            }
        }
        HirType::Unit => "()".into(),
        HirType::Decimal => "rust_decimal::Decimal".into(),
        // Function types (closures / HOF params + returns) → boxed trait objects,
        // the idiomatic Rust shape for closures with captured state. Without this
        // they fell through to `serde_json::Value`, breaking closure-typed vars
        // (E0282/E0308).
        HirType::Function(params, ret) => {
            let param_types: Vec<String> = params.iter().map(emit_type).collect();
            format!(
                "Box<dyn Fn({}) -> {}>",
                param_types.join(", "),
                emit_type(ret)
            )
        }
        _ => "serde_json::Value".into(),
    }
}
