//! Lexical graph search + knowledge-node projection (`vox-config::graphify`).

use serde_json::json;
use vox_config::graphify::{lexical_search_graph, project_graph_nodes_for_ingest};

fn sample_graph() -> serde_json::Value {
    json!({
        "nodes": [
            {"id": "auth", "label": "authentication module", "type": "module"},
            {"id": "db", "label": "database connection pool", "type": "service"},
            {"id": "authz", "label": "authentication authorization gateway", "type": "module"},
        ],
        "links": []
    })
}

#[test]
fn lexical_search_empty_graph_returns_no_hits() {
    let graph = json!({"nodes": [], "links": []});
    let hits = lexical_search_graph(&graph, "repo-code-graph", "authentication", 10);
    assert!(hits.is_empty());
}

#[test]
fn lexical_search_missing_nodes_returns_no_hits() {
    let graph = json!({"links": []});
    let hits = lexical_search_graph(&graph, "repo-code-graph", "authentication", 10);
    assert!(hits.is_empty());
}

#[test]
fn lexical_search_matching_query_returns_ranked_hits() {
    let graph = sample_graph();
    let hits = lexical_search_graph(
        &graph,
        "repo-code-graph",
        "authentication authorization",
        10,
    );
    assert!(!hits.is_empty());
    assert!(
        hits[0].score >= hits.last().unwrap().score,
        "hits must be sorted by descending score"
    );
    let top = &hits[0];
    assert_eq!(top.node_id, "authz");
    assert!(top.label.contains("authentication"));
    assert!(top.score >= 2, "authz should match both query tokens");
}

#[test]
fn lexical_search_respects_limit() {
    let graph = sample_graph();
    let hits = lexical_search_graph(&graph, "repo-code-graph", "authentication", 1);
    assert_eq!(hits.len(), 1);
}

#[test]
fn lexical_search_ignores_tokens_shorter_than_three_chars() {
    let graph = json!({
        "nodes": [
            {"id": "ab-node", "label": "ab only label"},
            {"id": "auth-node", "label": "authentication service"},
        ]
    });
    // "ab" is dropped (len <= 2); only "authentication" token matches.
    let hits = lexical_search_graph(&graph, "repo-code-graph", "ab authentication", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].node_id, "auth-node");
}

#[test]
fn lexical_search_uses_name_when_label_and_id_missing() {
    let graph = json!({
        "nodes": [
            {"name": "connection pool manager"},
            {"id": "ignored", "label": ""},
        ]
    });
    let hits = lexical_search_graph(&graph, "repo-code-graph", "connection pool", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].node_id, "connection pool manager");
    assert_eq!(hits[0].label, "connection pool manager");
}

#[test]
fn project_graph_nodes_for_ingest_uses_id_prefix() {
    let graph = sample_graph();
    let nodes = project_graph_nodes_for_ingest(&graph, "repo-code-graph");
    assert_eq!(nodes.len(), 3);
    for node in &nodes {
        assert!(
            node.id.starts_with("graphify:repo-code-graph:node:"),
            "unexpected id: {}",
            node.id
        );
    }
    let auth = nodes
        .iter()
        .find(|n| n.id.ends_with(":auth"))
        .expect("auth node");
    assert_eq!(auth.label, "authentication module");
    assert_eq!(auth.node_type, "module");
    assert!(auth.content.contains("authentication module"));
    assert!(auth.metadata.contains("repo-code-graph"));
    assert!(auth.metadata.contains("graphify_lexical_ingest"));
}

#[test]
fn project_graph_nodes_content_is_node_json() {
    let graph = json!({
        "nodes": [
            {
                "id": "n1",
                "label": "hello world",
                "description": "sample description"
            }
        ]
    });
    let nodes = project_graph_nodes_for_ingest(&graph, "test-corpus");
    assert_eq!(nodes.len(), 1);
    let parsed: serde_json::Value =
        serde_json::from_str(&nodes[0].content).expect("content must be valid JSON");
    assert_eq!(parsed.get("id").and_then(|v| v.as_str()), Some("n1"));
    assert_eq!(
        parsed.get("description").and_then(|v| v.as_str()),
        Some("sample description")
    );
}

#[test]
fn project_graph_nodes_default_type_when_absent() {
    let graph = json!({
        "nodes": [{"id": "n1", "label": "orphan node"}]
    });
    let nodes = project_graph_nodes_for_ingest(&graph, "vox-gui-surface");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node_type, "graph_node");
    assert_eq!(nodes[0].id, "graphify:vox-gui-surface:node:n1");
}
