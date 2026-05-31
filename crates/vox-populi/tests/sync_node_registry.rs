//! Integration test for `PopuliHttpClient::sync_node_registry`: fetch the
//! control-plane node list (mocked via wiremock), merge into a tempfile-backed
//! LocalRegistry (control-plane-wins on conflict, local-only kept, fresher-local
//! retained), and persist. Fully headless — no live control plane.
#![cfg(feature = "transport")]

use tempfile::tempdir;
use vox_populi::http_client::PopuliHttpClient;
use vox_populi::{LocalRegistry, PopuliRegistryFile};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn node(id: &str, last_seen: u64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "capabilities": {},
        "version": "t",
        "last_seen_unix_ms": last_seen,
    })
}

#[tokio::test]
async fn sync_node_registry_merges_control_plane_truth_and_persists() {
    let server = MockServer::start().await;

    // Control plane truth: conflict is FRESHER (5000), remote-only is new (3000),
    // stale-remote is OLDER than the local copy (2000).
    let control_plane = serde_json::json!({
        "schema_version": 2,
        "nodes": [node("conflict", 5000), node("remote-only", 3000), node("stale-remote", 2000)],
        "queue_depth": 7,
    });
    Mock::given(method("GET"))
        .and(path("/v1/populi/nodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(control_plane))
        .mount(&server)
        .await;

    let dir = tempdir().expect("tempdir");
    let reg = LocalRegistry::new(dir.path().join("local-registry.json"));

    // Pre-seed local: local-only (kept), conflict@1000 (older → loses), stale-remote@9000
    // (fresher than the control-plane copy → local retained).
    let local: PopuliRegistryFile = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "nodes": [node("local-only", 1000), node("conflict", 1000), node("stale-remote", 9000)],
        "queue_depth": 2,
    }))
    .expect("seed local registry");
    reg.save(&local).expect("save seed");

    let client = PopuliHttpClient::new(server.uri());
    let merged = client.sync_node_registry(&reg).await.expect("sync");

    // The returned merge must equal what was persisted.
    let persisted = reg.load().expect("load persisted");
    assert_eq!(persisted.nodes.len(), merged.nodes.len());

    let last_seen = |id: &str| {
        persisted
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id} missing"))
            .last_seen_unix_ms
    };

    // Union of ids, deterministically sorted by id.
    let ids: Vec<&str> = persisted.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec!["conflict", "local-only", "remote-only", "stale-remote"]);

    assert_eq!(last_seen("conflict"), 5000, "control-plane fresher wins the conflict");
    assert_eq!(last_seen("local-only"), 1000, "local-only node is kept");
    assert_eq!(last_seen("remote-only"), 3000, "control-plane-only node is added");
    assert_eq!(last_seen("stale-remote"), 9000, "strictly-fresher local copy is retained");
    assert_eq!(persisted.schema_version, 2, "schema_version = max(local, incoming)");
    assert_eq!(persisted.queue_depth, Some(7), "queue_depth comes from the live control plane");
}
