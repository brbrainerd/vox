//! Reference-LLM-panel client surface for CR-L0..CR-L4 measurement.
//!
//! Defines a [`PanelClient`] trait abstracting over LLM-call backends and
//! ships one real implementation: [`OpenRouterPanelClient`]. Tests use
//! per-test in-memory implementations of the trait — there is no
//! production stub.
//!
//! Panel members are pinned in
//! [`contracts/eval/llm-panel.v1.yaml`](../../../../contracts/eval/llm-panel.v1.yaml).
//! This module loads that file and routes each member to a concrete
//! OpenRouter model id.

use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

/// One panel member parsed from `llm-panel.v1.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PanelMemberConfig {
    pub id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub version_pinned: Option<String>,
    /// Optional explicit OpenRouter model slug (e.g. `anthropic/claude-sonnet-4-6`).
    /// When absent, [`PanelMemberConfig::openrouter_model_id`] derives it from
    /// `version_pinned` via the standard `<vendor>/<id>` convention.
    #[serde(default)]
    pub openrouter_model: Option<String>,
    #[serde(default)]
    pub pricing: Option<PricingConfig>,
}

impl PanelMemberConfig {
    /// Resolve the OpenRouter model slug for this member.
    ///
    /// Order:
    /// 1. Explicit `openrouter_model:` field if set.
    /// 2. Derived from `version_pinned`: `claude-*` → `anthropic/<pin>`,
    ///    `gpt-*` → `openai/<pin>`, otherwise pass through unchanged.
    /// 3. None for members that aren't reachable via OpenRouter (e.g.
    ///    project-owned MENS).
    pub fn openrouter_model_id(&self) -> Option<String> {
        if let Some(explicit) = &self.openrouter_model {
            return Some(explicit.clone());
        }
        let pin = self.version_pinned.as_deref()?;
        if pin.starts_with("claude-") {
            Some(format!("anthropic/{pin}"))
        } else if pin.starts_with("gpt-") {
            Some(format!("openai/{pin}"))
        } else if self.role == "project-owned" {
            // MENS / project-owned: not OpenRouter-routable. Caller decides
            // whether to skip or use a different backend.
            None
        } else {
            Some(pin.to_string())
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PricingConfig {
    #[serde(default)]
    pub input_per_million_tokens_usd: Option<f64>,
    #[serde(default)]
    pub output_per_million_tokens_usd: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PanelConfig {
    pub panel: PanelMetadata,
    pub members: Vec<PanelMemberConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PanelMetadata {
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub pinned_at: Option<String>,
}

impl PanelConfig {
    pub fn from_yaml_path(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read panel YAML {}: {e}", path.display()))?;
        serde_yaml::from_str(&text)
            .map_err(|e| format!("malformed panel YAML at {}: {e}", path.display()))
    }
}

/// One LLM round trip.
#[derive(Debug, Clone)]
pub struct PanelResponse {
    /// Raw text content returned by the model.
    pub content: String,
    /// Best-effort cost estimate based on member pricing × token usage.
    pub cost_usd: f64,
    /// Input + output token counts reported by the provider, when available.
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum PanelClientError {
    #[error("panel member `{0}` is not OpenRouter-routable (no openrouter_model_id)")]
    UnroutableMember(String),
    #[error("OpenRouter API key not configured (set VOX_OPENROUTER_API_KEY)")]
    MissingApiKey,
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("non-success status {status}: {body}")]
    BadStatus { status: u16, body: String },
    #[error("malformed response: {0}")]
    MalformedResponse(String),
}

/// Abstract LLM-completion surface for panel-based measurement.
///
/// Implementations: [`OpenRouterPanelClient`] for live HTTP calls; tests
/// in this module supply their own deterministic impls for orchestration
/// coverage.
pub trait PanelClient: Send + Sync {
    /// Send `prompt` to `member` and return the completion. Synchronous on
    /// the calling thread — `vox audit` runs are sequential per-fixture
    /// per-member by design (rate-limits, cost ceiling, reproducibility).
    fn complete(
        &self,
        member: &PanelMemberConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<PanelResponse, PanelClientError>;
}

/// Live OpenRouter-backed panel client.
pub struct OpenRouterPanelClient {
    api_key: String,
    http: reqwest::blocking::Client,
}

impl OpenRouterPanelClient {
    /// Construct from `VOX_OPENROUTER_API_KEY` / `OPENROUTER_API_KEY` via
    /// [`vox_config::inference::openrouter_api_key`]. Returns
    /// [`PanelClientError::MissingApiKey`] when neither is set.
    pub fn from_env() -> Result<Self, PanelClientError> {
        let api_key = vox_config::inference::openrouter_api_key()
            .ok_or(PanelClientError::MissingApiKey)?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| PanelClientError::Http(e.to_string()))?;
        Ok(Self { api_key, http })
    }
}

impl PanelClient for OpenRouterPanelClient {
    fn complete(
        &self,
        member: &PanelMemberConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<PanelResponse, PanelClientError> {
        let model = member
            .openrouter_model_id()
            .ok_or_else(|| PanelClientError::UnroutableMember(member.id.clone()))?;

        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ],
            "temperature": 0.0,
        });

        let resp = self
            .http
            .post(vox_config::inference::OPENROUTER_CHAT_COMPLETIONS_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Title", "Vox Audit Panel")
            .json(&body)
            .send()
            .map_err(|e| PanelClientError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(PanelClientError::BadStatus {
                status: status.as_u16(),
                body,
            });
        }

        #[derive(Deserialize)]
        struct OuterResp {
            choices: Vec<Choice>,
            #[serde(default)]
            usage: Option<Usage>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: ChoiceMessage,
        }
        #[derive(Deserialize)]
        struct ChoiceMessage {
            content: String,
        }
        #[derive(Deserialize)]
        struct Usage {
            #[serde(default)]
            prompt_tokens: Option<u32>,
            #[serde(default)]
            completion_tokens: Option<u32>,
        }

        let parsed: OuterResp = resp
            .json()
            .map_err(|e| PanelClientError::MalformedResponse(e.to_string()))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| PanelClientError::MalformedResponse("no choices in response".into()))?
            .message
            .content;

        let (input_tokens, output_tokens) = parsed
            .usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or((None, None));

        let cost_usd = estimate_cost(member, input_tokens, output_tokens);

        Ok(PanelResponse {
            content,
            cost_usd,
            input_tokens,
            output_tokens,
        })
    }
}

fn estimate_cost(member: &PanelMemberConfig, input: Option<u32>, output: Option<u32>) -> f64 {
    let Some(pricing) = &member.pricing else {
        return 0.0;
    };
    let input_cost = match (pricing.input_per_million_tokens_usd, input) {
        (Some(rate), Some(tokens)) => rate * f64::from(tokens) / 1_000_000.0,
        _ => 0.0,
    };
    let output_cost = match (pricing.output_per_million_tokens_usd, output) {
        (Some(rate), Some(tokens)) => rate * f64::from(tokens) / 1_000_000.0,
        _ => 0.0,
    };
    input_cost + output_cost
}

/// Extract a Vox source code block from a model response.
///
/// Looks for the first fenced block tagged ```vox / ```vox\n / ``` (untagged
/// fallback). Falls back to the entire content if no fence is present.
pub fn extract_vox_code(content: &str) -> String {
    if let Some(start) = content.find("```vox") {
        let after_fence = &content[start + "```vox".len()..];
        // Skip the rest of the opening-fence line.
        let after_newline = after_fence
            .find('\n')
            .map_or(after_fence, |idx| &after_fence[idx + 1..]);
        if let Some(end) = after_newline.find("```") {
            return after_newline[..end].trim_end().to_string();
        }
        return after_newline.trim_end().to_string();
    }
    if let Some(start) = content.find("```") {
        let after_fence = &content[start + 3..];
        let after_newline = after_fence
            .find('\n')
            .map_or(after_fence, |idx| &after_fence[idx + 1..]);
        if let Some(end) = after_newline.find("```") {
            return after_newline[..end].trim_end().to_string();
        }
        return after_newline.trim_end().to_string();
    }
    content.trim().to_string()
}

/// Test-only deterministic [`PanelClient`] implementations for orchestration
/// coverage. Compiled out of release builds.
#[cfg(test)]
pub(crate) mod test_support {
    use super::{PanelClient, PanelClientError, PanelMemberConfig, PanelResponse};
    use std::sync::Mutex;

    /// Returns canned [`PanelResponse`]s in LIFO order. The trait impl is real
    /// (reads bytes, returns them); no production code path uses it.
    pub(crate) struct ScriptedPanelClient {
        pub scripts: Mutex<Vec<PanelResponse>>,
    }

    impl ScriptedPanelClient {
        pub fn new(scripts: Vec<PanelResponse>) -> Self {
            Self {
                scripts: Mutex::new(scripts),
            }
        }
    }

    impl PanelClient for ScriptedPanelClient {
        fn complete(
            &self,
            _member: &PanelMemberConfig,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> Result<PanelResponse, PanelClientError> {
            self.scripts
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| PanelClientError::MalformedResponse("script exhausted".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openrouter_model_id_derives_anthropic_prefix_for_claude_pin() {
        let m = PanelMemberConfig {
            id: "claude-sonnet".into(),
            role: "frontier-baseline".into(),
            version_pinned: Some("claude-sonnet-4-6".into()),
            openrouter_model: None,
            pricing: None,
        };
        assert_eq!(
            m.openrouter_model_id().as_deref(),
            Some("anthropic/claude-sonnet-4-6")
        );
    }

    #[test]
    fn openrouter_model_id_derives_openai_prefix_for_gpt_pin() {
        let m = PanelMemberConfig {
            id: "gpt-frontier".into(),
            role: "frontier-baseline".into(),
            version_pinned: Some("gpt-5.4".into()),
            openrouter_model: None,
            pricing: None,
        };
        assert_eq!(m.openrouter_model_id().as_deref(), Some("openai/gpt-5.4"));
    }

    #[test]
    fn openrouter_model_id_honors_explicit_field() {
        let m = PanelMemberConfig {
            id: "custom".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: Some("vendor/custom-model".into()),
            pricing: None,
        };
        assert_eq!(
            m.openrouter_model_id().as_deref(),
            Some("vendor/custom-model")
        );
    }

    #[test]
    fn openrouter_model_id_returns_none_for_project_owned_mens() {
        let m = PanelMemberConfig {
            id: "mens-current".into(),
            role: "project-owned".into(),
            version_pinned: Some("0.5.0".into()),
            openrouter_model: None,
            pricing: None,
        };
        assert!(m.openrouter_model_id().is_none());
    }

    #[test]
    fn panel_config_parses_real_workspace_yaml() {
        let path = crate::workspace_root().join("contracts/eval/llm-panel.v1.yaml");
        let cfg = PanelConfig::from_yaml_path(&path).expect("workspace panel YAML loads");
        assert_eq!(cfg.panel.id, "vox-v1-reference-panel");
        assert!(
            cfg.members.iter().any(|m| m.id == "claude-sonnet"),
            "expected claude-sonnet in panel"
        );
        assert!(
            cfg.members.iter().any(|m| m.id == "mens-current"),
            "expected mens-current in panel"
        );
    }

    #[test]
    fn estimate_cost_uses_pricing_and_token_counts() {
        let m = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: Some(PricingConfig {
                input_per_million_tokens_usd: Some(3.00),
                output_per_million_tokens_usd: Some(15.00),
            }),
        };
        let cost = estimate_cost(&m, Some(1_000_000), Some(100_000));
        // 1 MTok input @ $3 + 0.1 MTok output @ $15 = $3.00 + $1.50 = $4.50
        assert!((cost - 4.50).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn extract_vox_code_picks_first_vox_fence() {
        let resp = "Here's the fix:\n```vox\nfn ok() to int { return 1 }\n```\nDone.";
        let code = extract_vox_code(resp);
        assert_eq!(code.trim(), "fn ok() to int { return 1 }");
    }

    #[test]
    fn extract_vox_code_falls_back_to_untagged_fence() {
        let resp = "```\nfn ok() to int { return 1 }\n```";
        let code = extract_vox_code(resp);
        assert_eq!(code.trim(), "fn ok() to int { return 1 }");
    }

    #[test]
    fn extract_vox_code_returns_whole_content_when_no_fence() {
        let resp = "fn raw() to int { return 1 }";
        let code = extract_vox_code(resp);
        assert_eq!(code.trim(), "fn raw() to int { return 1 }");
    }

    #[test]
    fn scripted_client_returns_scripts_in_lifo_order() {
        use super::test_support::ScriptedPanelClient;
        let client = ScriptedPanelClient::new(vec![
            PanelResponse {
                content: "second".into(),
                cost_usd: 0.0,
                input_tokens: None,
                output_tokens: None,
            },
            PanelResponse {
                content: "first".into(),
                cost_usd: 0.0,
                input_tokens: None,
                output_tokens: None,
            },
        ]);
        let m = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let r1 = client.complete(&m, "sys", "user").unwrap();
        let r2 = client.complete(&m, "sys", "user").unwrap();
        assert_eq!(r1.content, "first");
        assert_eq!(r2.content, "second");
        assert!(client.complete(&m, "sys", "user").is_err());
    }
}
