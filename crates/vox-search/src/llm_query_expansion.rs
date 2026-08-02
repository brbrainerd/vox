//! Shared LLM-driven CRAG query expansion, usable by both the primary
//! research-shim orchestrator pipeline and vox-search's standalone
//! autonomous-research loop (both depend on this crate).

/// Parses an LLM response expected to contain `{"followup_queries": [...]}`
/// somewhere in the text (tolerating markdown fences / surrounding prose).
/// Returns `None` if no valid, non-empty query list can be extracted.
pub fn parse_followup_queries(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start > end {
        return None;
    }
    let json_str = &text[start..=end];

    #[derive(serde::Deserialize)]
    struct Expansion {
        followup_queries: Vec<String>,
    }
    let parsed: Expansion = serde_json::from_str(json_str).ok()?;
    let queries: Vec<String> = parsed
        .followup_queries
        .into_iter()
        .filter(|q| !q.trim().is_empty())
        .collect();

    if queries.is_empty() { None } else { Some(queries) }
}

/// Attempts LLM-driven CRAG query expansion given a research question and
/// the top evidence snippets gathered so far. Returns `None` on any
/// failure (LLM call, parsing) — callers should fall back to
/// `CragRouter::expand_queries_from_partial_evidence` in that case.
pub async fn try_llm_query_expansion(
    original_query: &str,
    top_snippets: &[String],
    llm_endpoint: Option<&str>,
    api_key: Option<&str>,
    planner_model: Option<&str>,
) -> Option<Vec<String>> {
    use vox_actor_runtime::ActivityOptions;
    use vox_actor_runtime::llm::LlmChatMessage;
    use vox_actor_runtime::llm::cascade::{
        ResearchStage, cascade_with_optional_manual, chat_with_cascade,
    };
    use vox_actor_runtime::model_resolution::RouteResolutionInput;

    let snippets_text = top_snippets
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, s)| format!("{}. {}", i + 1, s.chars().take(300).collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n");

    let user_msg = format!(
        "Research question: {original_query}\n\nEvidence so far:\n{snippets_text}\n\n\
        Identify 2-4 specific follow-up search queries covering the most important missing \
        aspects. Output ONLY valid JSON:\n{{\"followup_queries\": [\"query 1\", \"query 2\"]}}"
    );

    let messages = vec![
        LlmChatMessage {
            role: "system".to_string(),
            content: "You are a research gap analyst. Generate precise follow-up search \
                      queries to fill knowledge gaps. Output only valid JSON."
                .to_string(),
            ..Default::default()
        },
        LlmChatMessage {
            role: "user".to_string(),
            content: user_msg,
            ..Default::default()
        },
    ];

    let candidates = cascade_with_optional_manual(
        ResearchStage::Planner,
        &RouteResolutionInput::default(),
        llm_endpoint,
        api_key,
        planner_model,
    );

    let opts = ActivityOptions::default();
    let Ok(response) =
        chat_with_cascade(&opts, messages, candidates, Some(ResearchStage::Planner)).await
    else {
        tracing::warn!("LLM query expansion cascade failed to generate chat completion");
        return None;
    };

    let queries = parse_followup_queries(response.content.trim());
    if queries.is_none() {
        tracing::warn!(raw_response = %response.content, "LLM query expansion failed to parse");
    }
    queries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json() {
        let text = r#"{"followup_queries": ["query one", "query two"]}"#;
        assert_eq!(
            parse_followup_queries(text),
            Some(vec!["query one".to_string(), "query two".to_string()])
        );
    }

    #[test]
    fn parses_json_with_surrounding_prose() {
        let text = "Here is my answer:\n{\"followup_queries\": [\"only query\"]}\nDone.";
        assert_eq!(
            parse_followup_queries(text),
            Some(vec!["only query".to_string()])
        );
    }

    #[test]
    fn filters_blank_queries() {
        let text = r#"{"followup_queries": ["real query", "  ", ""]}"#;
        assert_eq!(
            parse_followup_queries(text),
            Some(vec!["real query".to_string()])
        );
    }

    #[test]
    fn returns_none_on_empty_query_list() {
        let text = r#"{"followup_queries": []}"#;
        assert_eq!(parse_followup_queries(text), None);
    }

    #[test]
    fn returns_none_on_malformed_json() {
        let text = "not json at all, no braces";
        assert_eq!(parse_followup_queries(text), None);
    }

    #[test]
    fn returns_none_on_wrong_shape() {
        let text = r#"{"something_else": ["a"]}"#;
        assert_eq!(parse_followup_queries(text), None);
    }
}
