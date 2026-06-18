//! MCP argument structs for VoxKB tools.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbCreateParams {
    /// Unique name for the knowledge base (e.g. "Rust async patterns").
    pub name: String,
    /// Short description of what this KB collects.
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbDeleteParams {
    pub kb_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbAddEntryParams {
    pub kb_id: String,
    /// The content to store. Should be a self-contained atomic fact (~100-300 tokens).
    pub content: String,
    /// Signal source: "chat", "research", "code_activity", "web", "explicit", "scientia".
    #[serde(default = "default_explicit")]
    pub source_signal: String,
    /// Optional source reference (URL, file path, session ID).
    pub source_ref: Option<String>,
    /// JSON array of tag strings, e.g. ["rust","async"]. Default: [].
    #[serde(default = "default_empty_array")]
    pub tags: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbDeleteEntryParams {
    pub entry_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbListEntriesParams {
    pub kb_id: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbReviewEntryParams {
    pub entry_id: String,
    /// `true` = accept into KB (queued for MENS SFT). `false` = reject (DPO negative).
    pub accepted: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbGetFeedParams {
    #[serde(default = "default_feed_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbAddRuleParams {
    pub kb_id: String,
    /// Rule type: "keyword" (case-insensitive substring) or "regex" (pattern match).
    #[serde(default = "default_keyword")]
    pub rule_type: String,
    /// Pattern to match against entry content.
    pub pattern: String,
    /// Higher = checked first. Default: 0.
    #[serde(default)]
    pub priority: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbListRulesParams {
    pub kb_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbQueryParams {
    /// Free-text search query.
    pub query: String,
    /// Optional: only return results from these KB IDs. Empty = all KBs.
    #[serde(default)]
    pub kb_ids: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbClipParams {
    /// Content to save. Should be a self-contained atomic fact or insight.
    pub content: String,
    /// Optional source reference (URL, file path).
    pub source_ref: Option<String>,
    /// KB IDs to clip into. If empty, the router decides via keyword rules.
    #[serde(default)]
    pub kb_ids: Vec<String>,
    /// JSON array of tag strings.
    #[serde(default = "default_empty_array")]
    pub tags: String,
}

fn default_explicit() -> String {
    "explicit".to_string()
}
fn default_keyword() -> String {
    "keyword".to_string()
}
fn default_empty_array() -> String {
    "[]".to_string()
}
fn default_limit() -> i64 {
    20
}
fn default_feed_limit() -> i64 {
    50
}
