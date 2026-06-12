use vox_compiler::ast::types::TypeExpr;
use vox_compiler::hir::HirType;

pub(super) fn hir_type_to_voxdb_validator(ty: &HirType) -> String {
    match ty {
        HirType::Named(name) => match name.as_str() {
            "str" => "v.string()".to_string(),
            "int" | "float" | "float64" => "v.number()".to_string(),
            "bool" => "v.boolean()".to_string(),
            "bytes" | "Bytes" => "v.bytes()".to_string(),
            other => format!("v.any() /* {} */", other),
        },
        HirType::Generic(name, args) => match name.as_str() {
            "Option" => {
                let inner = args
                    .first()
                    .map(hir_type_to_voxdb_validator)
                    .unwrap_or_else(|| "v.any()".to_string());
                format!("v.optional({})", inner)
            }
            "List" | "list" => {
                let inner = args
                    .first()
                    .map(hir_type_to_voxdb_validator)
                    .unwrap_or_else(|| "v.any()".to_string());
                format!("v.array({})", inner)
            }
            "Id" => {
                let table = args
                    .first()
                    .and_then(|a| {
                        if let HirType::Named(n) = a {
                            Some(n.as_str())
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
        HirType::Tuple(elements) => {
            let els: Vec<String> = elements.iter().map(hir_type_to_voxdb_validator).collect();
            format!("v.array(v.union({}))", els.join(", "))
        }
        HirType::Function(..) => "v.any() /* Function */".to_string(),
        HirType::Unit => "v.null()".to_string(),
        HirType::Decimal => "v.string()".to_string(),
    }
}

pub(super) fn hir_type_to_ts(ty: &HirType) -> String {
    match ty {
        HirType::Named(name) => match name.as_str() {
            "str" => "string".to_string(),
            "int" | "float" | "float64" => "number".to_string(),
            "bool" => "boolean".to_string(),
            "bytes" | "Bytes" => "ArrayBuffer".to_string(),
            "Unit" => "void".to_string(),
            "Id" => "string".to_string(),
            other => other.to_string(),
        },
        HirType::Generic(name, args) => {
            let args_str: Vec<String> = args.iter().map(hir_type_to_ts).collect();
            match name.as_str() {
                "Option" => format!("{} | undefined", args_str.join(", ")),
                "List" | "list" => format!(
                    "readonly {}[]",
                    args_str.first().map(String::as_str).unwrap_or("unknown")
                ),
                "Map" | "map" if args_str.len() == 2 => {
                    format!("Record<{}, {}>", args_str[0], args_str[1])
                }
                "Set" | "set" if !args_str.is_empty() => format!("Set<{}>", args_str[0]),
                "Result" => format!("Result<{}>", args_str.join(", ")),
                "Id" => "string".to_string(),
                _ => format!("{}<{}>", name, args_str.join(", ")),
            }
        }
        HirType::Function(params, return_type) => {
            let params_str: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{i}: {}", hir_type_to_ts(p)))
                .collect();
            format!(
                "({}) => {}",
                params_str.join(", "),
                hir_type_to_ts(return_type)
            )
        }
        HirType::Tuple(elements) => {
            let elems: Vec<String> = elements.iter().map(hir_type_to_ts).collect();
            format!("[{}]", elems.join(", "))
        }
        HirType::Unit => "void".to_string(),
        HirType::Decimal => "string".to_string(),
    }
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
        TypeExpr::Tuple { elements, .. } => {
            let els: Vec<String> = elements.iter().map(type_to_voxdb_validator).collect();
            format!("v.array(v.union({}))", els.join(", "))
        }
        TypeExpr::Function { .. } => "v.any() /* Function */".to_string(),
        TypeExpr::Unit { .. } => "v.null()".to_string(),
        TypeExpr::Infer { .. } => "v.any()".to_string(),
        TypeExpr::Decimal { .. } => "v.string()".to_string(),
    }
}

/// Map a Vox TypeExpr to a TypeScript type string (for the interface declarations).
pub(super) fn type_to_ts(ty: &TypeExpr) -> String {
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
                "Option" => format!("{} | undefined", args_str.join(", ")),
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
        TypeExpr::Infer { .. } => "any".to_string(),
        TypeExpr::Decimal { .. } => "string".to_string(),
    }
}

/// Convert a PascalCase or snake_case name to camelCase for VoxDB table keys.
pub(super) fn to_camel_case(s: &str) -> String {
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
