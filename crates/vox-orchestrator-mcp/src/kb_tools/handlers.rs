//! MCP handler functions for VoxKB tools.

use crate::params::ToolResult;
use crate::server_state::ServerState;

use super::params::*;
use vox_orchestrator::knowledge_base::{
    router::apply_keyword_rules,
    store::KbStore,
    types::{KbEntrySource, KbRoutingRuleType},
};

const REM_KB_DB: &str =
    "Attach VoxDb (VOX_DB_PATH / VOX_DB_URL) to the MCP server for KB operations.";
const REM_KB_NOT_FOUND: &str = "Run vox_kb_list to see available KB IDs.";

fn require_db(state: &ServerState) -> Result<std::sync::Arc<vox_db::VoxDb>, String> {
    state.db.clone().ok_or_else(|| REM_KB_DB.to_string())
}

/// Create a new named knowledge base.
pub async fn kb_create(state: &ServerState, params: KbCreateParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.create(&params.name, &params.description).await {
                Ok(kb) => ToolResult::ok(serde_json::to_value(&kb).unwrap_or_default()).to_json(),
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// List all knowledge bases.
pub async fn kb_list(state: &ServerState) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.list().await {
                Ok(kbs) => ToolResult::ok(serde_json::to_value(&kbs).unwrap_or_default()).to_json(),
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Delete a knowledge base and all its entries.
pub async fn kb_delete(state: &ServerState, params: KbDeleteParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.delete(&params.kb_id).await {
                Ok(()) => ToolResult::ok("deleted").to_json(),
                Err(e) => ToolResult::<()>::err_with_remediation(e, REM_KB_NOT_FOUND).to_json(),
            }
        }
    }
}

/// Add an entry to a knowledge base (with deduplication).
pub async fn kb_add_entry(state: &ServerState, params: KbAddEntryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let source = match params.source_signal.as_str() {
                "chat" => KbEntrySource::Chat,
                "research" => KbEntrySource::Research,
                "code_activity" => KbEntrySource::CodeActivity,
                "web" => KbEntrySource::Web,
                "scientia" => KbEntrySource::Scientia,
                _ => KbEntrySource::Explicit,
            };
            let tags: Vec<String> = serde_json::from_str(&params.tags).unwrap_or_default();
            let store = KbStore::new(db);
            match store
                .add_entry(
                    &params.kb_id,
                    &params.content,
                    source,
                    params.source_ref.as_deref(),
                    1.0,
                    &tags,
                )
                .await
            {
                Ok(entry) => {
                    ToolResult::ok(serde_json::to_value(&entry).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err_with_remediation(e, REM_KB_NOT_FOUND).to_json(),
            }
        }
    }
}

/// Delete a specific KB entry (also decrements parent KB entry_count).
pub async fn kb_delete_entry(state: &ServerState, params: KbDeleteEntryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.delete_entry(&params.entry_id).await {
                Ok(()) => ToolResult::ok("deleted").to_json(),
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// List entries in a knowledge base.
pub async fn kb_list_entries(state: &ServerState, params: KbListEntriesParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store
                .list_entries(&params.kb_id, params.limit, params.offset)
                .await
            {
                Ok(entries) => {
                    ToolResult::ok(serde_json::to_value(&entries).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Accept or reject a KB entry (accepted → queued for MENS SFT; rejected → DPO pair).
pub async fn kb_review_entry(state: &ServerState, params: KbReviewEntryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.review_entry(&params.entry_id, params.accepted).await {
                Ok(()) => ToolResult::ok(if params.accepted {
                    "accepted"
                } else {
                    "rejected"
                })
                .to_json(),
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Get the knowledge feed — recent entries across all KBs.
pub async fn kb_get_feed(state: &ServerState, params: KbGetFeedParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.get_feed(params.limit).await {
                Ok(entries) => {
                    ToolResult::ok(serde_json::to_value(&entries).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Add a routing rule to a KB.
pub async fn kb_add_rule(state: &ServerState, params: KbAddRuleParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let rule_type = if params.rule_type == "regex" {
                KbRoutingRuleType::Regex
            } else {
                KbRoutingRuleType::Keyword
            };
            let store = KbStore::new(db);
            match store
                .add_rule(&params.kb_id, rule_type, &params.pattern, params.priority)
                .await
            {
                Ok(rule) => {
                    ToolResult::ok(serde_json::to_value(&rule).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err_with_remediation(e, REM_KB_NOT_FOUND).to_json(),
            }
        }
    }
}

/// List routing rules for a KB.
pub async fn kb_list_rules(state: &ServerState, params: KbListRulesParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.list_rules(&params.kb_id).await {
                Ok(rules) => {
                    ToolResult::ok(serde_json::to_value(&rules).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Substring search across accepted KB entries.
pub async fn kb_query(state: &ServerState, params: KbQueryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.search_entries(&params.query, params.limit).await {
                Ok(mut entries) => {
                    if !params.kb_ids.is_empty() {
                        entries.retain(|e| params.kb_ids.contains(&e.kb_id));
                    }
                    ToolResult::ok(serde_json::to_value(&entries).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Explicit clip — user saves content directly into specified KB(s).
/// If `kb_ids` is empty, auto-routes via KbRouter keyword rules.
pub async fn kb_clip(state: &ServerState, params: KbClipParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            let tags: Vec<String> = serde_json::from_str(&params.tags).unwrap_or_default();

            let target_kb_ids = if params.kb_ids.is_empty() {
                let kbs = store.list().await.unwrap_or_default();
                let mut all_rules = Vec::new();
                for kb in &kbs {
                    let rules = store.list_rules(&kb.id).await.unwrap_or_default();
                    all_rules.extend(rules);
                }
                apply_keyword_rules(&params.content, &all_rules)
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            } else {
                params.kb_ids.clone()
            };

            let mut saved = Vec::new();
            for kb_id in &target_kb_ids {
                match store
                    .add_entry(
                        kb_id,
                        &params.content,
                        KbEntrySource::Explicit,
                        params.source_ref.as_deref(),
                        1.0,
                        &tags,
                    )
                    .await
                {
                    Ok(entry) => saved.push(entry),
                    Err(e) => return ToolResult::<()>::err(e).to_json(),
                }
            }
            ToolResult::ok(serde_json::to_value(&saved).unwrap_or_default()).to_json()
        }
    }
}
