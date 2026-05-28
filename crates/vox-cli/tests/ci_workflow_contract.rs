//! Regression: GitHub Actions CI must stay on `vox ci` / `cargo run -p vox-cli` guards
//! (hybrid migration — do not reintroduce Python doc-inventory or raw bash matrices).

#[test]
fn github_ci_doc_inventory_is_rust() {
    let yml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/ci.yml"
    ));
    assert!(
        yml.contains("ci command-compliance") || yml.contains("ci ssot-drift"),
        "ci.yml should run `vox ci command-compliance` or `vox ci ssot-drift` (bundles command-compliance via run_ssot_drift)"
    );
    assert!(
        yml.contains("ci doc-inventory verify"),
        "ci.yml should verify inventory via `vox ci doc-inventory verify`"
    );
    assert!(
        !yml.contains("verify_doc_inventory_fresh.py"),
        "retired Python doc-inventory verifier must not return to ci.yml"
    );
}

#[test]
fn github_ci_populi_gate_is_unified() {
    let yml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/ci.yml"
    ));
    assert!(
        yml.contains("ci mens-gate --profile ci_full"),
        "ci.yml should run a single Mens gate profile (ci_full)"
    );
    assert!(
        !yml.contains("populi_release_gate.sh"),
        "do not call populi_release_gate.sh from CI; use `vox ci mens-gate`"
    );
}

#[test]
fn github_ci_no_duplicate_mens_populi_gate_tests_after_manifest() {
    let yml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/ci.yml"
    ));
    if yml.contains("ci mens-gate --profile ci_full") {
        assert!(
            !yml.contains("--test qwen35_native_parity"),
            "qwen35_native_parity is in scripts/populi/gates.yaml (ci_full); do not re-invoke in ci.yml"
        );
        assert!(
            !yml.contains("qwen35_linear_attention_forward_and_cache_progression"),
            "qwen35_linear_attention tests are in gates.yaml (ci_full); do not duplicate in ci.yml"
        );
    }
}

#[test]
fn github_ci_runs_llvm_cov_and_coverage_gates() {
    let yml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/ci.yml"
    ));
    assert!(
        yml.contains("cargo llvm-cov nextest --workspace"),
        "ci.yml should run workspace tests under cargo-llvm-cov nextest (do not pass a bare `run` — it becomes a test filter)"
    );
    assert!(
        yml.contains("ci coverage-gates") && yml.contains("--mode enforce"),
        "ci.yml should run `vox ci coverage-gates --mode enforce` after llvm-cov JSON summary"
    );
    assert!(
        yml.contains("llvm-tools-preview"),
        "ci.yml Rust toolchain should include llvm-tools-preview for cargo-llvm-cov"
    );
}

#[test]
fn linux_ci_runs_workspace_tests_and_windows_stack_wrappers_stay_cfg_gated() {
    let ci = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/ci.yml"
    ));
    assert!(
        ci.contains("runs-on: [self-hosted, linux, x64]"),
        "ci.yml should keep the main test job on the Linux self-hosted runner"
    );
    assert!(
        ci.contains("cargo llvm-cov nextest --workspace")
            && ci.contains("cargo nextest run --workspace"),
        "Linux CI should execute the workspace test suite, including vox-cli integration tests"
    );

    let root_parsing = include_str!("vox_cli_root_parsing.rs");
    let catalog = include_str!("command_catalog_paths_baseline.rs");
    for source in [root_parsing, catalog] {
        assert!(
            source.contains("#[cfg(windows)]") && source.contains("#[cfg(not(windows))]"),
            "large-stack test helpers must remain Windows-only with direct non-Windows execution"
        );
    }
}

#[test]
fn compile_matrix_runs_compile_suite_workspace_smoke() {
    let yml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/compile-matrix.yml"
    ));
    assert!(
        yml.contains("examples/compile-suite"),
        "compile-matrix.yml should run from examples/compile-suite"
    );
    assert!(
        yml.contains("compile --workspace --target native-binary"),
        "compile-matrix.yml should smoke `vox compile --workspace --target native-binary`"
    );
    assert!(
        yml.contains("compile --target desktop"),
        "compile-matrix.yml should smoke desktop Tauri codegen via `vox compile --target desktop`"
    );
}

#[test]
fn command_registry_has_ci_retirement_audit() {
    let reg = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/cli/command-registry.yaml"
    ));
    assert!(
        reg.contains("retirement-audit"),
        "command-registry should list `vox ci retirement-audit`"
    );
}

#[test]
fn packaging_ssot_matches_workspace_compile_behavior() {
    let doc = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/src/architecture/vox-application-packaging-ssot-2026.md"
    ));
    assert!(
        doc.contains("builds every member package with the requested `--target`"),
        "packaging SSOT should match compile.rs workspace behavior"
    );
    assert!(
        !doc.contains("whose `[bundle]` / inferred target matches"),
        "packaging SSOT must not promise target filtering that compile.rs does not implement"
    );
}

#[test]
fn packaging_ssot_documents_compile_matrix_locked_windows_fallback() {
    let doc = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/src/architecture/vox-application-packaging-ssot-2026.md"
    ));
    assert!(
        doc.contains("run from the built `vox.exe`") && doc.contains("cargo run fails to relink"),
        "packaging SSOT should document the locked-Windows-binary compile-matrix fallback"
    );
}

#[test]
fn ml_workflow_grammar_drift_and_eval_stay_native() {
    let yml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.github/workflows/ml_data_extraction.yml"
    ));
    assert!(
        yml.contains("ci grammar-drift") && yml.contains("--emit github"),
        "ml_data_extraction.yml should detect grammar drift via `vox ci grammar-drift`"
    );
    assert!(
        yml.contains("corpus eval") && yml.contains("--print-summary"),
        "ml_data_extraction.yml should summarize eval via `vox corpus eval --print-summary`"
    );
    assert!(
        !yml.contains("python3 -c"),
        "do not use inline Python in ml_data_extraction.yml; use Vox/Rust CLI output"
    );
}
