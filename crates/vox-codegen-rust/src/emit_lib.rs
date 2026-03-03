use vox_hir::HirModule;

use crate::emit::{emit_fn, emit_type};
use crate::emit_agent::{emit_activity, emit_actor, emit_workflow};
use crate::emit_table::{emit_collection_struct, emit_table_struct};

pub fn emit_lib(module: &HirModule) -> String {
    let mut out = String::new();
    out.push_str("use serde::{Serialize, Deserialize};\n");

    // Only import runtime types when actors are present
    if !module.actors.is_empty() {
        out.push_str(
            "use vox_runtime::{ProcessContext, Envelope, MessagePayload, Pid, Message};\n",
        );
    }

    // Cross-module imports
    for (idx, (target_mod, _)) in &module.resolved_imports {
        let item = &module.imports[*idx].item;
        out.push_str(&format!("use crate::{}::{};\n", target_mod, item));
    }

    // Emit Python bridge imports and lazy runtime when @py.import declarations are present.
    if !module.py_imports.is_empty() {
        out.push_str("use vox_py::VoxPyRuntime;\n");
        out.push_str("use once_cell::sync::Lazy;\n\n");
        // VoxPyRuntime::new() automatically detects the uv-managed venv:
        //   - Check VOX_VENV_PATH env var (Docker / CI override)
        //   - Check UV_PROJECT_ENVIRONMENT / VIRTUAL_ENV env vars
        //   - Fall back to .venv/ in the binary's directory, then cwd
        // Set VOX_VENV_PATH=<venv_root> for non-standard environments.
        out.push_str("static PY_RT: Lazy<VoxPyRuntime> = Lazy::new(|| {\n");
        out.push_str("    // Temporarily enter the binary's directory so .venv is resolved relative\n");
        out.push_str("    // to the project root, regardless of the shell's working directory.\n");
        out.push_str("    let bin_dir = std::env::current_exe()\n");
        out.push_str("        .ok()\n");
        out.push_str("        .and_then(|p| p.parent().map(|d| d.to_path_buf()));\n");
        out.push_str("    let saved_cwd = std::env::current_dir().ok();\n");
        out.push_str("    if let Some(ref dir) = bin_dir {\n");
        out.push_str("        let _ = std::env::set_current_dir(dir);\n");
        out.push_str("    }\n");
        out.push_str("    let rt = VoxPyRuntime::new();\n");
        out.push_str("    if let Some(cwd) = saved_cwd {\n");
        out.push_str("        let _ = std::env::set_current_dir(cwd);\n");
        out.push_str("    }\n");
        for py_import in &module.py_imports {
            out.push_str(&format!(
                "    rt.import_module(\"{}\", \"{}\");\n",
                py_import.module, py_import.alias
            ));
        }
        out.push_str("    rt\n");
        out.push_str("});\n");
    }

    // Turso is used by table impls (no explicit import needed here)

    // Emit vox-tensor crate imports when `import tensor` / `import nn` / `import optim` are present.
    if !module.tensor_imports.is_empty() {
        out.push_str("use vox_tensor::tensor::Tensor;\n");
        out.push_str("use vox_tensor::nn::Module as NnModule;\n");
        out.push_str("use vox_tensor::nn::Sequential;\n\n");

        for ti in &module.tensor_imports {
            match ti.module.as_str() {
                "tensor" => {
                    // Expose the Tensor construction helpers under the alias namespace.
                    // e.g. `import tensor as t` → `let t_zeros = Tensor::zeros_1d;`
                    out.push_str(&format!(
                        "// Tensor module bound as `{}`\n",
                        ti.alias
                    ));
                    out.push_str(&format!(
                        "mod {} {{\n    pub use vox_tensor::tensor::Tensor;\n}}\n",
                        ti.alias
                    ));
                }
                "nn" => {
                    out.push_str(&format!(
                        "// nn module bound as `{}`\n",
                        ti.alias
                    ));
                    out.push_str(&format!(
                        "mod {} {{\n    pub use vox_tensor::nn::Module;\n    pub use vox_tensor::nn::Sequential;\n}}\n",
                        ti.alias
                    ));
                }
                "optim" => {
                    out.push_str(&format!(
                        "// optim module bound as `{}`\n",
                        ti.alias
                    ));
                    // Placeholder — optimizer integration comes after autodiff wiring.
                    out.push_str(&format!(
                        "mod {} {{\n    // optimizer integration — SGD/Adam to be wired via burn::optim\n}}\n",
                        ti.alias
                    ));
                }
                _ => {}
            }
        }
        out.push('\n');
    }

    out.push('\n');

    // Helper for casts
    out.push_str("pub fn as_string<T: serde::Serialize>(v: &T) -> String {\n");
    out.push_str(
        "    let val = serde_json::to_value(v).unwrap_or_else(|_| serde_json::Value::Null);\n",
    );
    out.push_str("    if let Some(s) = val.as_str() { s.to_string() } else { val.to_string() }\n");
    out.push_str("}\n\n");

    // Only emit append helper when actors are present
    if !module.actors.is_empty() {
        out.push_str("pub fn append(list: &Vec<serde_json::Value>, item: &serde_json::Value) -> Vec<serde_json::Value> {\n");
        out.push_str("    let mut new_list = list.clone();\n");
        out.push_str("    new_list.push(item.clone());\n");
        out.push_str("    new_list\n");
        out.push_str("}\n\n");
    }

    // Re-export ADT variants (struct types don't need this)
    for typedef in &module.types {
        if typedef.fields.is_empty() && !typedef.variants.is_empty() {
            out.push_str(&format!("pub use self::{}::*;\n", typedef.name));
        }
    }

    // Types
    for typedef in &module.types {
        if typedef.is_deprecated {
            out.push_str("#[deprecated]\n");
        }

        let pub_kw = if typedef.is_pub { "pub " } else { "" };

        if let Some(ref alias_ty) = typedef.type_alias {
            out.push_str(&format!(
                "{}type {} = {};\n\n",
                pub_kw,
                typedef.name,
                emit_type(alias_ty)
            ));
            continue;
        }

        let mut has_forward = false;
        for impl_decl in &module.impls {
            if let vox_hir::HirType::Named(ref n) = impl_decl.target_type {
                if n == &typedef.name && impl_decl.methods.iter().any(|m| m.name == "forward") {
                    has_forward = true;
                    break;
                }
            }
        }

        if has_forward {
            out.push_str("#[derive(Debug, Clone, vox_tensor::burn::module::Module)]");
        } else {
            out.push_str("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]");
        }

        if !typedef.fields.is_empty() {
            // Struct type (product type)
            out.push('\n');
            out.push_str(&format!("{}struct {} {{\n", pub_kw, typedef.name));
            for (fname, ftype) in &typedef.fields {
                out.push_str(&format!("    pub {}: {},\n", fname, emit_type(ftype)));
            }
            out.push_str("}\n\n");
        } else {
            let is_enum_literal = !typedef.variants.is_empty()
                && typedef.variants.iter().all(|v| v.fields.is_empty());
            if is_enum_literal {
                // Pure enum-literal ADT: `type Role = | User | Assistant`
                // Serializes to plain strings ("User", "Assistant"), matching TS literal union.
                out.push_str("\n#[serde(rename_all = \"PascalCase\")]\n");
                out.push_str(&format!("{}enum {} {{\n", pub_kw, typedef.name));
                for variant in &typedef.variants {
                    out.push_str(&format!("    {},\n", variant.name));
                }
                out.push_str("}\n\n");
            } else {
                // ADT (sum type) — use internally-tagged serde representation to match TS `_tag`
                out.push_str("\n#[serde(tag = \"_tag\")]\n");
                out.push_str(&format!("{}enum {} {{\n", pub_kw, typedef.name));
                for variant in &typedef.variants {
                    if variant.fields.is_empty() {
                        out.push_str(&format!("    {},\n", variant.name));
                    } else {
                        // Named fields struct variant for serde compatibility
                        out.push_str(&format!("    {} {{\n", variant.name));
                        for (fname, ftype) in &variant.fields {
                            out.push_str(&format!("        {}: {},\n", fname, emit_type(ftype)));
                        }
                        out.push_str("    },\n");
                    }
                }
                out.push_str("}\n\n");
            }
        }
    }

    // Traits
    for trait_decl in &module.traits {
        out.push_str(&crate::emit_trait::emit_trait(trait_decl));
    }

    // Impls
    for impl_decl in &module.impls {
        out.push_str(&crate::emit_trait::emit_impl(impl_decl));
    }

    // Config blocks
    for cfg in &module.config_blocks {
        out.push_str(&format!(
            "#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]\n"
        ));
        out.push_str(&format!("pub struct {} {{\n", cfg.name));
        for field in &cfg.fields {
            let ty_str = emit_type(&field.type_ann);
            out.push_str(&format!("    pub {}: {},\n", field.name, ty_str));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("impl {} {{\n", cfg.name));
        out.push_str("    pub fn load_from_env() -> Result<Self, String> {\n");
        out.push_str("        Ok(Self {\n");
        for field in &cfg.fields {
            let is_opt =
                matches!(&field.type_ann, vox_hir::HirType::Generic(name, _) if name == "Option");
            if is_opt {
                out.push_str(&format!(
                    "            {}: std::env::var(\"{}\").ok(),\n",
                    field.name, field.name
                ));
            } else {
                out.push_str(&format!(
                    "            {}: std::env::var(\"{}\").map_err(|_| format!(\"Missing required environment variable: {}\"))?,\n",
                    field.name, field.name, field.name
                ));
            }
        }
        out.push_str("        })\n");
        out.push_str("    }\n");
        out.push_str("}\n\n");
    }

    // Table structs
    for table in &module.tables {
        out.push_str(&emit_table_struct(table));
    }

    // Collection structs
    for coll in &module.collections {
        out.push_str(&emit_collection_struct(coll));
    }

    // Functions (skip components)
    let has_model_metric = module.types.iter().any(|t| t.name == "ModelMetric");

    for func in &module.functions {
        if !func.is_component {
            out.push_str(&emit_fn(func, has_model_metric));
        }
    }

    for query in &module.queries {
        out.push_str(&emit_fn(&query.func, has_model_metric));
    }
    for mutation in &module.mutations {
        out.push_str(&emit_fn(&mutation.func, has_model_metric));
    }
    for action in &module.actions {
        out.push_str(&emit_fn(&action.func, has_model_metric));
    }
    for s in &module.scheduled {
        out.push_str(&emit_fn(&s.func, has_model_metric));
    }
    for sf in &module.server_fns {
        // Construct a temporary HirFn to use emit_fn
        let f = vox_hir::HirFn {
            id: sf.id,
            name: sf.name.clone(),
            generics: vec![],
            params: sf.params.clone(),
            return_type: sf.return_type.clone(),
            body: sf.body.clone(),
            is_component: false,
            is_async: true,
            is_deprecated: sf.is_deprecated,
            is_pure: false,
            is_traced: true,
            is_llm: false,
            llm_model: None,
            is_pub: true,
            is_metric: false,
            metric_name: None,
            is_health: false,
            is_layout: false,
            preconditions: vec![],
            span: sf.span,
        };
        out.push_str(&emit_fn(&f, has_model_metric));
    }

    // Workflows
    for workflow in &module.workflows {
        out.push_str(&emit_workflow(workflow));
    }

    // Activities
    for activity in &module.activities {
        out.push_str(&emit_activity(activity));
    }

    // Actors
    for actor in &module.actors {
        out.push_str(&emit_actor(actor));
    }

    // Native agents → Rust structs with handler methods
    for agent in &module.native_agents {
        if agent.is_deprecated {
            out.push_str("#[deprecated]\n");
        }
        out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
        out.push_str(&format!("pub struct {} {{\n", agent.name));
        for (field_name, field_type) in &agent.state_fields {
            out.push_str(&format!(
                "    pub {}: {},\n",
                field_name,
                emit_type(field_type)
            ));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("impl {} {{\n", agent.name));
        for handler in &agent.handlers {
            let params_str: Vec<String> = handler
                .params
                .iter()
                .map(|p| {
                    let ty_str = match &p.type_ann {
                        Some(t) => emit_type(t),
                        None => "String".to_string(),
                    };
                    format!("{}: {}", p.name, ty_str)
                })
                .collect();
            let ret = match &handler.return_type {
                Some(t) => format!(" -> {}", emit_type(t)),
                None => String::new(),
            };
            out.push_str(&format!(
                "    pub async fn {}(&mut self, {}){} {{\n",
                handler.event_name,
                params_str.join(", "),
                ret,
            ));
            for stmt in &handler.body {
                out.push_str(&crate::emit_expr::emit_stmt(stmt, 2, false, false));
            }
            out.push_str("    }\n");
        }
        out.push_str("}\n\n");
    }

    // Messages → Rust structs
    for msg in &module.messages {
        if msg.is_deprecated {
            out.push_str("#[deprecated]\n");
        }
        out.push_str("#[derive(Debug, Clone, Serialize, Deserialize)]\n");
        out.push_str(&format!("pub struct {} {{\n", msg.name));
        for (field_name, field_type) in &msg.fields {
            out.push_str(&format!(
                "    pub {}: {},\n",
                field_name,
                emit_type(field_type)
            ));
        }
        out.push_str("}\n\n");
    }

    // Fixtures
    if !module.fixtures.is_empty() {
        out.push_str("#[cfg(test)]\n");
        out.push_str("pub mod fixtures {\n");
        out.push_str("    use super::*;\n");
        for fixture in &module.fixtures {
            let mut f_out = crate::emit::emit_fn(fixture, false);
            // Replace `pub fn` or `fn` with `pub fn` to ensure visibility.
            if f_out.starts_with("fn ") {
                f_out = f_out.replacen("fn ", "pub fn ", 1);
            } else if f_out.starts_with("async fn ") {
                f_out = f_out.replacen("async fn ", "pub async fn ", 1);
            }
            out.push_str(&f_out);
        }
        out.push_str("}\n\n");
    }

    // Mocks
    if !module.mocks.is_empty() {
        out.push_str("#[cfg(test)]\n");
        out.push_str("pub mod mocks {\n");
        out.push_str("    use super::*;\n");
        for mock in &module.mocks {
            let mut f_out = crate::emit::emit_fn(&mock.func, false);
            // Ensure visibility
            if f_out.trim_start().starts_with("fn ") {
                f_out = f_out.replacen("fn ", "pub fn ", 1);
            } else if f_out.trim_start().starts_with("async fn ") {
                f_out = f_out.replacen("async fn ", "pub async fn ", 1);
            }
            out.push_str(&f_out);
        }
        out.push_str("}\n\n");
    }

    // Tests
    for test in &module.tests {
        let is_any_async = test.is_async || module.fixtures.iter().any(|f| f.is_async);
        if is_any_async {
            out.push_str("#[tokio::test]\n");
            out.push_str(&format!("pub async fn {}() {{\n", test.name));
        } else {
            out.push_str("#[test]\n");
            out.push_str(&format!("pub fn {}() {{\n", test.name));
        }
        for fixture in &module.fixtures {
            if fixture.is_async {
                out.push_str(&format!("    fixtures::{}().await;\n", fixture.name));
            } else {
                out.push_str(&format!("    fixtures::{}();\n", fixture.name));
            }
        }
        for stmt in &test.body {
            out.push_str(&crate::emit_expr::emit_stmt(stmt, 1, false, false));
        }
        out.push_str("}\n\n");
    }

    out
}
