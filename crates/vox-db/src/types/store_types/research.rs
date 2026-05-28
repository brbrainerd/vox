use serde::{Deserialize, Serialize};

/// Serializable metadata for a captured research artifact (before DB insert).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalResearchPacket {
    /// Subject area or product topic.
    pub topic: String,
    /// Vendor or competitor name.
    pub vendor: String,
    /// Optional sub-area (e.g. "CLI", "pricing").
    pub area: Option<String>,
    /// Canonical URL of the source page.
    pub source_url: String,
    /// MIME-ish label: `web`, `pdf`, etc.
    pub source_type: String,
    /// Human-readable document title.
    pub title: String,
    /// ISO-8601 or similar capture timestamp string.
    pub captured_at: String,
    /// Short abstract for search / UI.
    pub summary: String,
    /// Verbatim excerpt retained for audit.
    pub raw_excerpt: String,
    /// Structured claim objects (JSON).
    pub claims: Vec<serde_json::Value>,
    /// Free-form tags for filtering.
    pub tags: Vec<String>,
    /// Heuristic confidence in `[0, 1]` (caller-defined).
    pub confidence: f64,
    /// Blake3 hex over normalized content (dedup key).
    pub content_hash: String,
    /// Extra JSON bag (provenance, tool ids, etc.).
    pub metadata: serde_json::Value,
}

/// Full ingest input: structured packet plus raw body text for chunking.
pub struct ResearchIngestRequest {
    /// Structured header fields (hashed and stored in `knowledge_nodes.metadata`).
    pub packet: ExternalResearchPacket,
    /// Full text used for chunking and embedding hooks.
    pub body: String,
    /// Optional knowledge-base id for partitioning.
    pub kb_id: Option<String>,
    /// Optional chunk embeddings aligned with the chunked body when available.
    pub embeddings: Vec<Vec<f32>>,
}

/// Row ids returned from `vox_db::VoxDb::ingest_research_document`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchIngestResult {
    /// `knowledge_nodes` row id from the insert.
    pub packet_id: i64,
    /// Optional normalized `search_documents` row id when dual-write succeeds.
    pub document_id: Option<i64>,
    /// `snippets` row ids for each text chunk.
    pub chunk_ids: Vec<i64>,
    /// Echo of request `kb_id`.
    pub kb_id: Option<String>,
    /// Blake3 hex over `body` (also written into the packet).
    pub content_hash: String,
}

/// One capability comparison row for `codex_capability_map`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityMapRecord {
    /// Subject or product under comparison.
    pub topic: String,
    /// External vendor or competitor id.
    pub vendor: String,
    /// Sub-domain (e.g. `pricing`, `security`).
    pub area: String,
    /// Capability label in the comparison framework.
    pub openclaw_capability: String,
    /// Vox-side evidence or counterpoint text.
    pub vox_evidence: String,
    /// Workflow status (e.g. `open`, `closed`).
    pub status: String,
    /// Who leads: `vox`, `peer`, `tie`, etc.
    pub advantage_direction: String,
    /// Suggested next engineering or research step.
    pub recommended_action: String,
    /// Repo paths or URLs cited in the analysis.
    pub linked_paths: Vec<String>,
    /// Free-form JSON for tooling-specific fields.
    pub metadata: serde_json::Value,
}

/// Consolidated metrics for a research evaluation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchEvalRunRecord {
    pub run_id: String,
    pub model_id: String,
    pub config: serde_json::Value,
    pub metrics: serde_json::Value,
    pub latency_p50_ms: Option<i64>,
    pub latency_p99_ms: Option<i64>,
    pub tier_distribution: serde_json::Value,
    pub created_at_ms: i64,
}

/// Single query sample from a research evaluation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchEvalSampleRecord {
    pub run_id: String,
    pub query: String,
    pub gold_answer: Option<String>,
    pub model_answer: String,
    pub recall_at_5: Option<f64>,
    pub groundedness: Option<f64>,
    pub quality_score: Option<f64>,
    pub latency_ms: Option<i64>,
    pub evidence: serde_json::Value,
    pub recorded_at_ms: i64,
}
