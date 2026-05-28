use vox_plugin_catalog::{ResolveError, bundle_resolved};

#[test]
fn vox_base_resolves_to_zero_plugins() {
    let plugins = bundle_resolved("vox-base").expect("should resolve");
    assert!(plugins.is_empty());
}

#[test]
fn vox_fullstack_resolves_to_nine_plugins() {
    // 8 skills + runtime-wasm = 9 total
    let plugins = bundle_resolved("vox-fullstack").expect("should resolve");
    assert_eq!(plugins.len(), 9);
    assert!(plugins.iter().any(|p| p.id == "skill-compiler"));
    assert!(plugins.iter().any(|p| p.id == "runtime-wasm"));
}

#[test]
fn vox_ml_resolves_through_extends_chain() {
    // vox-ml extends vox-fullstack (9) + mens-candle-cuda + nvml-probe = 11.
    let plugins = bundle_resolved("vox-ml").expect("should resolve");
    assert_eq!(plugins.len(), 11);
    assert!(plugins.iter().any(|p| p.id == "mens-candle-cuda"));
    assert!(plugins.iter().any(|p| p.id == "nvml-probe"));
    assert!(plugins.iter().any(|p| p.id == "skill-compiler"));
}

#[test]
fn unknown_bundle_returns_error() {
    match bundle_resolved("nope") {
        Err(ResolveError::UnknownBundle(id)) => assert_eq!(id, "nope"),
        other => panic!("expected UnknownBundle, got {other:?}"),
    }
}

#[test]
fn duplicate_plugin_in_chain_is_deduped() {
    // skill-orchestrator is in vox-fullstack AND vox-mesh; vox-dev pulls both.
    // It must appear exactly once in the resolved set.
    let plugins = bundle_resolved("vox-dev").expect("should resolve");
    let count = plugins
        .iter()
        .filter(|p| p.id == "skill-orchestrator")
        .count();
    assert_eq!(
        count, 1,
        "skill-orchestrator should be deduped, got {count}"
    );
}
