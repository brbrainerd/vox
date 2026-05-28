//! Serializable payloads for agent-to-agent retrieval handoff (transport-agnostic).

use serde::{Deserialize, Serialize};

use vox_db::{SearchDiagnostics, SearchPlan};

use crate::execution::SearchExecution;

/// Agent asks peer or parent for grounded evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ARetrievalRequest {
    pub request_id: String,
    pub query: String,
    pub plan_hint: Option<SearchPlan>,
    pub repository_id: String,
    pub session_id: Option<String>,
    pub policy_version: u32,
}
/// Reference to a securely persisted retrieval artifact, replacing inline prompt injection risks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ADurableArtifact {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_ms: Option<u64>,
    pub chunk_count: usize,
}

/// Evidence package returned to the requester.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ARetrievalResponse {
    pub request_id: String,
    pub memory_excerpts: Vec<String>,
    pub knowledge_excerpts: Vec<String>,
    /// Document chunks (RAG corpus) plus Tantivy doc mirror and Qdrant ANN sidecar lines (same as MCP `chunk_lines` bundle).
    pub chunk_excerpts: Vec<String>,
    pub repo_paths: Vec<String>,
    /// Cross-corpus RRF ordering when enabled in policy (else empty).
    #[serde(default)]
    pub rrf_fused_excerpts: Vec<String>,
    /// Secure references to large evidence bodies, replacing inline text for high-volume results.
    #[serde(default)]
    pub durable_artifacts: Vec<A2ADurableArtifact>,
    pub diagnostics: SearchDiagnostics,
    pub from_node_id: Option<String>,
}

impl A2ARetrievalResponse {
    /// Maps a finished [`SearchExecution`] + diagnostics into the wire snapshot (request correlation only).
    #[must_use]
    pub fn from_search_pass(
        request_id: impl Into<String>,
        execution: &SearchExecution,
        diagnostics: SearchDiagnostics,
        from_node_id: Option<impl Into<String>>,
    ) -> Self {
        let mut chunk_excerpts = execution.chunk_lines.clone();
        chunk_excerpts.extend_from_slice(&execution.tantivy_doc_lines);
        chunk_excerpts.extend_from_slice(&execution.qdrant_lines);
        Self {
            request_id: request_id.into(),
            memory_excerpts: execution.memory_lines.clone(),
            knowledge_excerpts: execution.knowledge_lines.clone(),
            chunk_excerpts,
            repo_paths: execution.repo_lines.clone(),
            rrf_fused_excerpts: execution.rrf_fused_lines.clone(),
            durable_artifacts: execution
                .durable_artifacts
                .iter()
                .map(|da| A2ADurableArtifact {
                    uri: da.uri.clone(),
                    token: da.token.clone(),
                    expires_at_unix_ms: da.expires_at_unix_ms,
                    chunk_count: da.chunk_count,
                })
                .collect(),
            diagnostics,
            from_node_id: from_node_id.map(|s| s.into()),
        }
    }
}

/// Follow-up refinement (contradiction / weak recall) for shared critique loops.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ARetrievalRefinement {
    pub request_id: String,
    pub reason: String,
    pub rewritten_query: Option<String>,
    pub recommended_corpora: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::SearchDiagnostics;

    #[test]
    fn from_search_pass_mirrors_mcp_chunk_merging() {
        let execution = SearchExecution {
            memory_lines: Vec::new(),
            knowledge_lines: Vec::new(),
            chunk_lines: vec!["[chunk:a] x".into()],
            repo_lines: Vec::new(),
            tantivy_doc_lines: vec!["[tantivy:p] y".into()],
            qdrant_lines: vec!["[qdrant:9] z".into()],
            rrf_fused_lines: vec!["fused".into()],
            web_lines: vec![],
            symbol_proximity_lines: vec![],
            durable_artifacts: vec![],
            warnings: Vec::new(),
            used_vector: false,
            used_bm25: true,
            lexical_fallback_used: false,
            contradiction_count: 0,
            top_score: None,
            backend_mix: Vec::new(),
            source_diversity: 1,
            evidence_quality: 0.5,
            citation_coverage: 0.5,
            recommended_next_action: None,
        };
        let r = A2ARetrievalResponse::from_search_pass(
            "r1",
            &execution,
            SearchDiagnostics::default(),
            Some("node-a"),
        );
        assert_eq!(r.chunk_excerpts.len(), 3);
        assert_eq!(r.rrf_fused_excerpts, vec!["fused".to_string()]);
        assert_eq!(r.from_node_id.as_deref(), Some("node-a"));
    }
}
