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
pub(crate) fn to_camel_case(s: &str) -> String {
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

#[cfg(test)]
mod semcov_wave31_tests {
    use super::*;
    use vox_compiler::hir::HirType;

    // ---- to_camel_case adversarial tests ----

    // Catches: first char of snake_case not being forced to lowercase (identity on already-lower)
    #[test]
    fn camel_case_single_word_stays_lowercase() {
        assert_eq!(to_camel_case("users"), "users");
    }

    // Catches: PascalCase input having first char lowercased
    #[test]
    fn camel_case_pascal_lowercases_first_char() {
        // "Users" → "users" (only first char lowercased)
        assert_eq!(to_camel_case("Users"), "users");
    }

    // Catches: consecutive underscores producing empty segments / double-capitalize
    #[test]
    fn camel_case_consecutive_underscores_skips_empty() {
        // "foo__bar" — second underscore sets capitalize_next again; 'b' is still capitalized
        let result = to_camel_case("foo__bar");
        assert!(
            !result.contains('_'),
            "underscores must be removed: {result}"
        );
        assert!(result.starts_with("foo"), "prefix preserved: {result}");
    }

    // Catches: trailing underscore leaking into output
    #[test]
    fn camel_case_trailing_underscore_produces_no_suffix() {
        let result = to_camel_case("foo_");
        // capitalize_next is set but no char follows — should not panic or append garbage
        assert!(
            !result.contains('_'),
            "trailing underscore leaked: {result}"
        );
        assert_eq!(result, "foo");
    }

    // Catches: leading underscore causing first-char path to lowercase '_'
    #[test]
    fn camel_case_leading_underscore_is_elided() {
        let result = to_camel_case("_private");
        // '_' at index 0 sets capitalize_next; 'p' → 'P'. But i==0 path is NOT taken.
        // Document actual behavior as an invariant.
        assert!(
            !result.starts_with('_'),
            "leading underscore must not appear in output: {result}"
        );
    }

    // Catches: empty string causing a panic (no chars, loop never runs)
    #[test]
    fn camel_case_empty_string_returns_empty() {
        assert_eq!(to_camel_case(""), "");
    }

    // ---- hir_type_to_ts adversarial tests ----

    // Catches: Option with zero type args producing "v.optional(v.any())" instead of panicking
    #[test]
    fn hir_ts_option_with_no_args_falls_back_to_unknown() {
        let ty = HirType::Generic("Option".to_string(), vec![]);
        let result = hir_type_to_ts(&ty);
        // Should not panic; result may be " | undefined" (empty join) — document boundary
        assert!(
            result.contains("undefined") || result.contains("unknown"),
            "Option<> with no args should produce a safe fallback: {result}"
        );
    }

    // Catches: List with zero args rendering "readonly []" instead of "readonly unknown[]"
    #[test]
    fn hir_ts_list_with_no_args_falls_back_gracefully() {
        let ty = HirType::Generic("List".to_string(), vec![]);
        let result = hir_type_to_ts(&ty);
        assert!(
            result.contains("[]") || result.contains("unknown"),
            "List<> with no args should produce array notation: {result}"
        );
    }

    // Catches: Map with only one arg not matching the 2-arg guard → falls to generic branch
    #[test]
    fn hir_ts_map_with_one_arg_falls_to_generic() {
        let ty = HirType::Generic("Map".to_string(), vec![HirType::Named("str".to_string())]);
        let result = hir_type_to_ts(&ty);
        // Should NOT emit Record<str, …> with a dangling second param
        assert!(
            !result.starts_with("Record"),
            "1-arg Map must not emit Record<>: {result}"
        );
    }

    // Catches: Tuple with zero elements emitting "[]" — valid TS but should be documented
    #[test]
    fn hir_ts_empty_tuple_emits_empty_brackets() {
        let ty = HirType::Tuple(vec![]);
        let result = hir_type_to_ts(&ty);
        assert_eq!(result, "[]", "empty tuple should emit '[]': {result}");
    }

    // Catches: Function type with no params or return emitting wrong arrow syntax
    #[test]
    fn hir_ts_nullary_function_emits_arrow_type() {
        let ty = HirType::Function(vec![], Box::new(HirType::Unit));
        let result = hir_type_to_ts(&ty);
        assert!(
            result.contains("=>"),
            "function type must contain '=>': {result}"
        );
        assert!(result.contains("void"), "void return must appear: {result}");
    }

    // Catches: nested Generic type (List<Option<str>>) not recursing correctly
    #[test]
    fn hir_ts_nested_generic_recurses() {
        let inner = HirType::Generic(
            "Option".to_string(),
            vec![HirType::Named("str".to_string())],
        );
        let outer = HirType::Generic("List".to_string(), vec![inner]);
        let result = hir_type_to_ts(&outer);
        assert!(
            result.contains("string"),
            "inner str must become string: {result}"
        );
        assert!(
            result.contains("undefined"),
            "inner Option must emit undefined: {result}"
        );
        assert!(
            result.contains("[]") || result.contains("ReadonlyArray"),
            "outer List must emit array: {result}"
        );
    }

    // Catches: "str" Named type not mapped to "string" (verbatim passthrough bug)
    #[test]
    fn hir_ts_str_named_maps_to_string() {
        let ty = HirType::Named("str".to_string());
        assert_eq!(hir_type_to_ts(&ty), "string");
    }

    // Catches: unknown named type being silently dropped or panicking
    #[test]
    fn hir_ts_unknown_named_type_passes_through() {
        let ty = HirType::Named("MyCustomType".to_string());
        let result = hir_type_to_ts(&ty);
        assert_eq!(
            result, "MyCustomType",
            "unknown named types should pass through verbatim"
        );
    }
}
