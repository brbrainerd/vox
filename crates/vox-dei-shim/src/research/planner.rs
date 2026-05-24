//! Query planner for decomposing a research question into bounded subqueries.

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use super::types::{ResearchPlan, ResearchQuery};

/// Decompose a research query into a plan with at least one subquery.
///
/// **PHASE_0a_STUB**: returns a plan with the original query as the only subquery.
pub async fn decompose_query_with_config(
    query: &ResearchQuery,
    endpoint: Option<&str>,
    api_key: Option<&str>,
    model: Option<&str>,
    temperature: Option<f32>,
    max_subqueries: Option<usize>,
) -> Result<ResearchPlan> {
    let max_subqueries = max_subqueries.unwrap_or(6).clamp(1, 12);

    #[cfg(feature = "runtime")]
    {
        use vox_actor_runtime::ActivityOptions;
        use vox_actor_runtime::llm::LlmChatMessage;
        use vox_actor_runtime::llm::cascade::{
            ResearchStage, cascade_with_optional_manual, chat_with_cascade,
        };
        use vox_actor_runtime::model_resolution::RouteResolutionInput;

        let mut input = RouteResolutionInput::default();
        if let Some(model) = model.filter(|m| !m.trim().is_empty()) {
            input.openrouter_model = model.to_string();
        }
        let mut candidates =
            cascade_with_optional_manual(ResearchStage::Planner, &input, endpoint, api_key, model);
        for candidate in &mut candidates {
            candidate.temperature = temperature.or(candidate.temperature);
            candidate.max_tokens = Some(700);
            candidate.response_format = Some(serde_json::json!({"type": "json_object"}));
        }

        let messages = vec![
            LlmChatMessage {
                role: "system".to_string(),
                content: format!(
                    "Decompose the user's research question into 3-{max_subqueries} precise web/local retrieval subqueries. \
                     Output only valid JSON with schema: {{\"subqueries\": [\"...\"]}}."
                ),
            },
            LlmChatMessage {
                role: "user".to_string(),
                content: query.query.clone(),
            },
        ];
        let opts = ActivityOptions::new().with_timeout_secs(30);
        match chat_with_cascade(&opts, messages, candidates, None).await {
            Ok(response) => {
                match parse_planner_response(&response.content, query, max_subqueries) {
                    Ok(plan) => return Ok(plan),
                    Err(e) => {
                        tracing::warn!(error = %e, "research planner response was invalid; falling back")
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "research planner cascade failed; falling back"),
        }
    }

    Ok(passthrough_plan(query))
}

/// Serialize a plan to a JSON value for telemetry persistence.
#[must_use]
pub fn plan_to_json(plan: &ResearchPlan) -> Value {
    serde_json::to_value(plan).unwrap_or(Value::Null)
}

#[derive(Deserialize)]
struct PlannerPayload {
    subqueries: Vec<String>,
}

fn parse_planner_response(
    response: &str,
    query: &ResearchQuery,
    max_subqueries: usize,
) -> Result<ResearchPlan> {
    let payload: PlannerPayload = super::json_parse::parse_json_response(response)?;
    let mut seen = std::collections::HashSet::new();
    let mut subqueries = Vec::new();
    for raw in payload.subqueries {
        let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            continue;
        }
        let key = normalized.to_ascii_lowercase();
        if seen.insert(key) {
            subqueries.push(normalized);
        }
        if subqueries.len() >= max_subqueries.max(1) {
            break;
        }
    }
    if subqueries.is_empty() {
        anyhow::bail!("no usable subqueries in planner response");
    }
    Ok(ResearchPlan {
        original_query: query.query.clone(),
        subqueries,
        scope: query.scope.clone(),
        max_sources_per_subquery: query.max_sources,
    })
}

fn passthrough_plan(query: &ResearchQuery) -> ResearchPlan {
    ResearchPlan {
        original_query: query.query.clone(),
        subqueries: vec![query.query.clone()],
        scope: query.scope.clone(),
        max_sources_per_subquery: query.max_sources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dei_shim::research::types::ResearchScope;

    fn query() -> ResearchQuery {
        ResearchQuery {
            query: "compare CRAG and Deep Research citation verification".to_string(),
            scope: ResearchScope::Both,
            max_sources: 8,
            persist_to_docs: false,
            verify_claims: true,
            site_scope: None,
        }
    }

    #[test]
    fn parse_planner_response_accepts_json_codeblock_dedupes_and_limits() {
        let response = r#"```json
        {
          "subqueries": [
            "What is CRAG?",
            "How does Deep Research verify citations?",
            "What is CRAG?",
            "Compare CRAG to citation verification",
            "Find Vox integration constraints"
          ]
        }
        ```"#;

        let plan = parse_planner_response(response, &query(), 3).expect("plan parses");

        assert_eq!(plan.subqueries.len(), 3);
        assert_eq!(plan.subqueries[0], "What is CRAG?");
        assert_eq!(
            plan.subqueries[1],
            "How does Deep Research verify citations?"
        );
        assert_eq!(plan.scope, ResearchScope::Both);
        assert_eq!(plan.max_sources_per_subquery, 8);
    }

    #[test]
    fn parse_planner_response_rejects_empty_subqueries() {
        let err = parse_planner_response(r#"{"subqueries":["", "   "]}"#, &query(), 6)
            .expect_err("empty plans should not be accepted");

        assert!(err.to_string().contains("no usable subqueries"));
    }
}
