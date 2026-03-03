use vox_hir::HirType;

use crate::emit::emit_type;
use crate::emit_expr::emit_stmt;

pub fn emit_trait(t: &vox_hir::HirTrait) -> String {
    let mut out = String::new();
    if t.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
    out.push_str(&format!("pub trait {} {{\n", t.name));
    for method in &t.methods {
        if method.is_deprecated {
            out.push_str("    #[deprecated]\n");
        }
        out.push_str(&format!("    fn {}(", method.name));
        out.push_str("&self, ");
        for param in &method.params {
            out.push_str(&format!(
                "{}: {}, ",
                param.name,
                emit_type(
                    param
                        .type_ann
                        .as_ref()
                        .unwrap_or(&HirType::Named("serde_json::Value".into()))
                )
            ));
        }
        out.push(')');
        if let Some(ret) = &method.return_type {
            out.push_str(&format!(" -> {}", emit_type(ret)));
        }
        out.push_str(";\n");
    }
    out.push_str("}\n\n");
    out
}

pub fn emit_impl(i: &vox_hir::HirImpl) -> String {
    let mut out = String::new();
    if i.is_deprecated {
        out.push_str("#[deprecated]\n");
    }
    if !i.trait_name.is_empty() {
        out.push_str(&format!(
            "impl {} for {} {{\n",
            i.trait_name,
            emit_type(&i.target_type)
        ));
    } else {
        out.push_str(&format!("impl {} {{\n", emit_type(&i.target_type)));
    }
    for method in &i.methods {
        if method.is_deprecated {
            out.push_str("    #[deprecated]\n");
        }
        let async_kw = if method.is_async { "async " } else { "" };
        let pub_kw = if method.is_pub { "pub " } else { "" };
        out.push_str(&format!("    {}{}fn {}(", pub_kw, async_kw, method.name));
        out.push_str("&self, ");
        for param in &method.params {
            out.push_str(&format!(
                "{}: {}, ",
                param.name,
                emit_type(
                    param
                        .type_ann
                        .as_ref()
                        .unwrap_or(&HirType::Named("serde_json::Value".into()))
                )
            ));
        }
        out.push(')');
        if let Some(ret) = &method.return_type {
            out.push_str(&format!(" -> {}", emit_type(ret)));
        }
        out.push_str(" {\n");
        for stmt in &method.body {
            out.push_str(&emit_stmt(stmt, 2, false, false));
        }
        out.push_str("    }\n");
    }
    out.push_str("}\n\n");
    out
}
