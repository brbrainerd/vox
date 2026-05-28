//! Claim extraction for research answers and evidence.

use serde::{Deserialize, Serialize};

/// One extracted research claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// The claim text itself.
    pub text: String,
    /// Stable hash assigned downstream (FNV-1a of `text`).
    pub claim_id: u64,
    /// Heuristic flag: claim contains a numeric value.
    pub is_numeric: bool,
    /// Heuristic flag: claim mentions a recent date or "recently" / "latest".
    pub is_recent: bool,
    /// Heuristic flag: claim mentions a named entity / event.
    pub is_named_event: bool,
}

/// Extract claims from a query.
///
/// **PHASE_0a_STUB**: returns `Vec::new()`. No LLM invocation. Phase 1 wires
/// this to `vox-claim-extractor`.
///
/// # Parameters
/// - `_query`: the source text (in Phase 0a, this is the user query; Phase 1
///   will accept arbitrary documents).
/// - `_endpoint`, `_api_key`, `_model`, `_max_tokens`: ignored in Phase 0a.
pub async fn extract_claims_with_model(
    query: &str,
    endpoint: Option<&str>,
    api_key: Option<&str>,
    model: Option<&str>,
    max_tokens: Option<u32>,
) -> Vec<Claim> {
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
        let mut candidates = cascade_with_optional_manual(
            ResearchStage::ClaimExtraction,
            &input,
            endpoint,
            api_key,
            model,
        );
        for candidate in &mut candidates {
            candidate.temperature = Some(0.0);
            candidate.max_tokens = Some(u64::from(max_tokens.unwrap_or(900)));
            candidate.response_format = Some(serde_json::json!({"type": "json_object"}));
        }

        let messages = vec![
            LlmChatMessage {
                role: "system".to_string(),
                content: "Extract atomic, independently verifiable factual claims. \
                    Output only valid JSON. Use either {\"claims\": [...]} or a bare array. \
                    Each claim object must include text, is_numeric, is_recent, is_named_event."
                    .to_string(),
            },
            LlmChatMessage {
                role: "user".to_string(),
                content: query.to_string(),
            },
        ];
        let opts = ActivityOptions::new().with_timeout_secs(30);
        match chat_with_cascade(&opts, messages, candidates, None).await {
            Ok(response) => match parse_claims_response(&response.content) {
                Ok(claims) => return claims,
                Err(e) => {
                    tracing::warn!(error = %e, "research claim extraction response was invalid")
                }
            },
            Err(e) => tracing::warn!(error = %e, "research claim extraction cascade failed"),
        }
    }

    Vec::new()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ClaimsPayload {
    Array(Vec<ClaimPayload>),
    Object { claims: Vec<ClaimPayload> },
}

#[derive(Deserialize)]
struct ClaimPayload {
    text: String,
    #[serde(default)]
    is_numeric: bool,
    #[serde(default)]
    is_recent: bool,
    #[serde(default)]
    is_named_event: bool,
}

fn parse_claims_response(response: &str) -> anyhow::Result<Vec<Claim>> {
    let payload: ClaimsPayload = super::json_parse::parse_json_response(response)?;
    let claims = match payload {
        ClaimsPayload::Array(claims) | ClaimsPayload::Object { claims } => claims,
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for claim in claims {
        let text = claim.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }
        let key = text.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(Claim {
            claim_id: fnv1a_hash(&text),
            text,
            is_numeric: claim.is_numeric,
            is_recent: claim.is_recent,
            is_named_event: claim.is_named_event,
        });
    }
    Ok(out)
}

fn fnv1a_hash(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut hash = FNV_OFFSET;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_claims_response_accepts_json_codeblock_and_stable_shape() {
        let response = r#"```json
        [
          {
            "text": "CRAG performs corrective retrieval when evidence quality is weak.",
            "is_numeric": false,
            "is_recent": false,
            "is_named_event": true
          },
          {
            "text": "The eval suite contains 30 golden queries.",
            "is_numeric": true,
            "is_recent": false,
            "is_named_event": false
          }
        ]
        ```"#;

        let claims = parse_claims_response(response).expect("claims parse");

        assert_eq!(claims.len(), 2);
        assert_eq!(
            claims[0].text,
            "CRAG performs corrective retrieval when evidence quality is weak."
        );
        assert!(claims[1].is_numeric);
        assert_ne!(claims[0].claim_id, 0);
        assert_ne!(claims[0].claim_id, claims[1].claim_id);
    }

    #[test]
    fn parse_claims_response_ignores_blank_claims() {
        let response =
            r#"[{"text":"   ","is_numeric":false,"is_recent":false,"is_named_event":false}]"#;

        let claims = parse_claims_response(response).expect("claims parse");

        assert!(claims.is_empty());
    }
}
