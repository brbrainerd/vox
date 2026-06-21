use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Registry {
    components: Vec<Component>,
}

#[derive(Debug, Deserialize)]
struct Component {
    name: String,
}

#[test]
fn test_component_registry_sync() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // Manifest dir is crates/vox-codegen. Registry is at contracts/gui/component-registry.v1.json.
    let path = Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("contracts/gui/component-registry.v1.json");

    let content = std::fs::read_to_string(&path)
        .expect("Failed to read contracts/gui/component-registry.v1.json");
    let registry: Registry =
        serde_json::from_str(&content).expect("Failed to parse component-registry.v1.json");

    // SSOT for primitive tags — the same list the parser & lowerer recognize.
    let primitive_tags = vox_compiler::lowering_shared::primitive_tags::all_primitives();

    let registered: std::collections::HashSet<&str> = registry
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    // 1. Every canonical primitive tag MUST have a registry entry (drift guard).
    for tag in primitive_tags {
        assert!(
            registered.contains(tag),
            "primitive '{tag}' (from lowering_shared::primitive_tags) is not in component-registry.v1.json"
        );
    }

    // 2. Every registered component MUST be a real primitive tag (no stale rows).
    let canonical: std::collections::HashSet<&str> = primitive_tags.iter().copied().collect();
    for comp in &registry.components {
        assert!(
            canonical.contains(comp.name.as_str()),
            "registry component '{}' is not a canonical primitive tag",
            comp.name
        );
    }
}
