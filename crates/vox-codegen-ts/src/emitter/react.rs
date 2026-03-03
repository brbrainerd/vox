use vox_ast::decl::{ContextDecl, ProviderDecl, HookDecl};

pub fn generate_contexts(contexts: &[&ContextDecl]) -> String {
    let mut content = String::new();
    content.push_str("import React from \"react\";\n\n");
    for ctx in contexts {
        let state_type = ctx
            .state_type
            .as_ref()
            .map_or("any".to_string(), crate::component::map_vox_type_to_ts);
        content.push_str(&format!(
            "export const {}Context = React.createContext<{} | undefined>(undefined);\n\n",
            ctx.name, state_type
        ));
        content.push_str(&format!(
            "export function use{}(): {} {{\n  const ctx = React.useContext({}Context);\n  if (ctx === undefined) throw new Error(\"{}Context must be used within a provider\");\n  return ctx;\n}}\n\n",
            ctx.name, state_type, ctx.name, ctx.name
        ));
    }
    content
}

pub fn generate_provider(prov: &ProviderDecl) -> String {
    let mut content = String::new();
    let params: Vec<String> = prov
        .func
        .params
        .iter()
        .map(|p| {
            let ty = p
                .type_ann
                .as_ref()
                .map_or("any".to_string(), crate::component::map_vox_type_to_ts);
            format!("{}: {}", p.name, ty)
        })
        .collect();
    let mut props_with_children = params;
    props_with_children.push("children: React.ReactNode".to_string());
    content.push_str(&format!(
        "export function {}Provider({{ {} }}): React.ReactElement {{\n",
        prov.func.name,
        props_with_children.join(", ")
    ));
    for stmt in &prov.func.body {
        content.push_str(&format!("  {};\n", crate::jsx::emit_stmt(stmt, 1)));
    }
    content.push_str(&format!(
        "  return (\n    <{}Context.Provider value={{undefined as any}}>\n      {{children}}\n    </{}Context.Provider>\n  );\n}}\n\n",
        prov.context_name, prov.context_name
    ));
    content
}

pub fn generate_custom_hook(hook: &HookDecl) -> String {
    let mut content = String::new();
    let func = &hook.func;
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| {
            let ty = p
                .type_ann
                .as_ref()
                .map_or("any".to_string(), crate::component::map_vox_type_to_ts);
            format!("{}: {}", p.name, ty)
        })
        .collect();
    let ret_type = func
        .return_type
        .as_ref()
        .map_or("void".to_string(), crate::component::map_vox_type_to_ts);

    let hook_name = func.name.replace("use_", "use");
    let hook_name = {
        let mut chars = hook_name.chars().collect::<Vec<_>>();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '_' && i + 1 < chars.len() {
                chars.remove(i);
                if i < chars.len() {
                    chars[i] = chars[i].to_ascii_uppercase();
                }
            } else {
                i += 1;
            }
        }
        chars.into_iter().collect::<String>()
    };

    content.push_str(&format!(
        "export function {}({}): {} {{\n",
        hook_name,
        params.join(", "),
        ret_type
    ));
    for stmt in &func.body {
        content.push_str(&format!("  {};\n", crate::jsx::emit_stmt(stmt, 1)));
    }
    content.push_str("}\n\n");
    content
}
