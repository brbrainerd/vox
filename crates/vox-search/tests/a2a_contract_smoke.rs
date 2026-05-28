//! Cross-module smoke for `a2a_contract` + `policy` clamping (`vox-search`).
//!
//! Verifies that the agent-to-agent retrieval request payload survives a
//! JSON round-trip with `serde_json` (transport-agnostic wire shape) and
//! that policy clamping helpers return values inside `[0.0, 1.0]` even
//! when source weights drift outside the legal interval.

use vox_search::{A2ARetrievalRequest, SEARCH_POLICY_DEFAULT_VERSION, SearchPolicy};

#[test]
fn a2a_retrieval_request_roundtrips_json() {
    let req = A2ARetrievalRequest {
        request_id: "req-int-1".into(),
        query: "how does the orchestrator dispatch dei methods?".into(),
        plan_hint: None,
        repository_id: "repo:vox".into(),
        session_id: Some("mcp:session:42".into()),
        policy_version: SEARCH_POLICY_DEFAULT_VERSION,
    };

    let wire = serde_json::to_string(&req).expect("serialize A2ARetrievalRequest");
    assert!(wire.contains("\"request_id\":\"req-int-1\""));
    assert!(wire.contains("\"policy_version\":1"));

    let back: A2ARetrievalRequest =
        serde_json::from_str(&wire).expect("deserialize A2ARetrievalRequest");
    assert_eq!(back.request_id, req.request_id);
    assert_eq!(back.query, req.query);
    assert_eq!(back.repository_id, req.repository_id);
    assert_eq!(back.session_id, req.session_id);
    assert_eq!(back.policy_version, req.policy_version);
    assert!(back.plan_hint.is_none());
}

#[test]
fn search_policy_clamps_out_of_range_fusion_weights() {
    let p = SearchPolicy {
        memory_vector_fusion_weight: -0.4,
        chunk_vector_fusion_weight: 1.7,
        ..SearchPolicy::default()
    };

    let m = p.clamped_memory_vector_weight();
    let c = p.clamped_chunk_vector_weight();
    assert!((0.0..=1.0).contains(&m), "memory weight clamp: {m}");
    assert!((0.0..=1.0).contains(&c), "chunk weight clamp: {c}");
    assert!((m - 0.0).abs() < f32::EPSILON);
    assert!((c - 1.0).abs() < f32::EPSILON);
}
