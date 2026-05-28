//! `generate` / script-mode entrypoints (OP-0209).

use std::collections::HashMap;
use std::path::Path;

use vox_compiler::hir::HirModule;
use vox_compiler::rust_interop_support::{
    classify_rust_crate, is_template_managed_script_native_dependency,
    is_template_managed_script_wasi_dependency, is_wasi_unsupported_rust_import,
};

use super::GENERATED_CARGO_EDITION;
use super::emit;
use super::manifest::{CodegenOutput, manifest_dependency_path};

/// Generate a full Rust project from a HIR module.
pub fn generate(
    module: &HirModule,
    package_name: &str,
    shell: super::RustAppShell,
) -> Result<CodegenOutput, miette::Error> {
    let out = emit::generate(module, package_name, shell)?;
    Ok(CodegenOutput {
        files: out.files,
        api_client_ts: out.api_client_ts,
    })
}

/// Target for script-mode execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTarget {
    /// Native Rust binary (tokio, vox-actor-runtime).
    Native,
    /// WASI binary (wasm32-wasip1, vox-script-wasi, no tokio).
    Wasi,
}

/// Generate a minimal Rust binary project for script-mode execution.
///
/// Unlike [generate], this skips all web server boilerplate (warp, axum,
/// rust-embed, metrics) and emits only the code needed to compile and run
/// a `fn main()`.
///
/// `runtime_path` should point to the vox-actor-runtime crate directory (e.g. workspace
/// `crates/vox-actor-runtime`). If `None`, uses `VOX_RUNTIME_PATH` env or a fallback.
pub fn generate_script(
    module: &HirModule,
    package_name: &str,
    runtime_path: Option<&Path>,
) -> Result<CodegenOutput, miette::Error> {
    generate_script_with_target(module, package_name, runtime_path, ScriptTarget::Native)
}

/// Generate a script project for the given target (Native or Wasi).
pub fn generate_script_with_target(
    module: &HirModule,
    package_name: &str,
    runtime_path: Option<&Path>,
    target: ScriptTarget,
) -> Result<CodegenOutput, miette::Error> {
    let mut rust_import_dep_lines = std::collections::BTreeMap::<String, String>::new();
    for dep in &module.rust_imports {
        let crate_name = dep.crate_name.trim();
        let is_template_dep = match target {
            ScriptTarget::Native => is_template_managed_script_native_dependency(crate_name),
            ScriptTarget::Wasi => is_template_managed_script_wasi_dependency(crate_name),
        };
        if crate_name.is_empty() || is_template_dep {
            continue;
        }
        let support = classify_rust_crate(crate_name).as_label();
        let dep_spec = if let Some(path) = &dep.path {
            format!("{crate_name} = {{ path = \"{path}\" }}")
        } else if let Some(git) = &dep.git {
            if let Some(rev) = &dep.rev {
                format!("{crate_name} = {{ git = \"{git}\", rev = \"{rev}\" }}")
            } else {
                format!("{crate_name} = {{ git = \"{git}\" }}")
            }
        } else if let Some(version) = &dep.version {
            format!("{crate_name} = \"{version}\"")
        } else {
            format!("{crate_name} = \"*\"")
        };
        let line = format!("# vox_rust_import support_class={support}\n{dep_spec}");
        rust_import_dep_lines
            .entry(crate_name.to_string())
            .or_insert(line);
    }
    let rust_import_deps = if rust_import_dep_lines.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n",
            rust_import_dep_lines
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let mut files = HashMap::new();

    let crate_name = package_name.replace('-', "_");

    let runtime_path_str = runtime_path
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .or_else(|| std::env::var("VOX_RUNTIME_PATH").ok())
        .unwrap_or_else(|| "../vox-actor-runtime".to_string());

    // ── WASI feature guardrail ──────────────────────────────────────────────
    // Jai-inspired: fail loudly and immediately with a clear diagnostic
    // rather than emitting broken code that produces confusing linker errors
    // or silent runtime panics inside the Wasmtime sandbox.
    if target == ScriptTarget::Wasi {
        let mut unsupported: Vec<String> = Vec::new();

        if !module.endpoint_fns.is_empty() {
            unsupported.push(format!(
                "endpoint functions are not supported in WASI mode: {}",
                module
                    .endpoint_fns
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !module.mcp_tools.is_empty() || !module.mcp_resources.is_empty() {
            let mut names: Vec<&str> = module
                .mcp_tools
                .iter()
                .map(|t| t.func.name.as_str())
                .collect();
            names.extend(module.mcp_resources.iter().map(|r| r.func.name.as_str()));
            unsupported.push(format!(
                "MCP tools/resources are not supported in WASI mode: {}",
                names.join(", ")
            ));
        }
        let mut wasi_blocked = Vec::new();
        for dep in &module.rust_imports {
            if is_wasi_unsupported_rust_import(dep.crate_name.as_str()) {
                wasi_blocked.push(dep.crate_name.clone());
            }
        }
        if !wasi_blocked.is_empty() {
            unsupported.push(format!(
                "some rust imports are not supported in WASI mode: {}",
                wasi_blocked.join(", ")
            ));
        }

        if !unsupported.is_empty() {
            return Err(miette::miette!(
                help = "Remove these features from the script, or run without --isolation wasm to use the full native runtime.",
                "WASI mode does not support the following features used in this script:\n  • {}",
                unsupported.join("\n  • ")
            ));
        }
    }

    let cargo_toml = match target {
        ScriptTarget::Native => format!(
            r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "{edition}"

[workspace]

[dependencies]
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
vox-actor-runtime = {{ path = "{runtime_path_str}" }}
{rust_import_deps}
"#,
            package_name = package_name,
            runtime_path_str = runtime_path_str,
            rust_import_deps = rust_import_deps,
            edition = GENERATED_CARGO_EDITION,
        ),
        ScriptTarget::Wasi => {
            let wasi_path = runtime_path
                .and_then(|p| p.parent())
                .map(|p| manifest_dependency_path(&p.join("vox-script-wasi")))
                .unwrap_or_else(|| "../vox-script-wasi".to_string());
            format!(
                r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "{edition}"

[workspace]

[dependencies]
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
{rust_import_deps}

[target.'cfg(target_arch = "wasm32")'.dependencies]
vox-script-wasi = {{ path = "{wasi_path}" }}
"#,
                package_name = package_name,
                wasi_path = wasi_path,
                rust_import_deps = rust_import_deps,
                edition = GENERATED_CARGO_EDITION,
            )
        }
    };
    files.insert("Cargo.toml".to_string(), cargo_toml);

    // Emit lib.rs with all non-main declarations (no warp/SSE for script mode)
    files.insert("src/lib.rs".to_string(), emit::emit_lib(module));

    // Emit a script-mode main.rs: just `use crate::*;` and the user's main fn body
    let mut main_rs = String::new();
    main_rs.push_str("// Generated by Vox Compiler (script mode)\n");
    main_rs.push_str("#![allow(unused)]\n\n");
    main_rs.push_str(&format!("use {}::*;\n\n", crate_name));

    let mut found_main = false;
    for func in &module.functions {
        if func.name == "main" {
            found_main = true;
            match target {
                ScriptTarget::Native => {
                    let is_async = func.is_async;
                    if is_async {
                        main_rs.push_str("#[tokio::main]\nasync fn main() {\n");
                    } else {
                        main_rs.push_str("fn main() {\n");
                    }
                    for stmt in &func.body {
                        main_rs.push_str(&emit::emit_main_stmt(stmt, 1, Some(&module.inferred_types)));
                    }
                    main_rs.push_str("}\n");
                }
                ScriptTarget::Wasi => {
                    if func.is_async {
                        // Jai-inspired: compile-time error, not a runtime surprise.
                        // async fn main() is not supported in WASI mode because Wasmtime P1
                        // does not expose an async executor — use native mode for async scripts.
                        main_rs.push_str("fn main() {\n");
                        main_rs.push_str("    compile_error!(\"async fn main() is not supported in --isolation wasm mode. \\nRemove async or use vox run without --isolation wasm.\");\n");
                        main_rs.push_str("}\n");
                    } else {
                        main_rs.push_str("fn main() {\n");
                        for stmt in &func.body {
                            // WASI: same Rust statement emission for now; builtin routing can be
                            // reintroduced when `vox-script-wasi` shims are wired to HIR again.
                            main_rs.push_str(&emit::emit_main_stmt(stmt, 1, Some(&module.inferred_types)));
                        }
                        main_rs.push_str("}\n");
                    }
                }
            }
            break;
        }
    }

    if !found_main {
        main_rs.push_str("fn main() {\n    eprintln!(\"No main function found.\");\n}\n");
    }

    files.insert("src/main.rs".to_string(), main_rs);

    Ok(CodegenOutput {
        files,
        api_client_ts: String::new(),
    })
}
