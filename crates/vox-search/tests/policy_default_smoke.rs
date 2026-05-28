//! Offline policy defaults (`vox-search`).

use vox_search::{SEARCH_POLICY_DEFAULT_VERSION, SearchPolicy};

#[test]
fn default_search_policy_version_and_weights() {
    let p = SearchPolicy::default();
    assert_eq!(p.version, SEARCH_POLICY_DEFAULT_VERSION);
    assert!(p.memory_vector_fusion_weight > 0.0 && p.memory_vector_fusion_weight < 1.0);
    assert!(p.chunk_vector_fusion_weight > 0.0 && p.chunk_vector_fusion_weight <= 1.0);
    assert!(!p.repo_inventory_skip_dirs.is_empty());
}

#[test]
fn search_policy_roundtrips_json() {
    let p = SearchPolicy::default();
    let v = serde_json::to_value(&p).expect("serialize SearchPolicy");
    let back: SearchPolicy = serde_json::from_value(v).expect("deserialize SearchPolicy");
    assert_eq!(back.version, p.version);
    assert_eq!(
        back.memory_vector_fusion_weight,
        p.memory_vector_fusion_weight
    );
}
