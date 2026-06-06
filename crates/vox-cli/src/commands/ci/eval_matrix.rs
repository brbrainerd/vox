//! Verify `contracts/eval/benchmark-matrix.json` against
//! `contracts/eval/benchmark-matrix.schema.json` (JSON Schema `$id`:
//! `https://voxlang.org/schemas/eval/benchmark-matrix.schema.json`; M5 / WS11–WS12).
//!
//! [`run_executions`] maps each `benchmark_classes` entry to a concrete `cargo` invocation.
//! [`BENCHMARK_CLASS_IDS`] must match the schema `enum` for `benchmark_classes.items` and
//! must stay in sync with [`run_benchmark_class`].

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use vox_bounded_fs::read_utf8_path_capped;

const SCHEMA_REL: &str = "contracts/eval/benchmark-matrix.schema.json";
const DATA_REL: &str = "contracts/eval/benchmark-matrix.json";

// --- Crate / feature names (avoid scattered literals in cargo argv) ---
const PKG_VOX_CLI: &str = "vox-cli";
const PKG_VOX_POPULI_MENS: &str = "vox-populi";
const PKG_VOX_MCP: &str = "vox-mcp";
const PKG_VOX_RUNTIME: &str = "vox-actor-runtime";
const PKG_VOX_ORCHESTRATOR: &str = "vox-orchestrator";
const PKG_VOX_DOC_INVENTORY: &str = "vox-doc-inventory";
const PKG_VOX_COMPILER: &str = "vox-compiler";
const FEAT_GPU: &str = "gpu";
const FEAT_POPULI_MENS_HF_HUB: &str = "mens,mens-hf-hub";

// --- Cargo test name filters (module paths or substring filters) ---
const FILTER_MENS_GPU_TESTS: &str = "commands::mens::tests";
const FILTER_MCP_ROUTE_TESTS: &str = "llm_bridge::model_route_policy::tests::mcp_";
const FILTER_RUNTIME_MODEL_RESOLUTION: &str = "model_resolution";
const FILTER_DISPATCH_PROTOCOL: &str = "dispatch_protocol::tests";
const FILTER_ORCH_A2A_MESSAGE_IDS: &str = "message_ids_strictly_increasing";
const FILTER_MCP_LANGUAGE_SURFACE: &str = "introspection_tools::tests::language_surface";
const FILTER_MCP_TOOL_DISPATCH: &str = "test_mcp_tool_dispatch";
const FILTER_DOC_INVENTORY_RELEVANCE: &str = "relevance_score";
const FILTER_MENS_HUB_TESTS: &str = "mens::hub::tests";
const FILTER_COMPILER_SYNTAX_K_GOLDEN: &str = "all_golden_vox_examples_parse_and_lower";
const FILTER_COMPILER_SYNTAX_K_PARITY: &str = "syntax_k_artifact_for_parity_chain";
const FILTER_COMPILER_SYNTAX_K_GATE: &str = "syntax_k_regression_gate_observe_only";
const FILTER_CONTEXT_ENVELOPE_ORCH: &str =
    "context_envelope_projection_validates_against_contract_schema";
const FILTER_CONTEXT_ENVELOPE_MCP: &str =
    "retrieval_evidence_projection_validates_against_context_envelope_schema";
const FILTER_AGENT_HARNESS_CONTRACT: &str =
    "agent_harness_projection_validates_against_contract_schema";
const FILTER_CONTEXT_LIFECYCLE_TELEMETRY_FIXTURES: &str =
    "context_lifecycle_telemetry_fixtures_validate_against_schema";

/// Every `benchmark_classes` id in the matrix + JSON Schema enum; sorted lexicographically.
pub(crate) const BENCHMARK_CLASS_IDS: &[&str] = &[
    "contracts_eval_benchmark_matrix_schema",
    "vox_agent_harness_contract",
    "vox_ci_command_compliance",
    "vox_cli_dispatch_dei_schema",
    "vox_cli_gpu_check",
    "vox_cli_mens_gpu_tests",
    "vox_compiler_syntax_k_emit",
    "vox_compiler_syntax_k_regression_gate",
    "vox_compiler_syntax_k_webir",
    "vox_context_envelope_contract",
    "vox_doc_inventory_relevance_score",
    "vox_mcp_introspection_language_surface",
    "vox_mcp_route_telemetry_parity",
    "vox_mcp_tool_dispatch_smoke",
    "vox_mens_hub_token_resolution",
    "vox_orchestrator_a2a_message_ids",
    "vox_runtime_model_resolution_tests",
];

#[derive(Debug, Deserialize)]
struct MatrixFile {
    milestones: Vec<Milestone>,
}

#[derive(Debug, Deserialize)]
struct Milestone {
    id: String,
    benchmark_classes: Vec<String>,
}

/// Validate the committed benchmark matrix against its JSON Schema.
pub fn run_verify(repo_root: &Path) -> Result<()> {
    let schema_path = repo_root.join(SCHEMA_REL);
    let data_path = repo_root.join(DATA_REL);
    let schema_src = read_utf8_path_capped(&schema_path)?;
    let data_src = read_utf8_path_capped(&data_path)?;
    let schema_val: serde_json::Value = serde_json::from_str(&schema_src)
        .with_context(|| format!("parse {}", schema_path.display()))?;
    let data_val: serde_json::Value = serde_json::from_str(&data_src)
        .with_context(|| format!("parse {}", data_path.display()))?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .with_context(|| format!("compile JSON Schema {}", schema_path.display()))?;
    vox_jsonschema_util::validate(
        &data_val,
        &validator,
        format!("{} vs {}", data_path.display(), schema_path.display()),
    )?;
    println!(
        "OK: {} matches {}",
        data_path.display(),
        schema_path.display()
    );
    Ok(())
}

/// Run cargo workflows for each unique `benchmark_classes` id (optionally one milestone).
pub fn run_executions(repo_root: &Path, milestone_filter: Option<&str>) -> Result<()> {
    run_verify(repo_root)?;
    let data_path = repo_root.join(DATA_REL);
    let raw = read_utf8_path_capped(&data_path)?;
    let m: MatrixFile =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", data_path.display()))?;

    let milestones: Vec<&Milestone> = if let Some(mid) = milestone_filter {
        m.milestones
            .iter()
            .filter(|x| x.id == mid)
            .collect::<Vec<_>>()
    } else {
        m.milestones.iter().collect()
    };

    if milestones.is_empty() {
        anyhow::bail!("no milestones matched filter {:?}", milestone_filter);
    }

    let mut seen = HashSet::<String>::new();
    for ms in milestones {
        println!("— milestone {} —", ms.id);
        for class in &ms.benchmark_classes {
            if seen.contains(class) {
                continue;
            }
            println!("  class: {class}");
            run_benchmark_class(repo_root, class)?;
            if class.starts_with("vox_compiler_syntax_k_") {
                crate::benchmark_telemetry::record_syntax_k_opt_blocking(
                    class,
                    "eval-matrix",
                    None,
                    Some(serde_json::json!({
                        "status": "ok",
                        "runner": "vox ci eval-matrix"
                    })),
                );
            }
            seen.insert(class.clone());
        }
    }
    println!("eval-matrix run: OK ({} unique classes)", seen.len());
    Ok(())
}

fn run_cargo(repo_root: &Path, args: &[&str]) -> Result<()> {
    let cargo = super::cargo_bin();
    let st = Command::new(&cargo)
        .current_dir(repo_root)
        .args(args)
        .status()
        .with_context(|| format!("spawn {} {}", cargo.display(), args.join(" ")))?;
    if !st.success() {
        anyhow::bail!(
            "cargo {} failed with status {:?}",
            args.join(" "),
            st.code()
        );
    }
    Ok(())
}

/// `cargo test … -- --nocapture`
fn cargo_test_nocapture(repo_root: &Path, head: &[&str]) -> Result<()> {
    let mut args: Vec<&str> = head.to_vec();
    args.extend_from_slice(&["--", "--nocapture"]);
    run_cargo(repo_root, &args)
}

fn cargo_check_pkg_features(repo_root: &Path, package: &str, features: Option<&str>) -> Result<()> {
    match features {
        None => run_cargo(repo_root, &["check", "-p", package]),
        Some(f) => run_cargo(repo_root, &["check", "-p", package, "--features", f]),
    }
}

fn run_benchmark_class(repo_root: &Path, class: &str) -> Result<()> {
    match class {
        "vox_cli_gpu_check" => cargo_check_pkg_features(repo_root, PKG_VOX_CLI, Some(FEAT_GPU)),
        "vox_cli_mens_gpu_tests" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_CLI,
                "--features",
                FEAT_GPU,
                FILTER_MENS_GPU_TESTS,
            ],
        ),
        "vox_mens_hub_token_resolution" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_POPULI_MENS,
                "--lib",
                "--features",
                FEAT_POPULI_MENS_HF_HUB,
                FILTER_MENS_HUB_TESTS,
            ],
        ),
        "vox_mcp_route_telemetry_parity" => cargo_test_nocapture(
            repo_root,
            &["test", "-p", PKG_VOX_MCP, FILTER_MCP_ROUTE_TESTS],
        ),
        "vox_runtime_model_resolution_tests" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_RUNTIME,
                "--lib",
                FILTER_RUNTIME_MODEL_RESOLUTION,
            ],
        ),
        "vox_cli_dispatch_dei_schema" => cargo_test_nocapture(
            repo_root,
            &["test", "-p", PKG_VOX_CLI, "--lib", FILTER_DISPATCH_PROTOCOL],
        ),
        "vox_orchestrator_a2a_message_ids" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_ORCHESTRATOR,
                FILTER_ORCH_A2A_MESSAGE_IDS,
            ],
        ),
        "vox_agent_harness_contract" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_ORCHESTRATOR,
                "--test",
                "agent_harness_contract",
                FILTER_AGENT_HARNESS_CONTRACT,
            ],
        ),
        "vox_context_envelope_contract" => {
            cargo_test_nocapture(
                repo_root,
                &[
                    "test",
                    "-p",
                    PKG_VOX_ORCHESTRATOR,
                    "--test",
                    "context_envelope_contract",
                    FILTER_CONTEXT_ENVELOPE_ORCH,
                ],
            )?;
            cargo_test_nocapture(
                repo_root,
                &[
                    "test",
                    "-p",
                    PKG_VOX_ORCHESTRATOR,
                    "--test",
                    "context_lifecycle_telemetry_fixtures",
                    FILTER_CONTEXT_LIFECYCLE_TELEMETRY_FIXTURES,
                ],
            )?;
            cargo_test_nocapture(
                repo_root,
                &[
                    "test",
                    "-p",
                    PKG_VOX_MCP,
                    "--test",
                    "retrieval_evidence_context_envelope_contract",
                    FILTER_CONTEXT_ENVELOPE_MCP,
                ],
            )
        }
        "vox_mcp_introspection_language_surface" => cargo_test_nocapture(
            repo_root,
            &["test", "-p", PKG_VOX_MCP, FILTER_MCP_LANGUAGE_SURFACE],
        ),
        "vox_mcp_tool_dispatch_smoke" => cargo_test_nocapture(
            repo_root,
            &["test", "-p", PKG_VOX_MCP, FILTER_MCP_TOOL_DISPATCH],
        ),
        "vox_doc_inventory_relevance_score" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_DOC_INVENTORY,
                FILTER_DOC_INVENTORY_RELEVANCE,
            ],
        ),
        "vox_compiler_syntax_k_webir" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_COMPILER,
                FILTER_COMPILER_SYNTAX_K_GOLDEN,
            ],
        ),
        "vox_compiler_syntax_k_emit" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_COMPILER,
                FILTER_COMPILER_SYNTAX_K_PARITY,
            ],
        ),
        "vox_compiler_syntax_k_regression_gate" => cargo_test_nocapture(
            repo_root,
            &[
                "test",
                "-p",
                PKG_VOX_COMPILER,
                FILTER_COMPILER_SYNTAX_K_GATE,
            ],
        ),
        "contracts_eval_benchmark_matrix_schema" => run_verify(repo_root),
        // In-process: avoids `cargo run -p vox-cli` replacing `vox.exe` (Windows file-lock errors).
        "vox_ci_command_compliance" => super::command_compliance::run(repo_root),
        other => anyhow::bail!(
            "unknown benchmark_class {:?} — known ids: {}; add mapping + schema enum in {}",
            other,
            BENCHMARK_CLASS_IDS.join(", "),
            file!()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("vox-cli crate lives in crates/vox-cli")
            .to_path_buf()
    }

    #[test]
    fn benchmark_class_ids_sorted_and_unique() {
        let mut prev = "";
        for &id in BENCHMARK_CLASS_IDS {
            assert!(
                id > prev,
                "BENCHMARK_CLASS_IDS must be sorted, violation: {id} after {prev}"
            );
            prev = id;
        }
        let mut set = HashSet::new();
        for &id in BENCHMARK_CLASS_IDS {
            assert!(set.insert(id), "duplicate id: {id}");
        }
    }

    #[test]
    fn matrix_json_classes_match_schema_enum_and_rust_ssot() {
        let root = repo_root();
        let data_path = root.join(DATA_REL);
        let raw = read_utf8_path_capped(&data_path).expect("read benchmark-matrix.json");
        let m: MatrixFile = serde_json::from_str(&raw).expect("parse benchmark-matrix.json");

        let mut from_matrix = HashSet::new();
        for ms in &m.milestones {
            for c in &ms.benchmark_classes {
                from_matrix.insert(c.as_str());
            }
        }

        let ssot: HashSet<&str> = BENCHMARK_CLASS_IDS.iter().copied().collect();
        assert_eq!(
            from_matrix, ssot,
            "every benchmark class must appear exactly once across the union of milestones \
             (matrix defines the full set; add/remove in benchmark-matrix.json + schema + BENCHMARK_CLASS_IDS)"
        );

        let schema_path = root.join(SCHEMA_REL);
        let schema_src = read_utf8_path_capped(&schema_path).expect("read schema");
        let schema_val: serde_json::Value =
            serde_json::from_str(&schema_src).expect("parse schema");
        let enum_vals = schema_val
            .pointer("/properties/milestones/items/properties/benchmark_classes/items/enum")
            .and_then(|v| v.as_array())
            .expect("schema must define benchmark_classes.items.enum");
        let from_schema: HashSet<String> = enum_vals
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        let ssot_strings: HashSet<String> = BENCHMARK_CLASS_IDS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            from_schema, ssot_strings,
            "JSON Schema enum for benchmark_classes must match BENCHMARK_CLASS_IDS"
        );
    }
}
