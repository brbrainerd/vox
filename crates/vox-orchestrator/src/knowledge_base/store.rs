//! VoxDb CRUD for knowledge bases and entries.

use std::sync::Arc;

use vox_db::VoxDb;

use crate::{
    knowledge_base::types::{
        KbEntry, KbEntrySource, KbRoutingRule, KbRoutingRuleType, KnowledgeBase,
    },
    now_unix_ms,
};

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn kb_from_row(r: vox_db::KbRow) -> KnowledgeBase {
    KnowledgeBase {
        id: r.id,
        name: r.name,
        description: r.description,
        created_at_ms: r.created_at_ms,
        updated_at_ms: r.updated_at_ms,
        entry_count: r.entry_count,
    }
}

fn entry_from_row(r: vox_db::KbEntryRow) -> KbEntry {
    KbEntry {
        id: r.id,
        kb_id: r.kb_id,
        content: r.content,
        source_signal: r.source_signal,
        source_ref: r.source_ref,
        routing_confidence: r.routing_confidence,
        tags: r.tags,
        created_at_ms: r.created_at_ms,
        last_accessed_at_ms: r.last_accessed_at_ms,
        access_count: r.access_count,
        accepted: r.accepted,
        mens_queued: r.mens_queued,
    }
}

fn rule_from_row(r: vox_db::KbRuleRow) -> KbRoutingRule {
    let rule_type = if r.rule_type == "regex" {
        KbRoutingRuleType::Regex
    } else {
        KbRoutingRuleType::Keyword
    };
    KbRoutingRule {
        id: r.id,
        kb_id: r.kb_id,
        rule_type,
        pattern: r.pattern,
        priority: r.priority,
        created_at_ms: r.created_at_ms,
    }
}

/// Async CRUD for knowledge bases backed by VoxDb.
pub struct KbStore {
    db: Arc<VoxDb>,
}

impl KbStore {
    pub fn new(db: Arc<VoxDb>) -> Self {
        Self { db }
    }

    pub async fn create(&self, name: &str, description: &str) -> Result<KnowledgeBase, String> {
        let id = new_id();
        let now = now_unix_ms() as i64;
        self.db
            .kb_create(&id, name, description, now)
            .await
            .map_err(|e| e.to_string())?;
        Ok(KnowledgeBase {
            id,
            name: name.to_string(),
            description: description.to_string(),
            created_at_ms: now,
            updated_at_ms: now,
            entry_count: 0,
        })
    }

    pub async fn list(&self) -> Result<Vec<KnowledgeBase>, String> {
        self.db
            .kb_list()
            .await
            .map(|rows| rows.into_iter().map(kb_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        self.db.kb_delete(id).await.map_err(|e| e.to_string())
    }

    /// Add an entry to a KB with exact-content deduplication.
    /// If the content already exists in the KB, returns the existing entry's id without inserting.
    pub async fn add_entry(
        &self,
        kb_id: &str,
        content: &str,
        source: KbEntrySource,
        source_ref: Option<&str>,
        routing_confidence: f64,
        tags: &[String],
    ) -> Result<KbEntry, String> {
        // Deduplication check (SOTA: search-before-insert)
        if let Ok(Some(existing_id)) = self.db.kb_find_duplicate(kb_id, content).await {
            if let Ok(Some(row)) = self.db.kb_get_entry(&existing_id).await {
                return Ok(entry_from_row(row));
            }
        }

        let id = new_id();
        let now = now_unix_ms() as i64;
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        self.db
            .kb_add_entry(
                &id,
                kb_id,
                content,
                source.as_str(),
                source_ref,
                routing_confidence,
                &tags_json,
                now,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(KbEntry {
            id,
            kb_id: kb_id.to_string(),
            content: content.to_string(),
            source_signal: source.as_str().to_string(),
            source_ref: source_ref.map(str::to_string),
            routing_confidence,
            tags: tags_json,
            created_at_ms: now,
            last_accessed_at_ms: None,
            access_count: 0,
            accepted: 1,
            mens_queued: 0,
        })
    }

    pub async fn list_entries(
        &self,
        kb_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_list_entries(kb_id, limit, offset)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    /// Accept or reject an entry. Sets `mens_queued = 1` for accepted entries.
    pub async fn review_entry(&self, entry_id: &str, accepted: bool) -> Result<(), String> {
        self.db
            .kb_review_entry(entry_id, accepted, accepted)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn delete_entry(&self, entry_id: &str) -> Result<(), String> {
        self.db
            .kb_delete_entry(entry_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_feed(&self, limit: i64) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_get_feed(limit)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn add_rule(
        &self,
        kb_id: &str,
        rule_type: KbRoutingRuleType,
        pattern: &str,
        priority: i64,
    ) -> Result<KbRoutingRule, String> {
        let id = new_id();
        let now = now_unix_ms() as i64;
        self.db
            .kb_add_rule(&id, kb_id, rule_type.as_str(), pattern, priority, now)
            .await
            .map_err(|e| e.to_string())?;
        Ok(KbRoutingRule {
            id,
            kb_id: kb_id.to_string(),
            rule_type,
            pattern: pattern.to_string(),
            priority,
            created_at_ms: now,
        })
    }

    pub async fn list_rules(&self, kb_id: &str) -> Result<Vec<KbRoutingRule>, String> {
        self.db
            .kb_list_rules(kb_id)
            .await
            .map(|rows| rows.into_iter().map(rule_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn search_entries(&self, query: &str, limit: i64) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_search_entries(query, limit)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn unqueued_training_entries(&self, limit: i64) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_unqueued_training_entries(limit)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn mark_mens_queued(&self, ids: &[String]) -> Result<(), String> {
        self.db
            .kb_mark_mens_queued(ids)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_from_row_preserves_fields() {
        let row = vox_db::KbEntryRow {
            id: "e1".to_string(),
            kb_id: "k1".to_string(),
            content: "test content".to_string(),
            source_signal: "chat".to_string(),
            source_ref: Some("chat-session-42".to_string()),
            routing_confidence: 0.9,
            tags: "[\"rust\"]".to_string(),
            created_at_ms: 1000,
            last_accessed_at_ms: None,
            access_count: 0,
            accepted: 1,
            mens_queued: 0,
        };
        let entry = entry_from_row(row);
        assert_eq!(entry.id, "e1");
        assert_eq!(entry.kb_id, "k1");
        assert_eq!(entry.content, "test content");
        assert_eq!(entry.source_signal, "chat");
        assert!((entry.routing_confidence - 0.9).abs() < 1e-9);
        assert_eq!(entry.accepted, 1);
        assert_eq!(entry.mens_queued, 0);
    }

    #[test]
    fn rule_from_row_keyword_type() {
        let row = vox_db::KbRuleRow {
            id: "r1".to_string(),
            kb_id: "k1".to_string(),
            rule_type: "keyword".to_string(),
            pattern: "qdrant".to_string(),
            priority: 10,
            created_at_ms: 1000,
        };
        let rule = rule_from_row(row);
        assert_eq!(rule.rule_type, KbRoutingRuleType::Keyword);
        assert_eq!(rule.pattern, "qdrant");
    }

    #[test]
    fn rule_from_row_unknown_type_defaults_to_keyword() {
        let row = vox_db::KbRuleRow {
            id: "r2".to_string(),
            kb_id: "k1".to_string(),
            rule_type: "future_unknown_type".to_string(),
            pattern: "x".to_string(),
            priority: 0,
            created_at_ms: 1000,
        };
        let rule = rule_from_row(row);
        assert_eq!(rule.rule_type, KbRoutingRuleType::Keyword);
    }

    #[test]
    fn new_id_is_valid_uuid_format() {
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }
}
