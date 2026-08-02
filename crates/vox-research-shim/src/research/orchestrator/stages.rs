use super::super::types::{Citation, ResearchHit, SelfVerificationResult};
use super::config::RESEARCH_COMPLETENESS_RIDER;
use super::helpers::{sanitize_chatml, sanitize_evidence};

/// Count distinct registrable domains in research hits and flag diversity shortfall.
#[must_use]
pub fn evaluate_citation_diversity(
    hits: &[ResearchHit],
    min_distinct_domains: usize,
) -> (usize, bool) {
    let mut domains = std::collections::HashSet::new();
    for hit in hits {
        if let Some(host) = registrable_domain(&hit.url) {
            domains.insert(host);
        }
    }
    let count = domains.len();
    let below = min_distinct_domains > 0 && count < min_distinct_domains;
    (count, below)
}

pub(super) fn registrable_domain(url: &str) -> Option<String> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("repo://")
        || lower.starts_with("vox://")
        || lower.starts_with("tavily-research://")
    {
        return None;
    }
    let rest = lower.split("://").nth(1).unwrap_or(&lower);
    let host = rest
        .split('/')
        .next()
        .unwrap_or(rest)
        .split(':')
        .next()
        .unwrap_or(rest)
        .trim_start_matches("www.");
    if host.is_empty() || host == "localhost" {
        return None;
    }
    Some(host.to_string())
}

pub(super) struct JudgeParams<'a> {
    pub query: &'a str,
    pub answer: &'a str,
    pub citations: &'a [Citation],
    pub endpoint: Option<&'a str>,
    pub api_key: Option<&'a str>,
    pub model: &'a str,
    pub temperature: f32,
    pub max_tokens: u32,
    pub fallback_score: i32,
}

pub(super) fn build_judge_system_prompt() -> String {
    "You are a research quality evaluator. Score the following answer strictly based on the rubric.
You MUST output your evaluation as a valid JSON object embedded in a ```json codeblock. Do not output anything else.

Schema required:
{
  \"factual_accuracy_reasoning\": \"string\",
  \"factual_accuracy_score\": integer (0-33),
  \"citation_density_reasoning\": \"string\",
  \"citation_density_score\": integer (0-33),
  \"coverage_reasoning\": \"string\",
  \"coverage_score\": integer (0-34),
  \"total_score\": integer (0-100)
}
{}"
    .replace("{}", RESEARCH_COMPLETENESS_RIDER)
}

pub(super) async fn judge_quality(params: JudgeParams<'_>) -> i32 {
    let citation_snippets: String = params
        .citations
        .iter()
        .take(5)
        .map(|c| {
            format!(
                "- {} <{}>: {}",
                c.title,
                c.url,
                c.snippet.chars().take(200).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let sys_prompt = build_judge_system_prompt();

    let user_prompt = format!(
        "Query: {}
Answer: {}

Citations used:
{}

Scoring rubric:
1. Factual accuracy: Does the answer align with the cited sources?
2. Citation density: Are key claims backed by at least one citation?
3. Coverage: Does the answer address all major aspects of the query?",
        sanitize_chatml(params.query),
        sanitize_chatml(params.answer),
        sanitize_chatml(&citation_snippets)
    );

    if let Ok(content) = chat_stage(
        vox_actor_runtime::llm::cascade::ResearchStage::Judge,
        params.endpoint,
        params.api_key,
        params.model,
        params.temperature,
        params.max_tokens,
        vec![
            ("system".to_string(), sys_prompt),
            ("user".to_string(), user_prompt),
        ],
        Some(serde_json::json!({"type": "json_object"})),
    )
    .await
    {
        let mut block = content.as_str();
        if let Some(start) = content.find("```json") {
            let rest = &content[start + 7..];
            if let Some(end) = rest.find("```") {
                block = &rest[..end];
            } else {
                block = rest;
            }
        } else if let Some(start) = content.find("```") {
            let rest = &content[start + 3..];
            if let Some(end) = rest.find("```") {
                block = &rest[..end];
            } else {
                block = rest;
            }
        }

        #[derive(serde::Deserialize)]
        struct JudgeResponse {
            #[serde(default)]
            total_score: i32,
        }

        if let Ok(parsed) = serde_json::from_str::<JudgeResponse>(block.trim())
            && parsed.total_score > 0
        {
            return parsed.total_score.clamp(1, 100);
        }
    }

    params.fallback_score
}

pub(super) struct SynthesisParams<'a> {
    pub query: &'a str,
    pub hits: &'a [ResearchHit],
    pub verdicts: &'a [super::super::types::ClaimVerdict],
    pub endpoint: Option<&'a str>,
    pub api_key: Option<&'a str>,
    pub model: &'a str,
    pub temperature: f32,
    pub max_tokens: u32,
    pub context_max_chars: usize,
}

/// LLM-backed synthesis. Falls back to template when no endpoint is configured.
pub(super) async fn synthesize_answer_with_llm(params: SynthesisParams<'_>) -> String {
    if params.hits.is_empty() {
        return format!(
            "No external sources were found for: **{}**. \
             Answering from internal knowledge only.",
            params.query
        );
    }

    // Try LLM synthesis first.
    if let (Some(_ep), Some(_key)) = (params.endpoint, params.api_key) {
        match call_synthesis_llm(&params).await {
            Ok(answer) => return answer,
            Err(e) => tracing::warn!("LLM synthesis failed: {e}, falling back to template"),
        }
    }

    // Template fallback.
    synthesize_answer_template(params.query, params.hits, params.verdicts)
}

async fn call_synthesis_llm(params: &SynthesisParams<'_>) -> anyhow::Result<String> {
    let mut context_budget = params.context_max_chars;

    // Build evidence context from hits.
    let evidence: String = params
        .hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let snippet = sanitize_evidence(&h.snippet.chars().take(600).collect::<String>());
            format!("[{}] {}\nURL: {}\n{}\n", i + 1, h.title, h.url, snippet)
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Truncate to budget.
    let evidence_text: String = evidence.chars().take(context_budget).collect();
    context_budget = context_budget.saturating_sub(evidence_text.len());

    // Append verdict summary if room remains.
    let verdict_text: String = if !params.verdicts.is_empty() && context_budget > 100 {
        params
            .verdicts
            .iter()
            .map(|v| {
                format!(
                    "{}: {} ({:.0}% confidence)",
                    v.claim.text,
                    v.verdict,
                    v.confidence * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    } else {
        String::new()
    };

    let system = format!(
        "You are a precise research synthesizer. Using ONLY the provided evidence \
         snippets, write a thorough, well-structured answer to the user's question. \
         Cite sources inline as [1], [2], etc. matching the evidence numbers. \
         If evidence is insufficient, say so clearly.\n{}",
        RESEARCH_COMPLETENESS_RIDER
    );

    let user = format!(
        "Question: {}\n\nEvidence:\n{}{verdict_section}",
        params.query,
        evidence_text,
        verdict_section = if verdict_text.is_empty() {
            String::new()
        } else {
            format!("\n\nClaim verdicts: {verdict_text}")
        }
    );

    chat_stage(
        vox_actor_runtime::llm::cascade::ResearchStage::Synthesis,
        params.endpoint,
        params.api_key,
        params.model,
        params.temperature,
        params.max_tokens,
        vec![("system".to_string(), system), ("user".to_string(), user)],
        None,
    )
    .await
}

/// Template synthesis fallback (always succeeds, no network call).
fn synthesize_answer_template(
    query: &str,
    hits: &[ResearchHit],
    verdicts: &[super::super::types::ClaimVerdict],
) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("# Research Findings: {query}\n"));

    if !verdicts.is_empty() {
        parts.push("## Verification Status\n".to_string());
        for verdict in verdicts {
            let icon = match verdict.verdict {
                super::super::types::Verdict::Supported => "✅",
                super::super::types::Verdict::Contradicted => "❌",
                super::super::types::Verdict::Contested => "⚠️",
                super::super::types::Verdict::Unverified => "❓",
            };
            parts.push(format!(
                "- {icon} **{}**: {} (confidence: {:.0}%)",
                verdict.claim.text,
                verdict.verdict,
                verdict.confidence * 100.0
            ));
        }
        parts.push(String::new());
    }

    parts.push("## Evidence Summary\n".to_string());
    for (i, hit) in hits.iter().take(5).enumerate() {
        let snippet = hit.snippet.chars().take(500).collect::<String>();
        parts.push(format!(
            "### [{}] {}\n\nSource: <{}>\n\n{}\n",
            i + 1,
            hit.title,
            hit.url,
            snippet
        ));
    }
    if hits.len() > 5 {
        parts.push(format!(
            "*And {} other sources examined.*\n",
            hits.len() - 5
        ));
    }

    parts.push("## Citations\n".to_string());
    for (i, hit) in hits.iter().take(10).enumerate() {
        parts.push(format!(
            "{}. [^source{}]: {} - <{}>",
            i + 1,
            i + 1,
            hit.title,
            hit.url
        ));
    }

    parts.join("\n")
}

/// CoVE-style self-verification step.
pub(super) async fn run_self_verification(
    _query: &str,
    answer: &str,
    hits: &[ResearchHit],
    endpoint: Option<&str>,
    api_key: Option<&str>,
    model: &str,
) -> SelfVerificationResult {
    // Build a compact context from top-5 hits.
    let context: String = hits
        .iter()
        .take(5)
        .map(|h| {
            format!(
                "- {} — {}",
                h.title,
                h.snippet.chars().take(300).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Step 1: Ask the model to generate verification questions from the draft.
    let question_prompt = format!(
        "Given the following research answer, generate up to 5 yes/no verification questions \
that target specific factual claims in the answer. Return one question per line, no numbering.\n\n\
Answer: {answer}\n\nQuestions:"
    );

    let questions: Vec<String> = if let Ok(content) = chat_stage(
        vox_actor_runtime::llm::cascade::ResearchStage::SelfVerification,
        endpoint,
        api_key,
        model,
        0.3,
        300,
        vec![("user".to_string(), question_prompt)],
        None,
    )
    .await
    {
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(5)
            .map(|l| l.trim().to_string())
            .collect()
    } else {
        return SelfVerificationResult {
            checked: true,
            questions_generated: 0,
            inconsistency_count: 0,
            critical_inconsistency: false,
        };
    };

    let questions_generated = questions.len();
    if questions_generated == 0 {
        return SelfVerificationResult {
            checked: true,
            questions_generated: 0,
            inconsistency_count: 0,
            critical_inconsistency: false,
        };
    }

    // Step 2: Answer each question from the retrieved context only and check consistency.
    let mut inconsistency_count = 0usize;
    for q in &questions {
        let verify_prompt = format!(
            "Based ONLY on the following sources, answer this yes/no question.\n\
Sources:\n{context}\n\nQuestion: {q}\n\nAnswer with only 'yes', 'no', or 'unknown'."
        );
        if let Ok(ans) = chat_stage(
            vox_actor_runtime::llm::cascade::ResearchStage::SelfVerification,
            endpoint,
            api_key,
            model,
            0.0,
            10,
            vec![("user".to_string(), verify_prompt)],
            None,
        )
        .await
        {
            let cleaned = ans.trim().to_lowercase();
            // "unknown" counts as a soft inconsistency (answer claimed something the context can't confirm)
            if cleaned.contains("no") || cleaned.contains("unknown") {
                inconsistency_count += 1;
            }
        }
    }

    let critical_inconsistency = inconsistency_count > questions_generated / 2;
    SelfVerificationResult {
        checked: true,
        questions_generated,
        inconsistency_count,
        critical_inconsistency,
    }
}

#[cfg(feature = "runtime")]
async fn chat_stage(
    stage: vox_actor_runtime::llm::cascade::ResearchStage,
    endpoint: Option<&str>,
    api_key: Option<&str>,
    model: &str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<(String, String)>,
    response_format: Option<serde_json::Value>,
) -> anyhow::Result<String> {
    use vox_actor_runtime::ActivityOptions;
    use vox_actor_runtime::llm::LlmChatMessage;
    use vox_actor_runtime::llm::cascade::{
        ResearchStage, cascade_with_optional_manual, chat_with_cascade,
    };
    use vox_actor_runtime::model_resolution::RouteResolutionInput;

    let input = RouteResolutionInput {
        openrouter_model: model.to_string(),
        ..RouteResolutionInput::default()
    };
    // Mirrors the stage->intent mapping established in
    // `research::model_select::resolve_research_models`: Synthesis uses the
    // general research intent, Judge uses the review intent. SelfVerification
    // has no equivalent stage there yet, so it defaults to the research
    // intent as the closest fit (future refinement: a dedicated intent).
    let intent = match stage {
        ResearchStage::Judge => vox_orchestrator::models::SelectionIntent::review(),
        ResearchStage::Synthesis | ResearchStage::SelfVerification => {
            vox_orchestrator::models::SelectionIntent::research()
        }
        ResearchStage::Planner => vox_orchestrator::models::SelectionIntent::research(),
        ResearchStage::ClaimExtraction | ResearchStage::Verification => {
            vox_orchestrator::models::SelectionIntent::nli_classifier()
        }
    };
    let primary =
        crate::research::orchestrator::model_dispatch::primary_candidate_for_intent(intent);
    let mut candidates: Vec<vox_actor_runtime::llm::LlmConfig> = primary.into_iter().collect();
    candidates.extend(cascade_with_optional_manual(
        stage,
        &input,
        endpoint,
        api_key,
        Some(model),
    ));
    for candidate in &mut candidates {
        candidate.temperature = Some(temperature);
        candidate.max_tokens = Some(max_tokens.into());
        candidate.response_format = response_format.clone();
    }
    let messages = messages
        .into_iter()
        .map(|(role, content)| LlmChatMessage {
            role,
            content,
            ..Default::default()
        })
        .collect();
    let opts = ActivityOptions::new().with_timeout_secs(45);
    chat_with_cascade(&opts, messages, candidates, None)
        .await
        .map(|response| response.content)
        .map_err(|e| anyhow::anyhow!(e))
}

#[cfg(not(feature = "runtime"))]
async fn chat_stage(
    _stage: vox_actor_runtime::llm::cascade::ResearchStage,
    _endpoint: Option<&str>,
    _api_key: Option<&str>,
    _model: &str,
    _temperature: f32,
    _max_tokens: u32,
    _messages: Vec<(String, String)>,
    _response_format: Option<serde_json::Value>,
) -> anyhow::Result<String> {
    anyhow::bail!("research runtime feature is disabled")
}

#[cfg(test)]
mod citation_diversity_tests {
    use super::evaluate_citation_diversity;
    use crate::research::types::ResearchHit;

    #[test]
    fn diversity_gate_flags_insufficient_domains() {
        let hits = vec![
            ResearchHit {
                url: "https://a.example/x".into(),
                title: "a".into(),
                snippet: "s".into(),
                score: 1.0,
                http_status: 0,
                trust_score: 1.0,
                raw_content: String::new(),
            },
            ResearchHit {
                url: "https://a.example/y".into(),
                title: "b".into(),
                snippet: "s".into(),
                score: 1.0,
                http_status: 0,
                trust_score: 1.0,
                raw_content: String::new(),
            },
        ];
        let (count, below) = evaluate_citation_diversity(&hits, 3);
        assert_eq!(count, 1);
        assert!(below);
    }

    #[test]
    fn judge_prompt_has_no_code_generation_boilerplate() {
        let sys_prompt = super::build_judge_system_prompt();
        assert!(
            !sys_prompt.contains("TODO"),
            "judge prompt should not contain code-generation vocabulary: {sys_prompt}"
        );
        assert!(
            sys_prompt.contains("Cite every material claim"),
            "judge prompt should use research-appropriate completeness language: {sys_prompt}"
        );
    }
}
