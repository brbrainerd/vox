//! Canonical golden ladder driver — backwards verification from real fixtures.
//!
//! Loads [`contracts/pipeline/canonical-ladder.v1.yaml`] and drives each entry through
//! bundle projection + emission profile validation (and rust-script compile where required).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use vox_codegen::canonical_ladder::CanonicalLadder;
use vox_codegen::codegen_rust::generate_script;
use vox_codegen::projection_bundle::project_and_validate;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::target::Target;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_hir_module;

static COMPILE_LOCK: Mutex<()> = Mutex::new(());

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_ladder() -> CanonicalLadder {
    CanonicalLadder::load_from_repo_root(&repo_root()).expect("canonical ladder contract")
}

fn read_fixture_src(id: &str) -> String {
    let path = load_ladder()
        .fixture_vox_path(&repo_root(), id)
        .unwrap_or_else(|| panic!("ladder missing fixture {id}"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn check_typecheck_clean(
    src: &str,
    hir: &mut vox_compiler::hir::HirModule,
    label: &str,
) -> Result<(), String> {
    let diags = typecheck_hir_module(src, hir);
    let errors: Vec<String> = diags
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .map(|d| {
            if let Some(code) = &d.code {
                format!("[{code}] {}", d.message)
            } else {
                d.message.clone()
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("{label} typecheck errors:\n{}", errors.join("\n")))
    }
}

fn assert_typecheck_clean(src: &str, hir: &mut vox_compiler::hir::HirModule, label: &str) {
    check_typecheck_clean(src, hir, label).unwrap_or_else(|e| panic!("{e}"));
}

fn lower_and_typecheck(src: &str) -> vox_compiler::hir::HirModule {
    let module = parse_script(lex(src)).expect("parse ladder fixture");
    let mut hir = lower_module(&module);
    assert_typecheck_clean(src, &mut hir, "ladder fixture");
    hir
}

fn compile_rust_script(id: &str, src: &str) -> Result<(), String> {
    let module = parse_script(lex(src)).map_err(|e| format!("parse: {e:?}"))?;
    let mut hir = lower_module(&module);
    check_typecheck_clean(src, &mut hir, &format!("ladder {id}"))?;
    let runtime_path = repo_root().join("crates/vox-actor-runtime");
    let package_name = format!("vox-script-{}", id.replace('_', "-"));
    let output = generate_script(&hir, &package_name, Some(&runtime_path))
        .map_err(|e| format!("codegen: {e}"))?;

    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    output
        .write_to_dir(dir.path())
        .map_err(|e| format!("write_to_dir: {e}"))?;
    inject_workspace_patches(dir.path());

    let _guard = COMPILE_LOCK.lock().unwrap();
    let target_dir = std::env::temp_dir().join("vox-emit-harness-target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .current_dir(dir.path())
        .args(["build", "--config", "build.rustc-wrapper=\"\""])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env_remove("RUSTC_WRAPPER")
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn inject_workspace_patches(project_dir: &Path) {
    let cargo_path = project_dir.join("Cargo.toml");
    let Ok(mut toml) = std::fs::read_to_string(&cargo_path) else {
        return;
    };
    const CANON_TURSO: &str =
        "turso = { version = \"0.6\", default-features = false, features = [\"sync\"] }";
    if toml.contains("vox-db") {
        if let Some(start) = toml.find("turso = {") {
            if let Some(line_end) = toml[start..].find('\n') {
                let end = start + line_end;
                toml.replace_range(start..end, CANON_TURSO);
            }
        } else if let Some(idx) = toml.find("[dependencies]") {
            let insert_at = toml[idx..]
                .find('\n')
                .map(|off| idx + off + 1)
                .unwrap_or(toml.len());
            toml.insert_str(insert_at, &format!("{CANON_TURSO}\n"));
        }
    }
    let aegis_path = repo_root()
        .join("patches/aegis-0.9.8")
        .to_string_lossy()
        .replace('\\', "/");
    if !toml.contains("[patch.crates-io]") {
        toml.push_str(&format!(
            "\n[patch.crates-io]\naegis = {{ path = \"{aegis_path}\" }}\n"
        ));
    } else if !toml.contains("aegis = ") {
        toml = toml.replace(
            "[patch.crates-io]\n",
            &format!("[patch.crates-io]\naegis = {{ path = \"{aegis_path}\" }}\n"),
        );
    }
    let _ = std::fs::write(cargo_path, toml);
}

fn assert_ladder_rust_script(id: &str) {
    let src = read_fixture_src(id);
    if let Err(e) = compile_rust_script(id, &src) {
        panic!("ladder {id} rust-script compile failed:\n{e}");
    }
}

fn assert_ladder_typescript_profile(id: &str) {
    let src = read_fixture_src(id);
    let hir = lower_and_typecheck(&src);
    project_and_validate(&hir, Target::TypeScript)
        .unwrap_or_else(|hard| panic!("ladder {id} TS profile errors: {hard:?}"));
}

fn assert_ladder_interp(id: &str) {
    let src = read_fixture_src(id);
    lower_and_typecheck(&src);
}

#[test]
fn ladder_hello_compiles_as_rust_script() {
    assert_ladder_rust_script("hello");
}

#[test]
fn ladder_crud_api_compiles_as_rust_script() {
    assert_ladder_rust_script("crud_api");
}

#[test]
fn ladder_durable_workflow_real_compiles_as_rust_script() {
    assert_ladder_rust_script("durable_workflow_real");
}

#[test]
fn ladder_scheduled_tick_compiles_as_rust_script() {
    assert_ladder_rust_script("scheduled_tick");
}

#[test]
fn ladder_db_native_ir_compiles_as_rust_script() {
    assert_ladder_rust_script("db_native_ir");
}

#[test]
fn ladder_web_routing_fullstack_compiles_as_rust_script() {
    assert_ladder_rust_script("web_routing_fullstack");
}

#[test]
fn ladder_auth_patterns_compiles_as_rust_script() {
    assert_ladder_rust_script("auth_patterns");
}

#[test]
fn ladder_mcp_tools_compiles_as_rust_script() {
    assert_ladder_rust_script("mcp_tools");
}

#[test]
fn ladder_error_propagation_compiles_as_rust_script() {
    assert_ladder_rust_script("error_propagation");
}

#[test]
fn ladder_json_as_typed_compiles_as_rust_script() {
    assert_ladder_rust_script("json_as_typed");
}

#[test]
fn ladder_crud_api_passes_typescript_emission_profile() {
    assert_ladder_typescript_profile("crud_api");
}

#[test]
fn ladder_reactive_counter_passes_typescript_emission_profile() {
    assert_ladder_typescript_profile("reactive_counter");
}

#[test]
fn ladder_dashboard_ui_passes_typescript_emission_profile() {
    assert_ladder_typescript_profile("dashboard_ui");
}

#[test]
fn ladder_web_routing_fullstack_passes_typescript_emission_profile() {
    assert_ladder_typescript_profile("web_routing_fullstack");
}

#[test]
fn ladder_hello_typechecks_for_interp() {
    assert_ladder_interp("hello");
}

#[test]
fn ladder_crud_api_typechecks_for_interp() {
    assert_ladder_interp("crud_api");
}

#[test]
fn ladder_error_propagation_typechecks_for_interp() {
    assert_ladder_interp("error_propagation");
}

#[test]
fn assert_typecheck_clean_rejects_obvious_type_error() {
    let bad = "fn broken() to int {\n  let x: int = \"not an int\"\n  return x\n}\n";
    let module = parse_script(lex(bad)).expect("self-test source must parse");
    let mut hir = lower_module(&module);
    assert!(
        check_typecheck_clean(bad, &mut hir, "<self-test>").is_err(),
        "assert_typecheck_clean must fail on error-severity diagnostics"
    );
}

#[test]
fn ladder_contract_drives_each_fixture_target() {
    let ladder = load_ladder();
    assert!(
        !ladder.fixtures.is_empty(),
        "canonical ladder must list fixtures"
    );
    for fixture in &ladder.fixtures {
        assert!(
            !fixture.targets.is_empty(),
            "fixture `{}` must declare at least one target",
            fixture.id
        );
        for target in &fixture.targets {
            match target.as_str() {
                "interp" => assert_ladder_interp(&fixture.id),
                "rust-script" => assert_ladder_rust_script(&fixture.id),
                "typescript" => assert_ladder_typescript_profile(&fixture.id),
                other => panic!(
                    "unknown ladder target `{other}` for fixture `{}`",
                    fixture.id
                ),
            }
        }
    }
}
