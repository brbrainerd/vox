use std::path::Path;
use tempfile::tempdir;
use vox_graphify_reader::ast::{ExtractedGraph, ExtractedNode};
use vox_graphify_reader::cache::CacheManager;

#[test]
fn test_cache_management_cycle() {
    let tmp = tempdir().unwrap();
    let manager = CacheManager::new(tmp.path().to_path_buf());

    let file = Path::new("main.rs");
    let hash = "abc123hash";
    let graph = ExtractedGraph {
        nodes: vec![ExtractedNode {
            id: "a".to_string(),
            label: "a".to_string(),
            kind: "fn".to_string(),
        }],
        edges: vec![],
    };

    manager.write_cache(file, hash, &graph);
    let cached_hash = manager.get_cached_hash(file);
    assert_eq!(cached_hash.as_deref(), Some(hash));

    let loaded = manager.load_cache(file).unwrap();
    assert_eq!(loaded.nodes.len(), 1);
    assert_eq!(loaded.nodes[0].id, "a");
}
