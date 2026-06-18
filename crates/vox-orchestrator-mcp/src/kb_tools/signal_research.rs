//! Research completion signal adapter — ingests research synthesis reports into matching KBs.

use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator::knowledge_base::{
    router::{SIMILARITY_THRESHOLD, apply_keyword_rules, apply_similarity_routing},
    store::KbStore,
    types::KbEntrySource,
};

/// Ingest a completed research synthesis report into matching KBs.
/// Research reports are high-value (minimum confidence 0.95).
pub async fn ingest_research_result(
    db: Arc<VoxDb>,
    synthesis: &str,
    query: &str,
    session_id: Option<i64>,
) {
    if synthesis.trim().is_empty() {
        return;
    }

    let store = KbStore::new(db);
    let kbs = match store.list().await {
        Ok(kbs) if !kbs.is_empty() => kbs,
        _ => return,
    };

    let mut all_rules = Vec::new();
    for kb in &kbs {
        if let Ok(rules) = store.list_rules(&kb.id).await {
            all_rules.extend(rules);
        }
    }

    let combined = format!("{query}\n\n{synthesis}");
    let mut targets = apply_keyword_rules(&combined, &all_rules);

    if targets.is_empty() {
        let mut samples: Vec<(String, Vec<String>)> = Vec::new();
        for kb in &kbs {
            let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
            let contents: Vec<String> = entries.into_iter().map(|e| e.content).collect();
            if !contents.is_empty() {
                samples.push((kb.id.clone(), contents));
            }
        }
        targets = apply_similarity_routing(&combined, &samples, SIMILARITY_THRESHOLD);
    }

    let source_ref = session_id.map(|id| format!("research-session-{id}"));

    for (kb_id, confidence) in targets {
        let effective_confidence = confidence.max(0.95);
        let _ = store
            .add_entry(
                &kb_id,
                synthesis,
                KbEntrySource::Research,
                source_ref.as_deref(),
                effective_confidence,
                &[],
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_compiles() {}
}
