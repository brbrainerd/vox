use vox_compiler::hir::{HirModule, HirType, HirUrlDecl, HirUrlVariant};

/// Emit TypeScript for all `url` declarations in the module.
pub fn emit_url_decls(hir: &HirModule) -> String {
    if hir.url_decls.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for u in &hir.url_decls {
        out.push_str(&emit_url_decl(u));
        out.push('\n');
    }
    out
}

/// Emit a discriminated union + builder functions for one `url` block.
///
/// Example input: `url Path { Home; Task(id: string) }`
///
/// Emits:
/// ```ts
/// export type Path =
///   | { readonly _tag: "Home" }
///   | { readonly _tag: "Task"; readonly id: string };
///
/// export const Path = {
///   Home: (): Path => ({ _tag: "Home" }),
///   Task: (id: string): Path => ({ _tag: "Task", id }),
/// } as const;
/// ```
fn emit_url_decl(u: &HirUrlDecl) -> String {
    let name = &u.name;
    let pub_prefix = if u.is_pub { "export " } else { "" };

    // Union type
    let mut out = String::new();
    out.push_str(&format!("{pub_prefix}type {name} =\n"));
    for (i, variant) in u.variants.iter().enumerate() {
        let sep = if i < u.variants.len() - 1 { "" } else { ";" };
        if variant.args.is_empty() {
            out.push_str(&format!(
                "  | {{ readonly _tag: \"{}\" }}{sep}\n",
                variant.name
            ));
        } else {
            let fields: String = variant
                .args
                .iter()
                .map(|a| {
                    let opt = if a.optional { "?" } else { "" };
                    format!("readonly {}{opt}: {}", a.name, hir_type_to_ts(&a.ty))
                })
                .collect::<Vec<_>>()
                .join("; ");
            out.push_str(&format!(
                "  | {{ readonly _tag: \"{}\"; {fields} }}{sep}\n",
                variant.name
            ));
        }
    }
    out.push('\n');

    // Builder object
    out.push_str(&format!("{pub_prefix}const {name} = {{\n"));
    for variant in &u.variants {
        out.push_str(&emit_builder(name, variant));
    }
    out.push_str("} as const;\n");
    out
}

fn emit_builder(type_name: &str, v: &HirUrlVariant) -> String {
    let vname = &v.name;
    if v.args.is_empty() {
        format!("  {vname}: (): {type_name} => ({{ _tag: \"{vname}\" }}),\n")
    } else {
        let params: String = v
            .args
            .iter()
            .map(|a| {
                let opt = if a.optional { "?" } else { "" };
                format!("{}{opt}: {}", a.name, hir_type_to_ts(&a.ty))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let fields: String = v
            .args
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("  {vname}: ({params}): {type_name} => ({{ _tag: \"{vname}\", {fields} }}),\n")
    }
}

pub fn hir_type_to_ts(ty: &HirType) -> String {
    match ty {
        HirType::Named(n) => match n.as_str() {
            "str" | "String" => "string".to_string(),
            "int" | "i32" | "i64" | "u32" | "u64" | "usize" => "number".to_string(),
            "float" | "f64" | "f32" => "number".to_string(),
            "bool" => "boolean".to_string(),
            other => other.to_string(),
        },
        HirType::Generic(name, args) => match name.as_str() {
            "option" | "Option" if args.len() == 1 => {
                format!("{} | undefined", hir_type_to_ts(&args[0]))
            }
            "list" | "List" | "Vec" if args.len() == 1 => {
                format!("ReadonlyArray<{}>", hir_type_to_ts(&args[0]))
            }
            _ => {
                let args_ts: String = args
                    .iter()
                    .map(hir_type_to_ts)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}<{args_ts}>")
            }
        },
        HirType::Tuple(elems) => {
            let ts: String = elems
                .iter()
                .map(hir_type_to_ts)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{ts}]")
        }
        HirType::Unit => "void".to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::hir::lower::lower_module;
    use vox_compiler::lexer::cursor::lex;
    use vox_compiler::parser::parse;

    fn emit_from_src(src: &str) -> String {
        let tokens = lex(src);
        let module = parse(tokens).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
        let hir = lower_module(&module);
        emit_url_decls(&hir)
    }

    #[test]
    fn url_emit_simple_variant() {
        let out = emit_from_src("url Path {\nHome\n}");
        assert!(out.contains("type Path ="), "expected union type: {out}");
        assert!(
            out.contains("_tag: \"Home\""),
            "expected Home variant: {out}"
        );
        assert!(out.contains("Home: ():"), "expected Home builder: {out}");
    }

    #[test]
    fn url_emit_parameterized_variant() {
        let out = emit_from_src("url Path {\nTask(id: str)\n}");
        assert!(
            out.contains("readonly id: string"),
            "expected id field: {out}"
        );
        assert!(
            out.contains("Task: (id: string):"),
            "expected Task builder: {out}"
        );
    }

    #[test]
    fn url_emit_optional_arg() {
        let out = emit_from_src("url Path {\nLogin(?return_to: str)\n}");
        assert!(
            out.contains("return_to?: string"),
            "expected optional field: {out}"
        );
        assert!(
            out.contains("return_to?: string"),
            "expected optional param: {out}"
        );
    }

    #[test]
    fn url_emit_empty_when_no_url_decls() {
        let out = emit_from_src("fn foo() { }");
        assert!(out.is_empty(), "expected empty output for non-url source");
    }
}
