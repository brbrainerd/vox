use vox_graphify_reader::cluster::{ClusterEdge, ClusterNode, cluster_nodes};

#[test]
fn cluster_nodes_is_deterministic() {
    let nodes: Vec<ClusterNode> = ["a", "b", "c", "d", "e", "f"]
        .iter()
        .map(|s| ClusterNode {
            id: s.to_string(),
            label: s.to_string(),
        })
        .collect();
    let edges = vec![
        ("a", "b"),
        ("b", "c"),
        ("a", "c"), // triangle
        ("d", "e"),
        ("e", "f"),
        ("d", "f"), // triangle
        ("c", "d"), // bridge
    ]
    .into_iter()
    .map(|(s, t)| ClusterEdge {
        source: s.into(),
        target: t.into(),
    })
    .collect::<Vec<_>>();
    let r1 = cluster_nodes(&nodes, &edges);
    let r2 = cluster_nodes(&nodes, &edges);
    assert_eq!(r1, r2, "cluster_nodes must be deterministic across runs");
}

#[test]
fn test_leiden_clustering_success() {
    let nodes = vec![
        ClusterNode {
            id: "A".to_string(),
            label: "A".to_string(),
        },
        ClusterNode {
            id: "B".to_string(),
            label: "B".to_string(),
        },
        ClusterNode {
            id: "C".to_string(),
            label: "C".to_string(),
        },
        ClusterNode {
            id: "D".to_string(),
            label: "D".to_string(),
        },
    ];
    let edges = vec![
        ClusterEdge {
            source: "A".to_string(),
            target: "B".to_string(),
        },
        ClusterEdge {
            source: "B".to_string(),
            target: "C".to_string(),
        },
        ClusterEdge {
            source: "C".to_string(),
            target: "D".to_string(),
        },
    ];
    let communities = cluster_nodes(&nodes, &edges);
    assert_eq!(communities.len(), 4);
    assert!(communities.contains_key("A"));
    assert!(communities.contains_key("D"));
}
