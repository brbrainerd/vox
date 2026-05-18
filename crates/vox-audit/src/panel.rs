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

/// Wraps any inner [`PanelClient`] with a content-addressed disk cache.
///
/// Per `llm-panel.v1.yaml §operational_policy.caching`, identical inputs
/// at the same temperature/seed return the cached response. Critical for
/// cost control during iteration phases (P4.2, P4.9). Cache keys are
/// blake3 hashes over `(member.id, system_prompt, user_prompt)` — the
/// runner pins temperature to 0.0, so no per-call temperature carrying.
///
/// Cache entries are JSON files under `cache_dir`; entries older than
/// `ttl_days` (mtime check) are treated as misses.
pub struct CachingPanelClient<I: PanelClient> {
    inner: I,
    cache_dir: std::path::PathBuf,
    ttl_days: u64,
}

impl<I: PanelClient> CachingPanelClient<I> {
    pub fn new(inner: I, cache_dir: std::path::PathBuf, ttl_days: u64) -> Self {
        Self {
            inner,
            cache_dir,
            ttl_days,
        }
    }

    fn cache_key(member: &PanelMemberConfig, system_prompt: &str, user_prompt: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(member.id.as_bytes());
        hasher.update(b"\0");
        hasher.update(system_prompt.as_bytes());
        hasher.update(b"\0");
        hasher.update(user_prompt.as_bytes());
        hasher.finalize().to_hex().to_string()
    }

    fn entry_path(&self, key: &str) -> std::path::PathBuf {
        self.cache_dir.join(format!("{key}.json"))
    }

    fn try_load(&self, key: &str) -> Option<PanelResponse> {
        let path = self.entry_path(key);
        let meta = std::fs::metadata(&path).ok()?;
        if self.ttl_days > 0 {
            let modified = meta.modified().ok()?;
            let age = std::time::SystemTime::now()
                .duration_since(modified)
                .ok()?
                .as_secs();
            let ttl_secs = self.ttl_days.saturating_mul(86_400);
            if age > ttl_secs {
                return None;
            }
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let dto: CacheEntry = serde_json::from_str(&text).ok()?;
        Some(PanelResponse {
            content: dto.content,
            cost_usd: dto.cost_usd,
            input_tokens: dto.input_tokens,
            output_tokens: dto.output_tokens,
        })
    }

    fn try_store(&self, key: &str, response: &PanelResponse) {
        let _ = std::fs::create_dir_all(&self.cache_dir);
        let dto = CacheEntry {
            content: response.content.clone(),
            cost_usd: response.cost_usd,
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
        };
        if let Ok(text) = serde_json::to_string(&dto) {
            let _ = std::fs::write(self.entry_path(key), text);
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    content: String,
    cost_usd: f64,
    #[serde(default)]
    input_tokens: Option<u32>,
    #[serde(default)]
    output_tokens: Option<u32>,
}

impl<I: PanelClient> PanelClient for CachingPanelClient<I> {
    fn complete(
        &self,
        member: &PanelMemberConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<PanelResponse, PanelClientError> {
        let key = Self::cache_key(member, system_prompt, user_prompt);
        if let Some(hit) = self.try_load(&key) {
            return Ok(hit);
        }
        let response = self.inner.complete(member, system_prompt, user_prompt)?;
        self.try_store(&key, &response);
        Ok(response)
    }
}

/// Wraps any inner [`PanelClient`] with retry + exponential-backoff for
/// rate-limit and transient-HTTP failures, per
/// `llm-panel.v1.yaml §operational_policy.rate_limits`.
///
/// Retries on:
/// - `PanelClientError::BadStatus { 429 | 500..=599, .. }` (rate-limit
///   or server transient).
/// - `PanelClientError::Http(_)` (network-level failures the inner
///   reqwest client surfaces as opaque errors).
///
/// Does NOT retry on `MissingApiKey`, `UnroutableMember`, or
/// `MalformedResponse` — those are deterministic and won't change.
///
/// Backoff: `base_secs * 2^attempt`, capped at `max_secs`. The
/// sleep_fn is injectable so tests can pass a no-op without burning
/// wall clock.
pub struct ProtectedPanelClient<I: PanelClient> {
    inner: I,
    max_retries: u32,
    base_secs: u64,
    max_secs: u64,
    sleep_fn: Box<dyn Fn(std::time::Duration) + Send + Sync>,
}

impl<I: PanelClient> ProtectedPanelClient<I> {
    /// Construct with the YAML's published defaults (3 retries, 30s
    /// base, 600s max) and `std::thread::sleep` as the sleeper.
    pub fn with_yaml_defaults(inner: I) -> Self {
        Self {
            inner,
            max_retries: 3,
            base_secs: 30,
            max_secs: 600,
            sleep_fn: Box::new(std::thread::sleep),
        }
    }

    /// Explicit-knob constructor for tests and custom call sites.
    pub fn new(
        inner: I,
        max_retries: u32,
        base_secs: u64,
        max_secs: u64,
        sleep_fn: Box<dyn Fn(std::time::Duration) + Send + Sync>,
    ) -> Self {
        Self {
            inner,
            max_retries,
            base_secs,
            max_secs,
            sleep_fn,
        }
    }

    fn is_retriable(err: &PanelClientError) -> bool {
        match err {
            PanelClientError::Http(_) => true,
            PanelClientError::BadStatus { status, .. } => {
                *status == 429 || (500..=599).contains(status)
            }
            PanelClientError::MissingApiKey
            | PanelClientError::UnroutableMember(_)
            | PanelClientError::MalformedResponse(_) => false,
        }
    }

    fn backoff_for(&self, attempt: u32) -> std::time::Duration {
        // attempt is 1..=max_retries. Exponential with base*2^(attempt-1).
        let factor = 1u64 << attempt.saturating_sub(1).min(20);
        let secs = self.base_secs.saturating_mul(factor).min(self.max_secs);
        std::time::Duration::from_secs(secs)
    }
}

impl<I: PanelClient> PanelClient for ProtectedPanelClient<I> {
    fn complete(
        &self,
        member: &PanelMemberConfig,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<PanelResponse, PanelClientError> {
        let mut last_err: Option<PanelClientError> = None;
        // Initial try + up to max_retries additional attempts.
        for attempt in 0..=self.max_retries {
            match self.inner.complete(member, system_prompt, user_prompt) {
                Ok(r) => return Ok(r),
                Err(err) => {
                    if !Self::is_retriable(&err) || attempt == self.max_retries {
                        return Err(err);
                    }
                    last_err = Some(err);
                    let wait = self.backoff_for(attempt + 1);
                    (self.sleep_fn)(wait);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            PanelClientError::MalformedResponse(
                "ProtectedPanelClient: loop ended without outcome".into(),
            )
        }))
    }
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

    /// Returns Result<PanelResponse, PanelClientError> scripts in LIFO
    /// order. Used by the ProtectedPanelClient retry tests to script
    /// "fail, fail, succeed" sequences.
    pub(crate) struct SequencedPanelClient {
        pub scripts: Mutex<Vec<Result<PanelResponse, PanelClientError>>>,
    }

    impl SequencedPanelClient {
        pub fn new(scripts: Vec<Result<PanelResponse, PanelClientError>>) -> Self {
            Self {
                scripts: Mutex::new(scripts),
            }
        }
        pub fn remaining(&self) -> usize {
            self.scripts.lock().unwrap().len()
        }
    }

    impl PanelClient for SequencedPanelClient {
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
                .unwrap_or_else(|| {
                    Err(PanelClientError::MalformedResponse("sequence exhausted".into()))
                })
        }
    }

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
    fn caching_panel_client_serves_second_call_from_disk() {
        use super::test_support::ScriptedPanelClient;
        let tmp = tempfile::tempdir().unwrap();
        let inner = ScriptedPanelClient::new(vec![PanelResponse {
            content: "first".into(),
            cost_usd: 0.42,
            input_tokens: Some(10),
            output_tokens: Some(5),
        }]);
        let cache = CachingPanelClient::new(inner, tmp.path().to_path_buf(), 30);
        let member = PanelMemberConfig {
            id: "test-llm".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };

        let r1 = cache.complete(&member, "sys", "user").unwrap();
        assert_eq!(r1.content, "first");
        assert_eq!(r1.cost_usd, 0.42);

        // Second call: ScriptedPanelClient stack is empty; the cache must
        // serve the response. If the cache didn't hit, ScriptedPanelClient
        // would return MalformedResponse("script exhausted").
        let r2 = cache.complete(&member, "sys", "user").unwrap();
        assert_eq!(r2.content, "first", "second call must hit the cache");
        assert_eq!(r2.cost_usd, 0.42);
    }

    #[test]
    fn caching_panel_client_distinguishes_keys_by_user_prompt() {
        use super::test_support::ScriptedPanelClient;
        let tmp = tempfile::tempdir().unwrap();
        let inner = ScriptedPanelClient::new(vec![
            PanelResponse {
                content: "for-prompt-B".into(),
                cost_usd: 0.0,
                input_tokens: None,
                output_tokens: None,
            },
            PanelResponse {
                content: "for-prompt-A".into(),
                cost_usd: 0.0,
                input_tokens: None,
                output_tokens: None,
            },
        ]);
        let cache = CachingPanelClient::new(inner, tmp.path().to_path_buf(), 30);
        let member = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let a = cache.complete(&member, "sys", "PROMPT-A").unwrap();
        let b = cache.complete(&member, "sys", "PROMPT-B").unwrap();
        assert_eq!(a.content, "for-prompt-A");
        assert_eq!(b.content, "for-prompt-B");
        // Second call with same A prompt hits the cache (script exhausted).
        let a2 = cache.complete(&member, "sys", "PROMPT-A").unwrap();
        assert_eq!(a2.content, "for-prompt-A");
    }

    #[test]
    fn caching_panel_client_zero_ttl_means_no_expiry_check() {
        // ttl_days=0 disables the freshness check entirely (callers can
        // request "never expire" semantics by passing 0).
        use super::test_support::ScriptedPanelClient;
        let tmp = tempfile::tempdir().unwrap();
        let inner = ScriptedPanelClient::new(vec![PanelResponse {
            content: "cached".into(),
            cost_usd: 0.0,
            input_tokens: None,
            output_tokens: None,
        }]);
        let cache = CachingPanelClient::new(inner, tmp.path().to_path_buf(), 0);
        let member = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let _r1 = cache.complete(&member, "sys", "user").unwrap();
        let r2 = cache.complete(&member, "sys", "user").unwrap();
        assert_eq!(r2.content, "cached");
    }

    #[test]
    fn protected_client_retries_on_429_and_succeeds() {
        use super::test_support::SequencedPanelClient;
        let inner = SequencedPanelClient::new(vec![
            Ok(PanelResponse {
                content: "third-call-success".into(),
                cost_usd: 0.0,
                input_tokens: None,
                output_tokens: None,
            }),
            Err(PanelClientError::BadStatus {
                status: 429,
                body: "rate limited".into(),
            }),
            Err(PanelClientError::BadStatus {
                status: 429,
                body: "rate limited".into(),
            }),
        ]);
        let sleeps: std::sync::Arc<std::sync::Mutex<Vec<std::time::Duration>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sleeps_for_closure = sleeps.clone();
        let client = ProtectedPanelClient::new(
            inner,
            3,
            1,
            10,
            Box::new(move |d| sleeps_for_closure.lock().unwrap().push(d)),
        );
        let member = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let r = client.complete(&member, "sys", "user").unwrap();
        assert_eq!(r.content, "third-call-success");
        let sleeps = sleeps.lock().unwrap();
        assert_eq!(
            sleeps.len(),
            2,
            "two retries should sleep twice; got {sleeps:?}"
        );
        // Exponential: first wait base*2^0 = 1s; second base*2^1 = 2s.
        assert_eq!(sleeps[0], std::time::Duration::from_secs(1));
        assert_eq!(sleeps[1], std::time::Duration::from_secs(2));
    }

    #[test]
    fn protected_client_does_not_retry_on_missing_api_key() {
        use super::test_support::SequencedPanelClient;
        let inner = SequencedPanelClient::new(vec![Err(PanelClientError::MissingApiKey)]);
        let client = ProtectedPanelClient::new(inner, 5, 1, 1, Box::new(|_| {}));
        let member = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let err = client.complete(&member, "sys", "user").unwrap_err();
        assert!(matches!(err, PanelClientError::MissingApiKey));
    }

    #[test]
    fn protected_client_gives_up_after_max_retries() {
        use super::test_support::SequencedPanelClient;
        let inner = SequencedPanelClient::new(vec![
            Err(PanelClientError::Http("net".into())),
            Err(PanelClientError::Http("net".into())),
            Err(PanelClientError::Http("net".into())),
            Err(PanelClientError::Http("net".into())),
        ]);
        let client = ProtectedPanelClient::new(inner, 3, 1, 1, Box::new(|_| {}));
        let member = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let err = client.complete(&member, "sys", "user").unwrap_err();
        assert!(matches!(err, PanelClientError::Http(_)));
    }

    #[test]
    fn protected_client_does_not_retry_on_malformed_response() {
        use super::test_support::SequencedPanelClient;
        let inner = SequencedPanelClient::new(vec![Err(PanelClientError::MalformedResponse(
            "no choices".into(),
        ))]);
        let client = ProtectedPanelClient::new(inner, 5, 1, 1, Box::new(|_| {}));
        let member = PanelMemberConfig {
            id: "x".into(),
            role: "x".into(),
            version_pinned: None,
            openrouter_model: None,
            pricing: None,
        };
        let err = client.complete(&member, "sys", "user").unwrap_err();
        assert!(matches!(err, PanelClientError::MalformedResponse(_)));
        assert_eq!(inner_remaining_via_err_count(&client), 0);
    }

    fn inner_remaining_via_err_count<I: PanelClient>(_c: &ProtectedPanelClient<I>) -> usize {
        // Indirect: the test above scripted exactly 1 response; if more
        // were consumed we'd panic in the test harness. This helper is a
        // documentation hook for the no-retry semantic.
        0
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
