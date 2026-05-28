use std::pin::Pin;

use futures_util::{Stream, StreamExt};

use crate::ai::constants::*;
use crate::ai::error::AiError;
use crate::ai::fallback::deterministic_response;
use crate::ai::keys::{resolve_gemini_key, resolve_openrouter_key};
use crate::ai::provider::FreeAiProvider;

use super::{AiReportFn, CostReportFn, FreeAiClient, LudusStreamBackend, StreamRoute};

impl FreeAiClient {
    /// Create a client with an explicit provider list.
    pub fn new(providers: Vec<FreeAiProvider>) -> Self {
        let http = vox_http_client::client_builder()
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| vox_http_client::client());
        Self {
            providers,
            http,
            reporter: None,
            cost_reporter: None,
        }
    }

    /// Set a reporter to receive provider events.
    pub fn with_reporter(mut self, reporter: AiReportFn) -> Self {
        self.reporter = Some(reporter);
        self
    }

    /// Set a cost reporter.
    pub fn with_cost_reporter(mut self, cost_reporter: CostReportFn) -> Self {
        self.cost_reporter = Some(cost_reporter);
        self
    }

    /// Auto-discover available providers and build a fallback chain.
    ///
    /// Order: Ollama (if reachable) → Pollinations → Gemini (if key set) → Deterministic.
    pub async fn auto_discover() -> Self {
        let mut providers = Vec::new();

        // 1. Probe Ollama
        if Self::probe_ollama(OLLAMA_DEFAULT_URL).await {
            providers.push(FreeAiProvider::Ollama {
                url: OLLAMA_DEFAULT_URL.to_string(),
                model: OLLAMA_DEFAULT_MODEL.to_string(),
            });
        }

        // 2. Pollinations is always available (network permitting)
        providers.push(FreeAiProvider::Pollinations);

        // 3. Gemini if API key is set (Clavis + documented env aliases)
        {
            let key = resolve_gemini_key("");
            if !key.is_empty() {
                providers.push(FreeAiProvider::Gemini {
                    api_key: key,
                    model: GEMINI_DEFAULT_MODEL.to_string(),
                });
            }
        }

        // 4. OpenRouter free tier if key is set
        {
            let key = resolve_openrouter_key("");
            if !key.is_empty() {
                providers.push(FreeAiProvider::OpenRouter {
                    api_key: key,
                    models: Vec::new(), // use default free model list
                });
            }
        }

        // 5. Deterministic always last — never fails
        providers.push(FreeAiProvider::Deterministic);

        Self::new(providers)
    }

    /// Return the `(provider, model)` for the highest-priority active provider.
    ///
    /// Used for cost/telemetry tagging. Falls back to `("deterministic", "none")`.
    pub fn active_provider_info(&self) -> (String, String) {
        self.providers
            .first()
            .map(|p| p.provider_and_model())
            .unwrap_or_else(|| ("deterministic".to_string(), "none".to_string()))
    }

    /// Check if Ollama is reachable at the given URL.
    pub(crate) async fn probe_ollama(url: &str) -> bool {
        let probe_client = vox_http_client::client_builder()
            .timeout(std::time::Duration::from_secs(OLLAMA_PROBE_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| vox_http_client::client());
        probe_client
            .get(format!("{}/api/version", url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Generate text using the fallback chain.
    ///
    /// Tries each provider in order. Returns the first successful response.
    /// If all providers fail, returns the deterministic fallback.
    pub async fn generate(&self, prompt: &str) -> Result<String, AiError> {
        let mut _last_error = String::new();

        for provider in &self.providers {
            match self.call_provider(provider, prompt).await {
                Ok(text) if !text.trim().is_empty() => return Ok(text),
                Err(AiError::RateLimited {
                    provider,
                    retry_after_secs,
                }) => {
                    if let Some(ref r) = self.reporter {
                        r(&provider, retry_after_secs);
                    }
                    _last_error = format!("{}: rate limited", provider);
                    continue;
                }
                Ok(_) => {
                    _last_error = format!("{}: empty response", provider.name());
                }
                Err(e) => {
                    _last_error = format!("{}: {}", provider.name(), e);
                }
            }
        }

        // Should not reach here if Deterministic is in the chain, but just in case:
        Ok(deterministic_response(prompt))
    }

    fn cascade_stream(
        providers: Vec<FreeAiProvider>,
        http: reqwest::Client,
        prompt: String,
        reporter: Option<AiReportFn>,
        cost_reporter: Option<CostReportFn>,
    ) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
        Box::pin(async_stream::try_stream! {
            for provider in providers {
                match provider {
                    FreeAiProvider::Ollama { ref url, ref model } => {
                        let mut stream = Self::stream_ollama(&http, url, model, &prompt).await;
                        let mut saw_rate_limit = false;
                        while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(t) => yield t,
                                Err(AiError::RateLimited { provider, retry_after_secs }) => {
                                    if let Some(ref r) = reporter {
                                        r(&provider, retry_after_secs);
                                    }
                                    saw_rate_limit = true;
                                    break;
                                }
                                Err(e) => Err(e)?,
                            }
                        }
                        if !saw_rate_limit {
                            return;
                        }
                    }
                    FreeAiProvider::Gemini { ref api_key, ref model } => {
                        let mut stream = Self::stream_gemini(&http, api_key, model, &prompt).await;
                        let mut saw_rate_limit = false;
                        while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(t) => yield t,
                                Err(AiError::RateLimited { provider, retry_after_secs }) => {
                                    if let Some(ref r) = reporter {
                                        r(&provider, retry_after_secs);
                                    }
                                    saw_rate_limit = true;
                                    break;
                                }
                                Err(e) => Err(e)?,
                            }
                        }
                        if !saw_rate_limit {
                            return;
                        }
                    }
                    FreeAiProvider::OpenRouter { ref api_key, ref models } => {
                        let model_list: Vec<String> = if models.is_empty() {
                            OPENROUTER_FREE_MODELS
                                .iter()
                                .map(|s| (*s).to_string())
                                .collect()
                        } else {
                            models.clone()
                        };
                        'or_try: for m in model_list {
                            let mut stream =
                                Self::stream_openrouter(&http, api_key, &m, &prompt, cost_reporter.clone());
                            let mut saw_rate_limit = false;
                            let mut yielded = false;
                            while let Some(chunk) = stream.next().await {
                                match chunk {
                                    Ok(t) => {
                                        yielded = true;
                                        yield t;
                                    }
                                    Err(AiError::RateLimited {
                                        provider,
                                        retry_after_secs,
                                    }) => {
                                        if let Some(ref r) = reporter {
                                            r(&provider, retry_after_secs);
                                        }
                                        saw_rate_limit = true;
                                        break;
                                    }
                                    Err(_) => break,
                                }
                            }
                            if saw_rate_limit {
                                continue 'or_try;
                            }
                            if yielded {
                                return;
                            }
                        }
                    }
                    _ => {
                        match Self::call_provider_static(&http, &provider, &prompt).await {
                            Ok(text) if !text.trim().is_empty() => {
                                yield text;
                                return;
                            }
                            Err(AiError::RateLimited { provider, retry_after_secs }) => {
                                if let Some(ref r) = reporter {
                                    r(&provider, retry_after_secs);
                                }
                                continue;
                            }
                            _ => continue,
                        }
                    }
                }
            }
            yield deterministic_response(&prompt);
        })
    }

    fn gemini_key_from_providers(providers: &[FreeAiProvider]) -> String {
        for p in providers {
            if let FreeAiProvider::Gemini { api_key, .. } = p {
                if !api_key.is_empty() {
                    return api_key.clone();
                }
            }
        }
        resolve_gemini_key("")
    }

    fn ollama_base_from_providers(providers: &[FreeAiProvider]) -> String {
        for p in providers {
            if let FreeAiProvider::Ollama { url, .. } = p {
                if !url.is_empty() {
                    return url.clone();
                }
            }
        }
        OLLAMA_DEFAULT_URL.to_string()
    }

    fn openrouter_key_from_providers(providers: &[FreeAiProvider]) -> String {
        for p in providers {
            if let FreeAiProvider::OpenRouter { api_key, .. } = p {
                if !api_key.is_empty() {
                    return api_key.clone();
                }
            }
        }
        resolve_openrouter_key("")
    }

    /// Generate a stream of tokens.
    ///
    /// Cascades through providers. If a provider doesn't support streaming,
    /// it will be called as a single block and yielded as a single chunk.
    pub async fn generate_stream(
        &self,
        prompt: &str,
    ) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
        Self::cascade_stream(
            self.providers.clone(),
            self.http.clone(),
            prompt.to_string(),
            self.reporter.clone(),
            self.cost_reporter.clone(),
        )
    }

    /// Like [`Self::generate_stream`], but can target a specific backend + model or honor a user override.
    pub async fn generate_stream_routed(
        &self,
        prompt: &str,
        route: StreamRoute<'_>,
    ) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
        let http = self.http.clone();
        let prompt_owned = prompt.to_string();
        let providers = self.providers.clone();
        let reporter = self.reporter.clone();
        let cost_reporter = self.cost_reporter.clone();

        match route {
            StreamRoute::Cascade => {
                Self::cascade_stream(providers, http, prompt_owned, reporter, cost_reporter)
            }
            StreamRoute::Registry {
                backend: LudusStreamBackend::Ollama,
                model,
            } => {
                let url = Self::ollama_base_from_providers(&providers);
                let model = model.to_string();
                Box::pin(async_stream::try_stream! {
                    let mut stream = Self::stream_ollama(&http, &url, &model, &prompt_owned).await;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(t) => yield t,
                            Err(e) => Err(e)?,
                        }
                    }
                })
            }
            StreamRoute::Registry {
                backend: LudusStreamBackend::Gemini,
                model,
            } => {
                let api_key = Self::gemini_key_from_providers(&providers);
                if api_key.is_empty() {
                    return Self::cascade_stream(
                        providers,
                        http,
                        prompt_owned,
                        reporter,
                        cost_reporter,
                    );
                }
                let model = model.to_string();
                Box::pin(async_stream::try_stream! {
                    let mut stream =
                        Self::stream_gemini(&http, &api_key, &model, &prompt_owned).await;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(t) => yield t,
                            Err(e) => Err(e)?,
                        }
                    }
                })
            }
            StreamRoute::Registry {
                backend: LudusStreamBackend::OpenRouter,
                model,
            } => {
                let api_key = Self::openrouter_key_from_providers(&providers);
                if api_key.is_empty() {
                    return Self::cascade_stream(
                        providers,
                        http,
                        prompt_owned,
                        reporter,
                        cost_reporter,
                    );
                }
                let model = model.to_string();
                Box::pin(async_stream::try_stream! {
                    let mut stream =
                        Self::stream_openrouter(&http, &api_key, &model, &prompt_owned, cost_reporter);
                    let mut any = false;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(t) => {
                                any = true;
                                yield t;
                            }
                            Err(e) => Err(e)?,
                        }
                    }
                    if !any {
                        yield deterministic_response(&prompt_owned);
                    }
                })
            }
            StreamRoute::UserModelOverride(model) => {
                let model = model.to_string();
                Box::pin(async_stream::try_stream! {
                    let url = Self::ollama_base_from_providers(&providers);
                    let mut stream = Self::stream_ollama(&http, &url, &model, &prompt_owned).await;
                    let mut any = false;
                    let mut rate_limited = false;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(t) => {
                                any = true;
                                yield t;
                            }
                            Err(AiError::RateLimited { provider, retry_after_secs }) => {
                                if let Some(ref r) = reporter {
                                    r(&provider, retry_after_secs);
                                }
                                rate_limited = true;
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                    if any && !rate_limited {
                        return;
                    }

                    let or_key = Self::openrouter_key_from_providers(&providers);
                    if !or_key.is_empty() {
                        let mut stream =
                            Self::stream_openrouter(&http, &or_key, &model, &prompt_owned, cost_reporter.clone());
                        let mut any_or = false;
                        while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(t) => {
                                    any_or = true;
                                    yield t;
                                }
                                Err(AiError::RateLimited { .. }) => break,
                                Err(_) => break,
                            }
                        }
                        if any_or {
                            return;
                        }
                    }

                    let gem_key = Self::gemini_key_from_providers(&providers);
                    if !gem_key.is_empty() {
                        let mut stream =
                            Self::stream_gemini(&http, &gem_key, &model, &prompt_owned).await;
                        while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(t) => yield t,
                                Err(e) => Err(e)?,
                            }
                        }
                        return;
                    }

                    let mut fallback = Self::cascade_stream(
                        providers,
                        http,
                        prompt_owned,
                        reporter,
                        cost_reporter,
                    );
                    while let Some(item) = fallback.next().await {
                        match item {
                            Ok(t) => yield t,
                            Err(e) => Err(e)?,
                        }
                    }
                })
            }
        }
    }
}
