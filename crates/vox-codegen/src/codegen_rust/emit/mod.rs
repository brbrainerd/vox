//! Rust codegen emission (Axum server, lib, tables, TS client).
//!
//! Split from the historical single `emit.rs` (OP-0204).

use std::collections::HashMap;

use crate::projection_bundle::project_bundle_from_hir;
use vox_compiler::hir::HirModule;
use vox_compiler::rust_interop_support::{classify_rust_crate, is_template_managed_app_dependency};

use super::RustAppShell;

mod ai_fixture;
mod client;
mod durability_lower;
mod http;
mod main_boot;
mod method_emit;
pub(super) mod ownership;
mod state_machine;
mod stmt_expr;
mod stmt_expr_tail;
pub mod tables;
mod types;
mod usage;
mod with_emit;
mod workflow;

/// Generated bundles that call `execute_search_plan` need local path resolution.
fn module_has_distributed_subagent(module: &HirModule) -> bool {
    use vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture;
    let scan = |f: &vox_compiler::hir::HirFn| {
        matches!(
            &f.ai_fixture,
            Some(HirAiFixture::Subagent(s)) if s.policy.eq_ignore_ascii_case("distributed")
        )
    };
    module.functions.iter().any(scan)
        || module.tests.iter().any(scan)
        || module.mcp_tools.iter().any(|t| scan(&t.func))
        || module.mcp_resources.iter().any(|r| scan(&r.func))
        || module.foralls.iter().any(|forall| scan(&forall.func))
}

fn module_needs_vox_search_docs(module: &HirModule) -> bool {
    use vox_compiler::hir::nodes::boilerplate_grafts::HirAiFixture;
    let scan = |f: &vox_compiler::hir::HirFn| {
        matches!(
            &f.ai_fixture,
            Some(HirAiFixture::Search(s)) if s.corpus.eq_ignore_ascii_case("docs")
        )
    };
    module.functions.iter().any(scan)
        || module.tests.iter().any(scan)
        || module.mcp_tools.iter().any(|t| scan(&t.func))
        || module.mcp_resources.iter().any(|r| scan(&r.func))
        || module.foralls.iter().any(|forall| scan(&forall.func))
}

fn emit_generated_extra_deps(module: &HirModule) -> String {
    let mut out = String::new();
    out.push_str("vox-telemetry = { path = \"../../crates/vox-telemetry\" }\n");
    if module_needs_vox_search_docs(module) {
        out.push_str(
            "vox-search = { path = \"../../crates/vox-search\", default-features = false }\n",
        );
        out.push_str("vox-repository = { path = \"../../crates/vox-repository\" }\n");
    }
    out
}

pub use client::{emit_api_client, emit_mcp_server};
pub use http::emit_main;
pub use main_boot::{emit_durable_boot_helpers, emit_durable_boot_prelude, emit_main_boot};
pub use stmt_expr::{emit_expr, emit_main_stmt};
pub use tables::{
    emit_index_ddl, emit_table_ddl, emit_table_struct, validate_db_projection_suffixes_unique,
};
pub use workflow::{emit_fn, emit_lib};

pub struct CodegenOutput {
    /// Relative path → file contents (e.g. `Cargo.toml`, `src/main.rs`).
    pub files: HashMap<String, String>,
    /// TypeScript API client for server functions (empty if no server fns)
    pub api_client_ts: String,
}

/// Emit a minimal Axum + Turso backend crate from `module` (paths relative to generated root).
fn format_generated_lib_rs(src: &str) -> String {
    match syn::parse_file(src) {
        Ok(ast) => prettyplease::unparse(&ast),
        Err(_) => src.to_string(),
    }
}

fn rust_app_shell_marker(shell: RustAppShell) -> &'static str {
    match shell {
        RustAppShell::AxumLocalServer => "// vox-generated rust_app_shell=AxumLocalServer\n",
        RustAppShell::TauriApp => "// vox-generated rust_app_shell=TauriApp\n",
    }
}

pub fn generate(
    module: &HirModule,
    package_name: &str,
    shell: RustAppShell,
) -> Result<CodegenOutput, miette::Error> {
    match shell {
        RustAppShell::TauriApp => generate_tauri_workspace(module, package_name),
        RustAppShell::AxumLocalServer => generate_axum_local_server(module, package_name, shell),
    }
}

/// Axum + embedded static assets (`native-binary` / default `vox build`).
fn generate_axum_local_server(
    module: &HirModule,
    package_name: &str,
    shell: RustAppShell,
) -> Result<CodegenOutput, miette::Error> {
    let mut files = HashMap::new();

    let table_projections = tables::collect_table_select_projections(module);
    for table in &module.tables {
        if let Some(projs) = table_projections.get(&table.name) {
            tables::validate_db_projection_suffixes_unique(&table.name, projs)?;
        }
    }

    // Cargo.toml
    files.insert(
        "Cargo.toml".to_string(),
        emit_cargo_toml(package_name, module),
    );

    let bundle = project_bundle_from_hir(module);

    // src/main.rs (Entry point + Routes)
    let main_rs = emit_main(module, package_name, &bundle.app);
    files.insert(
        "src/main.rs".to_string(),
        format!("{}{}", rust_app_shell_marker(shell), main_rs),
    );

    // src/lib.rs (Types, Actors, Workflows, Functions)
    let lib_rs = emit_lib(module);
    files.insert("src/lib.rs".to_string(), format_generated_lib_rs(&lib_rs));
    if let Ok(contract_json) = serde_json::to_string_pretty(&bundle.app) {
        files.insert("app_contract.json".to_string(), contract_json);
    }

    // Legacy `api.ts` emission removed — use Contract-IR-driven `vox-client.ts` from TS codegen.
    let api_client_ts = String::new();

    // MCP stdio server when `@mcp.tool` and/or `@mcp.resource` declarations are present.
    if !module.mcp_tools.is_empty() || !module.mcp_resources.is_empty() {
        files.insert(
            "src/mcp_server.rs".to_string(),
            emit_mcp_server(module, package_name),
        );
    }

    // Minimal static tree so `rust_embed::Embed` on `public/` always has compile-time inputs
    // (empty/missing folders break `#[derive(Embed)]` in generated `main.rs`).
    files.insert(
        "public/index.html".to_string(),
        concat!(
            "<!doctype html><html lang=\"en\"><head>",
            "<meta charset=\"utf-8\"/><title>Vox</title>",
            "</head><body></body></html>\n",
        )
        .to_string(),
    );

    Ok(CodegenOutput {
        files,
        api_client_ts,
    })
}

/// Tauri 2 desktop/mobile shell: workspace root + `src-tauri/` binary crate (no Axum in `main`).
///
/// `@endpoint` functions are lowered to `#[tauri::command]` handlers in `src-tauri/src/main.rs`
/// (see [`emit_tauri_main_rs`]). The browser/TS side uses Contract-IR [`crate::codegen_ts::vox_client`]
/// (`vox-client.ts`), which branches on `isTauri()` and calls `invoke` instead of `fetch`.
/// [`CodegenOutput::api_client_ts`] stays empty: legacy Rust-path `api.ts` emit is retired
/// ([`emit_api_client`]); consumers must import generated `vox-client.ts`.
fn generate_tauri_workspace(
    module: &HirModule,
    package_name: &str,
) -> Result<CodegenOutput, miette::Error> {
    let mut files = HashMap::new();

    let table_projections = tables::collect_table_select_projections(module);
    for table in &module.tables {
        if let Some(projs) = table_projections.get(&table.name) {
            tables::validate_db_projection_suffixes_unique(&table.name, projs)?;
        }
    }

    files.insert("Cargo.toml".to_string(), emit_workspace_root_toml());

    files.insert(
        "src-tauri/Cargo.toml".to_string(),
        emit_cargo_toml_tauri_app(package_name, module),
    );

    files.insert("src-tauri/build.rs".to_string(), emit_tauri_build_rs());

    let marker = rust_app_shell_marker(RustAppShell::TauriApp);
    files.insert(
        "src-tauri/src/main.rs".to_string(),
        emit_tauri_main_rs(marker, module, package_name),
    );

    let lib_rs = emit_lib(module);
    files.insert(
        "src-tauri/src/lib.rs".to_string(),
        format_generated_lib_rs(&lib_rs),
    );

    let bundle = project_bundle_from_hir(module);
    if let Ok(contract_json) = serde_json::to_string_pretty(&bundle.app) {
        files.insert("app_contract.json".to_string(), contract_json);
    }

    let display = package_name.replace('_', " ");
    let tauri_params = vox_tauri_codegen::TauriEmitParams {
        identifier: "com.vox.generated",
        display_name: &display,
        frontend_dist_relative: "../public",
    };
    let tauri_conf = vox_tauri_codegen::serialize_tauri_desktop_config(&tauri_params)
        .map_err(|e| miette::miette!("{:#}", e))?;
    files.insert("src-tauri/tauri.conf.json".to_string(), tauri_conf);

    files.insert(
        "src-tauri/capabilities/default.json".to_string(),
        emit_tauri_default_capability_json(),
    );

    if !module.mcp_tools.is_empty() || !module.mcp_resources.is_empty() {
        files.insert(
            "src-tauri/src/mcp_server.rs".to_string(),
            emit_mcp_server(module, package_name),
        );
    }

    files.insert(
        "public/index.html".to_string(),
        concat!(
            "<!doctype html><html lang=\"en\"><head>",
            "<meta charset=\"utf-8\"/><title>Vox</title>",
            "</head><body></body></html>\n",
        )
        .to_string(),
    );

    Ok(CodegenOutput {
        files,
        // Intentionally empty — see module comment on [`generate_tauri_workspace`].
        api_client_ts: String::new(),
    })
}

fn emit_workspace_root_toml() -> String {
    r#"[workspace]
resolver = "2"
members = ["src-tauri"]
"#
    .to_string()
}

fn emit_tauri_build_rs() -> String {
    r#"fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().plugin(
            "vox-stt",
            tauri_build::InlinedPlugin::new()
                .commands(&["transcribe"])
                .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
        ),
    )
    .expect("failed to run tauri-build (ACL / codegen)");
}
"#
    .to_string()
}

fn emit_tauri_main_rs(shell_header: &str, module: &HirModule, package_name: &str) -> String {
    let mut out = format!(
        r#"{shell_header}// Generated by Vox Compiler — Tauri 2 application entry
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use {}::*;

"#,
        package_name.replace('-', "_")
    );

    let has_tables = !module.tables.is_empty();
    let mut command_names = Vec::new();

    for sf in &module.endpoint_fns {
        command_names.push(sf.name.clone());

        out.push_str("#[tauri::command]\n");
        out.push_str(&format!("async fn {}(", sf.name));

        if has_tables {
            out.push_str("db: tauri::State<'_, std::sync::Arc<vox_db::Codex>>, ");
        }
        out.push_str("request: serde_json::Value)");

        let ret_type = sf
            .return_type
            .as_ref()
            .map(|t| types::emit_type(t))
            .unwrap_or_else(|| "()".to_string());

        if ret_type != "()" {
            out.push_str(&format!(" -> {} {{\n", ret_type));
        } else {
            out.push_str(" {\n");
        }

        if has_tables {
            out.push_str("    let db = &*db;\n");
        }

        for param in &sf.params {
            out.push_str(&format!(
                "    let {} = request[\"{}\"].clone();\n",
                param.name, param.name
            ));
        }

        for stmt in &sf.body {
            let emitted =
                stmt_expr::emit_stmt(stmt, 1, false, false, false, Some(&module.inferred_types), None, None);
            out.push_str(&emitted);
        }

        out.push_str("}\n\n");
    }

    out.push_str("fn main() {\n");
    out.push_str("    tauri::Builder::default()\n");
    out.push_str("        .plugin(vox_tauri_stt::plugin::init())\n");

    if !command_names.is_empty() {
        out.push_str(&format!(
            "        .invoke_handler(tauri::generate_handler![{}])\n",
            command_names.join(", ")
        ));
    }

    if has_tables {
        out.push_str("        .setup(|app| {\n");
        out.push_str(r#"            let db_url = std::env::var("VOX_DB_URL").unwrap_or_else(|_| "sqlite://local.db".to_string());
            let db_token = std::env::var("VOX_DB_TOKEN").unwrap_or_default();
            let db = vox_db::Codex::open_with_embedded_migrations(&db_url, &db_token);
            app.manage(std::sync::Arc::new(db));
            Ok(())
        })
"#);
    }

    out.push_str("        .run(tauri::generate_context!())\n");
    out.push_str("        .expect(\"error while running tauri application\");\n");
    out.push_str("}\n");

    out
}

fn emit_tauri_default_capability_json() -> String {
    r#"{
  "$schema": "https://schema.tauri.app/config/2/capability.json",
  "identifier": "default",
  "description": "Default permissions for the main window",
  "windows": ["main"],
  "permissions": ["core:default", "vox-stt:default"]
}
"#
    .to_string()
}

/// Path deps in `src-tauri/Cargo.toml` must reach the repo `crates/` tree (`..` ×3 from `target/generated/src-tauri/`).
fn adjust_crate_paths_for_src_tauri_manifest(deps: &str) -> String {
    deps.replace("../../crates/", "../../../crates/")
}

/// `src-tauri/Cargo.toml` — Tauri binary + library; omits Axum/rust-embed server stack.
fn emit_cargo_toml_tauri_app(name: &str, module: &HirModule) -> String {
    let rust_import_deps =
        adjust_crate_paths_for_src_tauri_manifest(&emit_rust_import_dependencies(module));
    let fixture_deps =
        adjust_crate_paths_for_src_tauri_manifest(&emit_generated_extra_deps(module));
    let features_section = if module_has_distributed_subagent(module) {
        "[features]\ndefault = [\"populi-transport\"]\npopuli-transport = [\"vox-orchestrator/populi-transport\"]\n\n"
    } else {
        ""
    };
    let mcp_bin = if !module.mcp_tools.is_empty() || !module.mcp_resources.is_empty() {
        r#"

[[bin]]
name = "mcp_server"
path = "src/mcp_server.rs"
"#
    } else {
        ""
    };
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "{edition}"
default-run = "{name}"

[lib]
name = "{lib_name}"
path = "src/lib.rs"

[[bin]]
name = "{name}"
path = "src/main.rs"

[build-dependencies]
tauri-build = "2"

{features_section}[dependencies]
tauri = "2"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
reqwest = {{ version = "0.12", default-features = false, features = ["rustls-tls"] }}
vox-http-client = {{ path = "../../../crates/vox-http-client" }}
tracing = "0.1"
tracing-subscriber = "0.3"
turso = {{ version = "0.4", default-features = false }}
vox-db = {{ path = "../../../crates/vox-db" }}
vox-actor-runtime = {{ path = "../../../crates/vox-actor-runtime" }}
vox-orchestrator = {{ path = "../../../crates/vox-orchestrator" }}
vox-oratio = {{ path = "../../../crates/vox-oratio" }}
vox-tauri-stt = {{ path = "../../../crates/vox-tauri-stt", features = ["tauri-plugin"] }}
# P9 (2026-05-24): durable boot prelude — see vox-codegen emit/main_boot.rs.
# (Tauri main.rs does not yet emit the prelude; deps added for symmetry +
# so future Tauri-side durable wiring is unblocked.)
vox-compiler = {{ path = "../../../crates/vox-compiler" }}
vox-workflow-runtime = {{ path = "../../../crates/vox-workflow-runtime", default-features = false }}
{fixture_deps}{rust_import_deps}

[dev-dependencies]
proptest = "1"
{mcp_bin}"#,
        lib_name = name.replace('-', "_"),
        features_section = features_section,
        fixture_deps = fixture_deps,
        rust_import_deps = rust_import_deps,
        mcp_bin = mcp_bin,
        edition = crate::codegen_rust::GENERATED_CARGO_EDITION,
    )
}

fn emit_rust_import_dependencies(module: &HirModule) -> String {
    let mut lines = std::collections::BTreeMap::<String, String>::new();
    for dep in &module.rust_imports {
        let crate_name = dep.crate_name.trim();
        if crate_name.is_empty() {
            continue;
        }
        if is_template_managed_app_dependency(crate_name) {
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
        lines.entry(crate_name.to_string()).or_insert(line);
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n",
            lines.values().cloned().collect::<Vec<_>>().join("\n")
        )
    }
}

/// `Cargo.toml` body for the generated Rust package `name` (Axum local-server shell).
pub fn emit_cargo_toml(name: &str, module: &HirModule) -> String {
    let rust_import_deps = emit_rust_import_dependencies(module);
    let fixture_deps = emit_generated_extra_deps(module);
    let features_section = if module_has_distributed_subagent(module) {
        "[features]\ndefault = [\"populi-transport\"]\npopuli-transport = [\"vox-orchestrator/populi-transport\"]\n\n"
    } else {
        ""
    };
    let mcp_bin = if !module.mcp_tools.is_empty() || !module.mcp_resources.is_empty() {
        r#"

[[bin]]
name = "mcp_server"
path = "src/mcp_server.rs"
"#
    } else {
        ""
    };
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "{edition}"

{features_section}[workspace]

[dependencies]
tokio = {{ version = "1", features = ["full"] }}
# tokio-stream + futures-util drive the SSE handler emitted for
# `@endpoint(kind: stream)` (axum::response::sse::Sse). The `sync`
# feature on tokio-stream provides BroadcastStream, which the
# `subscribe(Actor)` bridge wraps around the actor's SubscriptionManager
# channel.
tokio-stream = {{ version = "0.1", features = ["sync"] }}
futures-util = "0.3"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
axum = "0.7"
tower = "0.4"
tower-http = {{ version = "0.5", features = ["cors", "trace", "request-id"] }}
governor = "0.10"
rust-embed = "8"
mime_guess = "2"
reqwest = {{ version = "0.12", default-features = false, features = ["rustls-tls"] }}
vox-http-client = {{ path = "../../crates/vox-http-client" }}
tracing = "0.1"
tracing-subscriber = "0.3"
turso = {{ version = "0.4", default-features = false }}
vox-db = {{ path = "../../crates/vox-db" }}
vox-actor-runtime = {{ path = "../../crates/vox-actor-runtime" }}
vox-orchestrator = {{ path = "../../crates/vox-orchestrator" }}
vox-oratio = {{ path = "../../crates/vox-oratio" }}
# P9 (2026-05-24): durable boot prelude in main.rs references both — see
# crates/vox-codegen/src/codegen_rust/emit/main_boot.rs (HirModule embed
# + scheduled runner + set_current_hir_module).
vox-compiler = {{ path = "../../crates/vox-compiler" }}
vox-workflow-runtime = {{ path = "../../crates/vox-workflow-runtime", default-features = false }}
{fixture_deps}{rust_import_deps}{mcp_bin}"#,
        features_section = features_section,
        fixture_deps = fixture_deps,
        rust_import_deps = rust_import_deps,
        mcp_bin = mcp_bin,
        edition = crate::codegen_rust::GENERATED_CARGO_EDITION,
    )
}
