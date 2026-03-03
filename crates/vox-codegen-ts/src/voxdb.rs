use vox_ast::decl::Module;

/// Generate typed args from function parameters.
fn emit_voxdb_args(params: &[vox_ast::expr::Param]) -> String {
    if params.is_empty() {
        return "{}".to_string();
    }
    let mut out = String::from("{\n");
    for p in params {
        let validator = p.type_ann.as_ref().map_or("v.any()".to_string(), |ty| {
            crate::schema::type_to_voxdb_validator(ty)
        });
        out.push_str(&format!("    {}: {},\n", p.name, validator));
    }
    out.push_str("  }");
    out
}

pub fn generate_voxdb_handlers(module: &Module) -> String {
    let mut out = String::new();
    let mut queries = Vec::new();
    let mut mutations = Vec::new();
    let mut actions = Vec::new();

    for decl in &module.declarations {
        match decl {
            vox_ast::decl::Decl::Query(q) => queries.push(q),
            vox_ast::decl::Decl::Mutation(m) => mutations.push(m),
            vox_ast::decl::Decl::Action(a) => actions.push(a),
            _ => {}
        }
    }

    if queries.is_empty() && mutations.is_empty() && actions.is_empty() {
        return "".to_string();
    }

    out.push_str("import { query, mutation, action } from \"voxdb/server\";\n");
    out.push_str("import { v } from \"voxdb/values\";\n\n");

    for q in queries {
        let name = &q.func.name;
        let args = emit_voxdb_args(&q.func.params);
        out.push_str(&format!("export const {} = query({{\n", name));
        out.push_str(&format!("  args: {},\n", args));
        out.push_str("  handler: async (ctx, args) => {\n");
        out.push_str("    const db = ctx.db;\n");
        if !q.func.params.is_empty() {
            let destructured: Vec<&str> = q.func.params.iter().map(|p| p.name.as_str()).collect();
            out.push_str(&format!(
                "    const {{ {} }} = args;\n",
                destructured.join(", ")
            ));
        }
        for stmt in &q.func.body {
            out.push_str(&format!("    {};\n", crate::jsx::emit_stmt(stmt, 2)));
        }
        out.push_str("  }\n});\n\n");
    }

    for m in mutations {
        let name = &m.func.name;
        let args = emit_voxdb_args(&m.func.params);
        out.push_str(&format!("export const {} = mutation({{\n", name));
        out.push_str(&format!("  args: {},\n", args));
        out.push_str("  handler: async (ctx, args) => {\n");
        out.push_str("    const db = ctx.db;\n");
        if !m.func.params.is_empty() {
            let destructured: Vec<&str> = m.func.params.iter().map(|p| p.name.as_str()).collect();
            out.push_str(&format!(
                "    const {{ {} }} = args;\n",
                destructured.join(", ")
            ));
        }
        for stmt in &m.func.body {
            out.push_str(&format!("    {};\n", crate::jsx::emit_stmt(stmt, 2)));
        }
        out.push_str("  }\n});\n\n");
    }

    for a in actions {
        let name = &a.func.name;
        let args = emit_voxdb_args(&a.func.params);
        out.push_str(&format!("export const {} = action({{\n", name));
        out.push_str(&format!("  args: {},\n", args));
        out.push_str("  handler: async (ctx, args) => {\n");
        out.push_str("    const db = ctx.db;\n");
        if !a.func.params.is_empty() {
            let destructured: Vec<&str> = a.func.params.iter().map(|p| p.name.as_str()).collect();
            out.push_str(&format!(
                "    const {{ {} }} = args;\n",
                destructured.join(", ")
            ));
        }
        for stmt in &a.func.body {
            out.push_str(&format!("    {};\n", crate::jsx::emit_stmt(stmt, 2)));
        }
        out.push_str("  }\n});\n\n");
    }

    out
}
