//! Integration tests for vox-graphify-reader (BFS, path, compare).

use vox_graphify_reader::{GraphifyReader, GraphifyReaderError};

fn three_node_graph() -> serde_json::Value {
    serde_json::json!({
        "nodes": [
            {"id": "a", "label": "alpha node", "community": "c1"},
            {"id": "b", "label": "beta node",  "community": "c1"},
            {"id": "c", "label": "gamma node", "community": "c2"},
        ],
        "links": [
            {"source": "a", "target": "b"},
            {"source": "b", "target": "c"},
        ]
    })
}

#[test]
fn reader_loads_node_and_edge_counts() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    assert_eq!(g.node_count(), 3);
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn bfs_depth_1_returns_direct_neighbors_only() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    let hits = g.bfs_from_seeds(&["a"], 1, 100);
    let ids: Vec<&str> = hits.iter().map(|h| h.node_id.as_str()).collect();
    assert!(ids.contains(&"b"), "b must be a depth-1 neighbor of a");
    assert!(
        !ids.contains(&"c"),
        "c is depth-2, must not appear at depth-1"
    );
}

#[test]
fn bfs_depth_2_reaches_indirect_neighbors() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    let hits = g.bfs_from_seeds(&["a"], 2, 100);
    let ids: Vec<&str> = hits.iter().map(|h| h.node_id.as_str()).collect();
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"c"));
}

#[test]
fn bfs_hit_has_correct_depth_field() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    let hits = g.bfs_from_seeds(&["a"], 2, 100);
    let b_hit = hits
        .iter()
        .find(|h| h.node_id == "b")
        .expect("b must be in hits");
    let c_hit = hits
        .iter()
        .find(|h| h.node_id == "c")
        .expect("c must be in hits");
    assert_eq!(b_hit.depth, 1);
    assert_eq!(c_hit.depth, 2);
}

#[test]
fn bfs_respects_limit() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    let hits = g.bfs_from_seeds(&["a"], 5, 1);
    assert_eq!(hits.len(), 1, "limit=1 must cap results");
}

#[test]
fn bfs_unknown_seed_returns_empty() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    assert!(g.bfs_from_seeds(&["nonexistent"], 2, 100).is_empty());
}

#[test]
fn bfs_path_field_traces_from_seed_to_hit() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    let hits = g.bfs_from_seeds(&["a"], 2, 100);
    let c_hit = hits
        .iter()
        .find(|h| h.node_id == "c")
        .expect("c must be in hits");
    assert_eq!(c_hit.path, vec!["a", "b", "c"]);
}

#[test]
fn shortest_path_two_hops() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    assert_eq!(g.shortest_path("a", "c").unwrap(), vec!["a", "b", "c"]);
}

#[test]
fn shortest_path_same_node_is_single_element() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    assert_eq!(g.shortest_path("a", "a").unwrap(), vec!["a"]);
}

#[test]
fn shortest_path_unreachable_returns_none() {
    let graph = serde_json::json!({"nodes": [{"id": "x"}, {"id": "y"}], "links": []});
    let g = GraphifyReader::from_value(graph).unwrap();
    assert!(g.shortest_path("x", "y").is_none());
}

#[test]
fn god_nodes_orders_by_degree_descending() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    // b connects to a and c (degree 2); a and c have degree 1
    let gods = g.god_nodes(3);
    assert_eq!(gods[0].0, "b", "b must be the highest-degree node");
    assert_eq!(gods[0].1, 2);
}

#[test]
fn community_members_returns_correct_group() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    let mut members = g.community_members("c1");
    members.sort(); // sort for determinism
    assert_eq!(members, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn community_members_unknown_community_returns_empty() {
    let g = GraphifyReader::from_value(three_node_graph()).unwrap();
    assert!(g.community_members("nonexistent").is_empty());
}

#[test]
fn compare_diff_manifests_computes_deltas() {
    use vox_graphify_reader::compare::{ManifestSummary, diff_manifests};
    let old = ManifestSummary {
        node_count: 100,
        edge_count: 50,
        community_count: 5,
    };
    let new = ManifestSummary {
        node_count: 120,
        edge_count: 60,
        community_count: 7,
    };
    let diff = diff_manifests(&old, &new);
    assert_eq!(diff.node_delta, 20);
    assert_eq!(diff.edge_delta, 10);
    assert_eq!(diff.community_delta, 2);
}

#[test]
fn compare_negative_delta_when_graph_shrinks() {
    use vox_graphify_reader::compare::{ManifestSummary, diff_manifests};
    let old = ManifestSummary {
        node_count: 200,
        edge_count: 100,
        community_count: 10,
    };
    let new = ManifestSummary {
        node_count: 150,
        edge_count: 80,
        community_count: 8,
    };
    let diff = diff_manifests(&old, &new);
    assert_eq!(diff.node_delta, -50);
    assert_eq!(diff.edge_delta, -20);
    assert_eq!(diff.community_delta, -2);
}

#[test]
fn reader_errors_on_missing_nodes_key() {
    let bad = serde_json::json!({"links": []});
    let err = GraphifyReader::from_value(bad).unwrap_err();
    assert!(matches!(err, GraphifyReaderError::MissingNodes));
}

#[test]
fn reader_accepts_edges_key_as_alias_for_links() {
    let graph = serde_json::json!({
        "nodes": [{"id": "x"}, {"id": "y"}],
        "edges": [{"source": "x", "target": "y"}]
    });
    let g = GraphifyReader::from_value(graph).unwrap();
    assert_eq!(g.edge_count(), 1);
}

#[test]
fn reader_deduplicates_bidirectional_and_duplicate_links() {
    let graph = serde_json::json!({
        "nodes": [{"id": "x"}, {"id": "y"}],
        "links": [
            {"source": "x", "target": "y"},
            {"source": "y", "target": "x"},
            {"source": "x", "target": "y"}
        ]
    });
    let g = GraphifyReader::from_value(graph).unwrap();
    assert_eq!(g.edge_count(), 1);
}
