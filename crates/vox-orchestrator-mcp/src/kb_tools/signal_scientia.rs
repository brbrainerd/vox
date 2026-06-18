//! Scientia finding promotion adapter — ingests approved findings into matching KBs.

use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator::knowledge_base::{
    router::{SIMILARITY_THRESHOLD, apply_keyword_rules, apply_similarity_routing},
    store::KbStore,
    types::KbEntrySource,
};

/// Ingest an approved Scientia finding.
/// Scientia-approved findings always get confidence 0.98 (high trust).
pub async fn ingest_scientia_finding(db: Arc<VoxDb>, finding_text: &str, finding_id: &str) {
    if finding_text.trim().is_empty() {
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

    let mut targets = apply_keyword_rules(finding_text, &all_rules);
    if targets.is_empty() {
        let mut samples: Vec<(String, Vec<String>)> = Vec::new();
        for kb in &kbs {
            let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
            let contents: Vec<String> = entries.into_iter().map(|e| e.content).collect();
            if !contents.is_empty() {
                samples.push((kb.id.clone(), contents));
            }
        }
        targets = apply_similarity_routing(finding_text, &samples, SIMILARITY_THRESHOLD);
    }

    for (kb_id, _) in targets {
        let _ = store
            .add_entry(
                &kb_id,
                finding_text,
                KbEntrySource::Scientia,
                Some(finding_id),
                0.98,
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
