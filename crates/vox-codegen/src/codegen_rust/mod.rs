//! Vox → Rust code generator.
//!
//! Generates a full Rust web server project from a Vox HIR module.
//!
//! Integration tests also use [`emit`] as `vox_codegen_rust::emit::*`.

#![allow(clippy::collapsible_if)]

pub mod emit;

mod manifest;
mod pipeline;

pub use emit::emit_api_client;

/// Selects the generated Rust **application shell** for full-stack app bundles.
///
/// - [`RustAppShell::AxumLocalServer`] — localhost Axum + embedded assets (`native-binary`, default `vox build`).
/// - [`RustAppShell::TauriApp`] — Tauri 2 packaging path (`vox compile --target desktop|mobile-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RustAppShell {
    #[default]
    AxumLocalServer,
    TauriApp,
}

/// Rust `edition` written into generated `Cargo.toml` files (keep aligned with root workspace).
pub const GENERATED_CARGO_EDITION: &str = "2024";

pub use manifest::CodegenOutput;
pub use pipeline::{ScriptTarget, generate, generate_script, generate_script_with_target};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection_bundle::project_bundle_from_hir;
    use emit::{emit_cargo_toml, emit_main, emit_table_struct};
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{
        DefId, HirEndpointFn, HirExpr, HirModule, HirRustImport, HirStmt, HirTable, HirTableField,
        HirType,
    };

    fn empty_module() -> HirModule {
        HirModule::default()
    }

    fn simple_task_table() -> HirTable {
        HirTable {
            id: DefId(1),
            name: "Task".to_string(),
            fields: vec![
                HirTableField {
                    name: "id".to_string(),
                    type_ann: HirType::Named("int".to_string()),
                    span: Span::new(0, 0),
                },
                HirTableField {
                    name: "title".to_string(),
                    type_ann: HirType::Named("str".to_string()),
                    span: Span::new(0, 0),
                },
                HirTableField {
                    name: "done".to_string(),
                    type_ann: HirType::Named("bool".to_string()),
                    span: Span::new(0, 0),
                },
                HirTableField {
                    name: "priority".to_string(),
                    type_ann: HirType::Generic(
                        "Option".to_string(),
                        vec![HirType::Named("int".to_string())],
                    ),
                    span: Span::new(0, 0),
                },
            ],
            is_pub: true,
            is_deprecated: false,
            span: Span::new(0, 0),
        }
    }

    // ── emit_table_struct: typed DSL surface ──────────────────────────────────

    #[test]
    #[ignore = "emit_table_struct DSL drift vs tests — reconcile when table codegen stabilizes — owner: codegen sunset: 2026-12-31"]
    fn emits_struct_and_patch_struct() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        assert!(out.contains("pub struct Task {"), "should emit Task struct");
        assert!(
            out.contains("pub struct TaskPatch {"),
            "should emit TaskPatch struct"
        );
        // patch fields are Option<T>
        assert!(
            out.contains("pub title: Option<String>"),
            "patch.title should be Option<String>"
        );
        assert!(
            out.contains("pub done: Option<bool>"),
            "patch.done should be Option<bool>"
        );
    }

    #[test]
    fn emits_id_field() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        assert!(
            out.contains("pub _id: Option<i64>"),
            "should emit _id Option<i64>"
        );
    }

    #[test]
    #[ignore = "emit_table_struct DSL drift vs tests — reconcile when table codegen stabilizes — owner: codegen sunset: 2026-12-31"]
    fn emits_all_typed_dsl_methods() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        // Typed DSL surface — these are the LLM-native methods
        assert!(out.contains("pub async fn all("), "should emit all()");
        assert!(
            out.contains("pub async fn all_ordered("),
            "should emit all_ordered()"
        );
        assert!(out.contains("pub async fn find("), "should emit find()");
        assert!(
            out.contains("pub async fn where_eq("),
            "should emit where_eq()"
        );
        assert!(out.contains("pub async fn count("), "should emit count()");
        assert!(
            out.contains("pub async fn insert_typed("),
            "should emit insert_typed()"
        );
        assert!(
            out.contains("pub async fn update_id("),
            "should emit update_id()"
        );
        assert!(
            out.contains("pub async fn update_patch("),
            "should emit update_patch()"
        );
        assert!(
            out.contains("pub async fn delete_id("),
            "should emit delete_id()"
        );
        assert!(
            out.contains("pub async fn fts_search("),
            "should emit fts_search()"
        );
    }

    #[test]
    fn emits_legacy_escape_hatch_methods() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        // Legacy compat — must remain as frozen escape hatches
        assert!(
            out.contains("pub async fn insert("),
            "should retain legacy insert"
        );
        assert!(
            out.contains("pub async fn get("),
            "should retain legacy get"
        );
        assert!(out.contains("pub async fn all("), "should emit safe all()");
        assert!(
            out.contains("pub async fn unsafe_query_raw_clause("),
            "should retain escape-hatch dynamic SQL (unsafe_query_raw_clause)"
        );
        assert!(
            out.contains("pub async fn delete("),
            "should retain legacy delete"
        );
        assert!(
            out.contains("pub async fn count("),
            "should emit safe count()"
        );
        assert!(
            out.contains("pub async fn count_where("),
            "should emit parameterized count_where()"
        );
        assert!(
            out.contains("pub async fn all_order_limit("),
            "should emit all_order_limit()"
        );
        assert!(
            out.contains("pub async fn filter_where_order_limit("),
            "should emit filter_where_order_limit()"
        );
        assert!(
            out.contains("pub async fn filter_where("),
            "should emit parameterized filter_where()"
        );
    }

    #[test]
    fn emits_correct_table_name_in_sql() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        assert!(
            out.contains("FROM task"),
            "SQL should reference lowercase table name 'task'"
        );
        assert!(
            out.contains("INSERT INTO task"),
            "INSERT should use lowercase table name"
        );
        assert!(
            out.contains("DELETE FROM task"),
            "DELETE should use lowercase table name"
        );
    }

    #[test]
    fn rejects_colliding_select_projection_suffixes() {
        use super::emit::validate_db_projection_suffixes_unique;
        let err = validate_db_projection_suffixes_unique(
            "Task",
            &[
                vec!["x".into(), "y_z".into()],
                vec!["x_y".into(), "z".into()],
            ],
        )
        .expect_err("x_y_z projection suffix collision");
        let msg = format!("{err}");
        assert!(msg.contains("suffix") && msg.contains("Task"), "{msg}");
    }

    #[test]
    fn emits_select_projection_helpers_when_configured() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[vec!["title".into(), "done".into()]]);
        assert!(
            out.contains("fn from_row_sel_title_done"),
            "should emit from_row_sel_* for projection"
        );
        assert!(out.contains("all_proj_title_done"));
        assert!(
            out.contains("SELECT _id, title, done FROM task"),
            "projection SQL should list explicit columns"
        );
    }

    #[test]
    #[ignore = "emit_table_struct DSL drift vs tests — reconcile when table codegen stabilizes — owner: codegen sunset: 2026-12-31"]
    fn bool_field_maps_to_i64_in_from_row() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        // bool stored as i64 (0/1)
        assert!(
            out.contains("row.get::<i64>") && out.contains("!= 0"),
            "bool field should use i64 + != 0 in from_row"
        );
    }

    #[test]
    #[ignore = "emit_table_struct DSL drift vs tests — reconcile when table codegen stabilizes — owner: codegen sunset: 2026-12-31"]
    fn option_field_maps_to_option_type_in_from_row() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        assert!(
            out.contains("row.get::<Option<i64>>"),
            "Option<int> should deserialize via Option<i64>"
        );
    }

    #[test]
    fn find_returns_bare_t_with_error_on_missing() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        // find must return Self with turso::Error if missing, per vox non-null policy
        assert!(
            out.contains("Result<Self, turso::Error>"),
            "find, get must return Result<Self> (and error if missing) per non-null policy"
        );
    }

    #[test]
    #[ignore = "emit_table_struct DSL drift vs tests — reconcile when table codegen stabilizes — owner: codegen sunset: 2026-12-31"]
    fn insert_typed_uses_turso_value_not_clone() {
        let table = simple_task_table();
        let out = emit_table_struct(&table, &[]);
        // insert_typed uses turso::Value, not .clone() raw value
        assert!(
            out.contains("turso::Value::Text(") || out.contains("turso::Value::Integer("),
            "insert_typed should use turso::Value constructors"
        );
    }

    // ── WASI guardrail tests (unchanged) ─────────────────────────────────────

    #[test]
    fn wasi_clean_script_passes_guardrail() {
        let module = empty_module();
        let result = generate_script_with_target(&module, "test-script", None, ScriptTarget::Wasi);
        assert!(result.is_ok(), "clean WASI script should pass guardrail");
    }

    #[test]
    fn script_cargo_toml_merges_rust_import_dependencies() {
        let sp = Span::new(0, 0);
        let mut module = empty_module();
        module.rust_imports.push(HirRustImport {
            crate_name: "chrono".to_string(),
            alias: "chrono".to_string(),
            version: Some("0.4".to_string()),
            path: None,
            git: None,
            rev: None,
            span: sp,
        });
        let out =
            generate_script_with_target(&module, "vox-script", None, ScriptTarget::Native).unwrap();
        let cargo = out.files.get("Cargo.toml").expect("Cargo.toml present");
        assert!(
            cargo.contains("chrono") && cargo.contains("0.4"),
            "expected merged crate dep in Cargo.toml:\n{cargo}"
        );
    }

    #[test]
    fn app_emit_cargo_toml_includes_rust_import_lines() {
        let sp = Span::new(0, 0);
        let mut module = empty_module();
        module.rust_imports.push(HirRustImport {
            crate_name: "uuid".to_string(),
            alias: "uuid".to_string(),
            version: Some("1".to_string()),
            path: None,
            git: None,
            rev: None,
            span: sp,
        });
        let toml = emit_cargo_toml("demo_pkg", &module);
        assert!(
            toml.contains("uuid") && toml.contains("\"1\""),
            "expected rust import in full-app Cargo.toml:\n{toml}"
        );
    }

    #[test]
    fn emit_main_registers_query_and_mutation_routes() {
        let sp = Span::new(0, 0);
        let mut module = empty_module();
        module.endpoint_fns.push(HirEndpointFn {
            kind: vox_compiler::hir::HirEndpointKind::Query,
            id: DefId(10),
            name: "q1".into(),
            params: vec![],
            return_type: None,
            body: vec![HirStmt::Return {
                value: Some(HirExpr::IntLit(0, sp)),
                span: sp,
            }],
            route_path: "/api/query/q1".into(),
            is_pure: false,
            effects: vec![],
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            span: sp,
        });
        module.endpoint_fns.push(HirEndpointFn {
            kind: vox_compiler::hir::HirEndpointKind::Mutation,
            id: DefId(11),
            name: "m1".into(),
            params: vec![],
            return_type: None,
            body: vec![HirStmt::Return {
                value: Some(HirExpr::IntLit(0, sp)),
                span: sp,
            }],
            route_path: "/api/mutation/m1".into(),
            is_pure: false,
            effects: vec![],
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            span: sp,
        });

        let bundle = project_bundle_from_hir(&module);
        let out = emit_main(&module, "demo_pkg", &bundle.app);
        assert!(out.contains(".route(\"/api/query/q1\", get(handle_q_q1))"));
        assert!(out.contains(".route(\"/api/mutation/m1\", post(handle_m_m1))"));
        assert!(out.contains("async fn handle_q_q1("));
        assert!(out.contains("Query(q): Query<std::collections::BTreeMap<String, String>>"));
        assert!(out.contains("async fn handle_m_m1("));
    }

    #[test]
    fn emit_main_mutation_wraps_db_transaction_when_codex_present() {
        let sp = Span::new(0, 0);
        let mut module = empty_module();
        module.tables.push(simple_task_table());
        module.endpoint_fns.push(HirEndpointFn {
            kind: vox_compiler::hir::HirEndpointKind::Mutation,
            id: DefId(11),
            name: "m1".into(),
            params: vec![],
            return_type: None,
            body: vec![HirStmt::Return {
                value: Some(HirExpr::IntLit(0, sp)),
                span: sp,
            }],
            route_path: "/api/mutation/m1".into(),
            is_pure: false,
            effects: vec![],
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            span: sp,
        });

        let bundle = project_bundle_from_hir(&module);
        let out = emit_main(&module, "demo_pkg", &bundle.app);
        assert!(
            out.contains("async fn handle_m_m1("),
            "expected mutation handler: {out}"
        );
        assert!(
            out.contains("match db.transaction(async move"),
            "mutation with @table should wrap handler body in Codex::transaction: {out}"
        );
        assert!(
            out.contains("return Ok(Json(serde_json::to_value"),
            "mutation JSON returns should use Result for transactional handler: {out}"
        );
    }

    #[test]
    fn rust_app_shell_marker_axum_in_main_rs() {
        let module = empty_module();
        let out = pipeline::generate(&module, "pkg", RustAppShell::AxumLocalServer).unwrap();
        let main = out.files.get("src/main.rs").expect("main.rs");
        assert!(main.contains("rust_app_shell=AxumLocalServer"), "{main}");
    }

    #[test]
    fn rust_app_shell_marker_tauri_in_main_rs() {
        let module = empty_module();
        let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
        let main = out
            .files
            .get("src-tauri/src/main.rs")
            .expect("src-tauri main.rs");
        assert!(main.contains("rust_app_shell=TauriApp"), "{main}");
    }

    #[test]
    fn tauri_main_registers_scheduled_fns() {
        let src = r#"
@scheduled("1m")
fn heartbeat() { }
"#;
        let res =
            vox_compiler::pipeline::run_frontend_str(src, "sched_test.vox").expect("frontend ok");
        let module = res.hir;
        let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
        let main = out
            .files
            .get("src-tauri/src/main.rs")
            .expect("src-tauri main.rs");
        assert!(
            main.contains("vox_workflow_runtime::scheduled::register"),
            "must register @scheduled fns: {main}"
        );
        assert!(
            main.contains("scheduled::start"),
            "must start scheduler: {main}"
        );
        assert!(
            main.contains("load_hir_module_from_embedded"),
            "must embed+register HirModule: {main}"
        );
    }

    #[test]
    fn tauri_tables_only_emits_setup_without_scheduler() {
        // Regression guard: the `needs_setup` refactor must still emit `.setup()` + Codex
        // management for a @table-only module (no @scheduled functions).
        let mut module = empty_module();
        module.tables.push(simple_task_table());
        let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
        let main = out.files.get("src-tauri/src/main.rs").unwrap();
        assert!(
            main.contains(".setup("),
            "tables-only app must still emit a setup block: {main}"
        );
        assert!(
            main.contains("app.manage"),
            "tables-only app must manage the Codex db: {main}"
        );
        assert!(
            !main.contains("scheduled::register"),
            "tables-only app must NOT register a scheduler: {main}"
        );
    }

    #[test]
    fn tauri_emit_registers_sherpa_acl_in_build_rs() {
        let module = vox_compiler::pipeline::run_frontend_str(
            "fn note() -> Result[str] { Speech.transcribe_microphone() }",
            "t.vox",
        )
        .expect("frontend ok")
        .hir;
        let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
        let build_rs = out.files.get("src-tauri/build.rs").expect("build.rs");
        assert!(
            build_rs.contains("InlinedPlugin::new()"),
            "expected tauri-build inlined plugin ACL: {build_rs}"
        );
        assert!(
            build_rs.contains("\"vox-stt\""),
            "expected plugin id vox-stt in build.rs: {build_rs}"
        );
        let cargo = out
            .files
            .get("src-tauri/Cargo.toml")
            .expect("src-tauri/Cargo.toml");
        assert!(
            cargo.contains("vox-tauri-stt") && cargo.contains("tauri-plugin"),
            "expected vox-tauri-stt path dep with feature: {cargo}"
        );
        assert!(
            cargo.contains("path = \"../../../crates/vox-actor-runtime\""),
            "src-tauri path deps must use ../../../crates (manifest under target/generated/src-tauri): {cargo}"
        );
        assert!(
            !cargo.contains("path = \"../../crates/vox-actor-runtime\""),
            "src-tauri must not use ../../crates (wrong resolve from src-tauri/): {cargo}"
        );
        let cap = out
            .files
            .get("src-tauri/capabilities/default.json")
            .expect("default capability");
        assert!(
            cap.contains("vox-stt:default"),
            "expected vox-stt default permission in capability: {cap}"
        );
    }

    #[test]
    fn tauri_emits_stt_only_when_speech_used() {
        let module = vox_compiler::pipeline::run_frontend_str(
            "fn note() -> Result[str] { Speech.transcribe_microphone() }",
            "t.vox",
        )
        .expect("frontend ok")
        .hir;
        let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
        assert!(
            out.files
                .get("src-tauri/src/main.rs")
                .unwrap()
                .contains("vox_tauri_stt::plugin::init()")
        );
        assert!(
            out.files
                .get("src-tauri/build.rs")
                .unwrap()
                .contains("\"vox-stt\"")
        );
        assert!(
            out.files
                .get("src-tauri/capabilities/default.json")
                .unwrap()
                .contains("vox-stt:default")
        );
    }

    #[test]
    fn tauri_emits_stt_for_file_transcribe_without_mic_permission() {
        // File-based `transcribe(path)` derives `speech` (STT plugin) but NOT `microphone`.
        let module = vox_compiler::pipeline::run_frontend_str(
            "fn note() -> Result[str] { Speech.transcribe(\"/tmp/a.wav\") }",
            "t.vox",
        )
        .expect("frontend ok")
        .hir;
        let caps = crate::projection_bundle::project_bundle_from_hir(&module)
            .capabilities
            .capability_ids;
        assert!(caps.iter().any(|c| c == "speech"), "{caps:?}");
        assert!(!caps.iter().any(|c| c == "microphone"), "{caps:?}");
        let out = pipeline::generate(&module, "pkg", RustAppShell::TauriApp).unwrap();
        // STT plugin still emitted (gated on speech).
        assert!(
            out.files
                .get("src-tauri/src/main.rs")
                .unwrap()
                .contains("vox_tauri_stt::plugin::init()")
        );
        assert!(
            out.files
                .get("src-tauri/capabilities/default.json")
                .unwrap()
                .contains("vox-stt:default")
        );
    }

    #[test]
    fn tauri_omits_stt_without_speech() {
        let out = pipeline::generate(&empty_module(), "pkg", RustAppShell::TauriApp).unwrap();
        assert!(
            !out.files
                .get("src-tauri/src/main.rs")
                .unwrap()
                .contains("vox_tauri_stt")
        );
        assert!(
            !out.files
                .get("src-tauri/build.rs")
                .unwrap()
                .contains("vox-stt")
        );
        assert!(
            !out.files
                .get("src-tauri/capabilities/default.json")
                .unwrap()
                .contains("vox-stt")
        );
        assert!(
            !out.files
                .get("src-tauri/Cargo.toml")
                .unwrap()
                .contains("vox-tauri-stt"),
            "vox-tauri-stt must not appear in Cargo.toml without speech"
        );
    }

    #[test]
    fn tauri_workspace_cargo_excludes_axum_from_root() {
        let module = empty_module();
        let out = pipeline::generate(&module, "vox_generated_app", RustAppShell::TauriApp).unwrap();
        let root = out.files.get("Cargo.toml").expect("workspace Cargo.toml");
        assert!(
            root.contains("[workspace]") && root.contains("src-tauri"),
            "{root}"
        );
        assert!(
            !root.contains("axum"),
            "workspace root should not list axum: {root}"
        );
        let st = out
            .files
            .get("src-tauri/Cargo.toml")
            .expect("src-tauri/Cargo.toml");
        assert!(
            !st.contains("axum") && !st.contains("rust-embed"),
            "unexpected server deps in Tauri crate: {st}"
        );
        assert!(st.contains("tauri"), "expected tauri dep: {st}");
        assert!(
            out.files
                .get("src-tauri/tauri.conf.json")
                .expect("tauri.conf.json")
                .contains("com.vox.generated"),
            "expected placeholder bundle id in tauri.conf.json"
        );
    }
}
