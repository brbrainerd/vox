//! MCP `vox_memory_search` envelope shape — minimal JSON deserialization.
//!
//! Filter with **`cargo test -p vox-integration-tests memory_retrieval_envelope`** (substring matches the test name below).

use serde_json::json;
use vox_orchestrator_mcp::memory_tools::{RetrievalEvidenceEnvelope, RetrievalTriggerMode};

#[test]
fn memory_retrieval_envelope_deserializes_minimal_tool_shape() {
    let v = json!({
        "trigger": "explicit_tool_query",
        "retrieval_tier": "hybrid",
        "memory_hit_count": 0,
        "knowledge_hit_count": 0,
        "used_vector": false,
        "used_bm25": true,
        "used_lexical_fallback": false,
        "contradiction_count": 0,
        "top_score": null,
        "search_plan": {},
        "search_diagnostics": {},
    });

    let env: RetrievalEvidenceEnvelope = serde_json::from_value(v).expect("deserialize");
    assert_eq!(env.chunk_hit_count, 0);
    assert_eq!(env.rrf_fused_hit_count, 0);
    assert_eq!(env.trigger, RetrievalTriggerMode::ExplicitToolQuery);
}
