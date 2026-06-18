//! Chat turn signal adapter — extracts KB entries from completed assistant turns.

use std::collections::HashSet;

/// Minimum character length for a snippet to be stored.
const MIN_SNIPPET_LEN: usize = 20;

/// Maximum characters per paragraph before sentence-splitting is applied.
const MAX_SNIPPET_CHARS: usize = 512;

/// Extract KB-candidate snippets from an assistant turn's content.
///
/// Splits by paragraph boundary (`\n\n`), then by sentence boundary (`. `) for long
/// paragraphs. Deduplicates case-insensitively and filters very short strings.
pub fn extract_chat_snippets(content: &str) -> Vec<String> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    let paragraphs: Vec<&str> = content
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for para in paragraphs {
        let chunks: Vec<String> = if para.len() <= MAX_SNIPPET_CHARS {
            vec![para.to_string()]
        } else {
            para.split(". ")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };

        for chunk in chunks {
            if chunk.len() < MIN_SNIPPET_LEN {
                continue;
            }
            let key = chunk.to_ascii_lowercase();
            if !seen.contains(&key) {
                seen.insert(key);
                result.push(chunk);
            }
        }
    }
    result
}

/// Fire-and-forget: route `assistant_content` snippets into matching KBs.
///
/// Called after a chat turn completes. Errors are swallowed to avoid disrupting
/// the chat flow. If no KBs are configured, returns immediately.
///
/// NOTE: `session_ref` is the `session_id: String` from message.rs — pass as
/// `Some(session_id.as_str())`.
pub async fn ingest_chat_turn(
    db: std::sync::Arc<vox_db::VoxDb>,
    assistant_content: &str,
    session_ref: Option<&str>,
) {
    use vox_orchestrator::knowledge_base::{
        router::{SIMILARITY_THRESHOLD, apply_keyword_rules, apply_similarity_routing},
        store::KbStore,
        types::KbEntrySource,
    };

    let snippets = extract_chat_snippets(assistant_content);
    if snippets.is_empty() {
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

    for snippet in &snippets {
        // Tier 1: keyword rules
        let mut targets = apply_keyword_rules(snippet, &all_rules);

        // Tier 2: Jaccard similarity (only if tier 1 found no match)
        if targets.is_empty() {
            let mut samples: Vec<(String, Vec<String>)> = Vec::new();
            for kb in &kbs {
                // Cap at 10 samples per KB to keep O(n) bounded
                let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
                let contents: Vec<String> = entries.into_iter().map(|e| e.content).collect();
                if !contents.is_empty() {
                    samples.push((kb.id.clone(), contents));
                }
            }
            targets = apply_similarity_routing(snippet, &samples, SIMILARITY_THRESHOLD);
        }

        for (kb_id, confidence) in targets {
            let _ = store
                .add_entry(
                    &kb_id,
                    snippet,
                    KbEntrySource::Chat,
                    session_ref,
                    confidence,
                    &[],
                )
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_chat_snippets_short_content_single_snippet() {
        let content = "The quick brown fox jumps over the lazy dog repeatedly";
        let snippets = extract_chat_snippets(content);
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0], content);
    }

    #[test]
    fn extract_chat_snippets_filters_very_short_content() {
        let snippets = extract_chat_snippets("ok");
        assert!(snippets.is_empty());
    }

    #[test]
    fn extract_chat_snippets_empty_returns_empty() {
        let snippets = extract_chat_snippets("   ");
        assert!(snippets.is_empty());
    }

    #[test]
    fn extract_chat_snippets_deduplicates() {
        let content =
            "Rust uses ownership for memory safety.\n\nRust uses ownership for memory safety.";
        let snippets = extract_chat_snippets(content);
        assert_eq!(snippets.len(), 1);
    }

    #[test]
    fn extract_chat_snippets_splits_long_content() {
        let content = "a ".repeat(300);
        let snippets = extract_chat_snippets(&content);
        assert!(snippets.len() <= 600);
    }
}
