/// Band B.1 — registry.v1.yaml must contain >= 20 orchestrator/* knobs.
///
/// This test parses contracts/config/registry.v1.yaml and asserts that the
/// HIGH-PRIORITY orchestrator env-var rows registered in Band B.1 are present.
#[test]
fn orchestrator_rows_registered() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("contracts/config/registry.v1.yaml");

    let content = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", manifest.display(), e));

    // Count knobs whose owner_crate is vox-orchestrator
    let orchestrator_count = content
        .lines()
        .filter(|l| l.trim().starts_with("owner_crate: vox-orchestrator"))
        .count();

    assert!(
        orchestrator_count >= 20,
        "expected >= 20 vox-orchestrator rows in registry.v1.yaml, found {}",
        orchestrator_count
    );
}
