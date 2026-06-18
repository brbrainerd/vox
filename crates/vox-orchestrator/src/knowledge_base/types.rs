//! Core types for VoxKB — knowledge bases, entries, routing rules.

use serde::{Deserialize, Serialize};

/// A named, topic-scoped knowledge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub entry_count: i64,
}

/// A single entry stored in a knowledge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KbEntry {
    pub id: String,
    pub kb_id: String,
    pub content: String,
    pub source_signal: String,
    pub source_ref: Option<String>,
    pub routing_confidence: f64,
    /// JSON array of tag strings, e.g. `["rust","async"]`.
    pub tags: String,
    pub created_at_ms: i64,
    pub last_accessed_at_ms: Option<i64>,
    pub access_count: i64,
    /// `1` = accepted into KB (SFT signal); `0` = rejected (DPO negative).
    pub accepted: i64,
    /// `1` = already queued for MENS training.
    pub mens_queued: i64,
}

/// Signal source that produced a KB entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KbEntrySource {
    Chat,
    Research,
    CodeActivity,
    Web,
    Explicit,
    Scientia,
}

impl KbEntrySource {
    pub fn as_str(self) -> &'static str {
        match self {
            KbEntrySource::Chat => "chat",
            KbEntrySource::Research => "research",
            KbEntrySource::CodeActivity => "code_activity",
            KbEntrySource::Web => "web",
            KbEntrySource::Explicit => "explicit",
            KbEntrySource::Scientia => "scientia",
        }
    }
}

/// Type of routing rule that classifies content into a KB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KbRoutingRuleType {
    Keyword,
    Regex,
}

impl KbRoutingRuleType {
    pub fn as_str(self) -> &'static str {
        match self {
            KbRoutingRuleType::Keyword => "keyword",
            KbRoutingRuleType::Regex => "regex",
        }
    }
}

/// A rule that routes content into a specific KB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KbRoutingRule {
    pub id: String,
    pub kb_id: String,
    pub rule_type: KbRoutingRuleType,
    pub pattern: String,
    /// Higher priority rules are checked first.
    pub priority: i64,
    pub created_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kb_entry_source_roundtrips_as_str() {
        assert_eq!(KbEntrySource::Chat.as_str(), "chat");
        assert_eq!(KbEntrySource::Research.as_str(), "research");
        assert_eq!(KbEntrySource::CodeActivity.as_str(), "code_activity");
        assert_eq!(KbEntrySource::Web.as_str(), "web");
        assert_eq!(KbEntrySource::Explicit.as_str(), "explicit");
        assert_eq!(KbEntrySource::Scientia.as_str(), "scientia");
    }

    #[test]
    fn kb_routing_rule_type_roundtrips() {
        assert_eq!(KbRoutingRuleType::Keyword.as_str(), "keyword");
        assert_eq!(KbRoutingRuleType::Regex.as_str(), "regex");
    }
}
