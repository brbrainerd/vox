use vox_graphify_reader::cluster::{cluster_nodes, ClusterEdge, ClusterNode};

#[test]
fn test_leiden_clustering_success() {
    let nodes = vec![
        ClusterNode { id: "A".to_string(), label: "A".to_string() },
        ClusterNode { id: "B".to_string(), label: "B".to_string() },
        ClusterNode { id: "C".to_string(), label: "C".to_string() },
        ClusterNode { id: "D".to_string(), label: "D".to_string() },
    ];
    let edges = vec![
        ClusterEdge { source: "A".to_string(), target: "B".to_string() },
        ClusterEdge { source: "B".to_string(), target: "C".to_string() },
        ClusterEdge { source: "C".to_string(), target: "D".to_string() },
    ];
    let communities = cluster_nodes(&nodes, &edges);
    assert_eq!(communities.len(), 4);
    assert!(communities.contains_key("A"));
    assert!(communities.contains_key("D"));
}
