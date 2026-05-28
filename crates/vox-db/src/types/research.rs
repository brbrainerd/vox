use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchSessionRecord {
    pub id: i64,
    pub session_key: String,
    pub status: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub query_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchSessionSummary {
    pub id: i64,
    pub session_key: String,
    pub status: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub query_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchArtifactRecord {
    pub session_id: i64,
    pub artifact_json: String,
    pub report_markdown: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}
