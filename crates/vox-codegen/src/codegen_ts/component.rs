//! Classic `@component fn` → React TSX type helpers.
//!
//! The classic AST → TSX emit path has been retired; live web/component codegen
//! flows through [`crate::codegen_ts::reactive`] (Path C, Web IR canonical
//! `view:` emit). What remains here are the two small TS-type helpers still
//! consumed by the live emitter:
//!
//! - `map_vox_type_to_ts` — Vox type expression → TypeScript type string.
//! - `ts_default_value` — placeholder JSX attribute value for a required prop
//!   in the auto-generated flat-app scaffold (keeps generated previews `tsc`-valid).

use vox_compiler::ast::scalar_mapping::VoxScalar;

/// A type-appropriate placeholder value, as a JSX attribute expression (e.g.
/// `{0}`), for a **required** prop in the auto-generated flat-app scaffold. The
/// scaffold flat-mounts the first component but cannot know real app data, so a
/// required prop with no Vox default would otherwise emit `<C />` and fail
/// `tsc` (TS2741). The value keeps the generated preview type-valid.
pub fn ts_default_value(ts_type: &str) -> &'static str {
    let t = ts_type.trim();
    if t == "number" {
        "{0}"
    } else if t == "string" {
        "{\"\"}"
    } else if t == "boolean" {
        "{false}"
    } else if t.ends_with("[]") {
        "{[]}"
    } else if t.contains("| undefined") {
        "{undefined}"
    } else {
        "{undefined as any}"
    }
}

/// Map a Vox type expression to a TypeScript type string.
pub fn map_vox_type_to_ts(ty: &vox_compiler::ast::types::TypeExpr) -> String {
    match ty {
        vox_compiler::ast::types::TypeExpr::Named { name, .. } => {
            if let Some(s) = VoxScalar::parse(name) {
                s.as_ts_primitive().to_string()
            } else {
                match name.as_str() {
                    "Element" => "React.ReactElement".to_string(),
                    "Unit" => "void".to_string(),
                    other => other.to_string(),
                }
            }
        }
        vox_compiler::ast::types::TypeExpr::Generic { name, args, .. } => {
            let args_str: Vec<String> = args.iter().map(map_vox_type_to_ts).collect();
            match name.as_str() {
                "list" => format!("{}[]", args_str.join(", ")),
                "Result" => format!("Result<{}>", args_str.join(", ")),
                "Option" => format!("{} | undefined", args_str.join(", ")),
                _ => format!("{}<{}>", name, args_str.join(", ")),
            }
        }
        vox_compiler::ast::types::TypeExpr::Function {
            params,
            return_type,
            ..
        } => {
            let params_str: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{i}: {}", map_vox_type_to_ts(p)))
                .collect();
            format!(
                "({}) => {}",
                params_str.join(", "),
                map_vox_type_to_ts(return_type)
            )
        }
        vox_compiler::ast::types::TypeExpr::Tuple { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(map_vox_type_to_ts).collect();
            format!("[{}]", elems.join(", "))
        }
        vox_compiler::ast::types::TypeExpr::Unit { .. } => "void".to_string(),
        vox_compiler::ast::types::TypeExpr::Infer { .. } => "any".to_string(),
        vox_compiler::ast::types::TypeExpr::Decimal { .. } => "string".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_default_value_covers_primitive_array_and_fallback() {
        assert_eq!(ts_default_value("number"), "{0}");
        assert_eq!(ts_default_value("string"), "{\"\"}");
        assert_eq!(ts_default_value("boolean"), "{false}");
        assert_eq!(ts_default_value("Foo[]"), "{[]}");
        assert_eq!(ts_default_value("string | undefined"), "{undefined}");
        assert_eq!(ts_default_value("SomeIface"), "{undefined as any}");
        // Leading/trailing whitespace is trimmed before matching.
        assert_eq!(ts_default_value("  number  "), "{0}");
    }
}
