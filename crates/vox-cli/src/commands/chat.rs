//! `vox chat` — Native interactive AI chat command.
//!
//! Replaces the legacy OpenCode bridge with a native first-class,
//! provider-agnostic, free-tier-first chat interface.
//!
//! # Zero-Friction Provider Cascade
//!
//! The chat command uses a three-layer provider cascade designed so that
//! the user is **never blocked**:
//!
//! 1. **Google AI Studio (free, no credit card)** — primary. Just needs a
//!    Google account to get an API key from <https://aistudio.google.com/apikey>.
//!    Provides Gemini 2.5 Pro/Flash/Flash-Lite at zero cost.
//!
//! 2. **OpenRouter (free key, `:free` models)** — secondary. If the user has
//!    an OpenRouter API key, unlocks dozens of free models (Devstral 2,
//!    Qwen3 Coder, Llama 4 Scout, Kimi K2, etc.).
//!
//! 3. **OpenRouter (paid SOTA)** — optional upgrade. When budget is configured,
//!    auto-selects the best model for the task: DeepSeek v3.2, Claude Sonnet 4.5,
//!    GPT-5, etc.
//!
//! 4. **Ollama (local)** — always-available zero-auth fallback if running locally.
//!
//! # Usage
//! ```text
//! vox chat                         # auto-selects best free model
//! vox chat --model gemini-2.5-pro  # explicit model
//! vox chat --free                  # force free only
//! vox chat --session abc123        # resume previous session
//! ```

use anyhow::Result;
use serde_json::json;

/// Default Google AI Studio model — fastest free model, no credit card needed.
const DEFAULT_GOOGLE_MODEL: &str = "gemini-2.5-flash-preview";

/// Free-tier escalation chain (Google AI Studio direct).
const GOOGLE_ESCALATION: &[&str] = &[
    "gemini-2.0-flash-lite",     // Fast, 1000 RPD
    "gemini-2.5-flash-preview",  // Better, 250 RPD
    "gemini-2.5-pro",            // Best free, 100 RPD
];

/// Free-tier escalation chain (OpenRouter :free models, requires OR key).
const OPENROUTER_FREE_ESCALATION: &[&str] = &[
    "mistral/devstral-2-2512:free",                        // Agentic coding
    "qwen/qwen3-coder:free",                               // Code generation
    "meta-llama/llama-4-scout:free",                       // Vision, general
    "moonshotai/kimi-k2:free",                             // Tool use
    "meta-llama/llama-3.3-70b-instruct:free",              // General
];

/// Google AI Studio API endpoint template.
const GOOGLE_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// OpenRouter API endpoint.
const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Detected provider availability.
struct ProviderState {
    google_key: Option<String>,
    openrouter_key: Option<String>,
    ollama_available: bool,
}

impl ProviderState {
    fn detect() -> Self {
        // Priority: env var > ~/.vox/auth.json (via vox login)
        let google_key = std::env::var("GEMINI_API_KEY")
            .or_else(|_| std::env::var("GOOGLE_AI_STUDIO_KEY"))
            .ok()
            .or_else(|| super::login::get_auth("google").map(|a| a.token));

        let openrouter_key = std::env::var("OPENROUTER_API_KEY")
            .ok()
            .or_else(|| super::login::get_auth("openrouter").map(|a| a.token));

        // Quick check: is Ollama running on default port?
        let ollama_available = std::net::TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], 11434)),
            std::time::Duration::from_millis(200),
        )
        .is_ok();

        Self {
            google_key,
            openrouter_key,
            ollama_available,
        }
    }

    fn best_provider(&self) -> &str {
        if self.google_key.is_some() {
            "google"
        } else if self.openrouter_key.is_some() {
            "openrouter"
        } else if self.ollama_available {
            "ollama"
        } else {
            "none"
        }
    }
}

/// Helper to generate a random 6-hex session ID
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_micros();
    format!("{:06x}", ms)
}

/// Run the native Vox chat command.
pub async fn run(
    model: Option<&str>,
    free: bool,
    session: Option<&str>,
) -> Result<()> {
    let mut providers = ProviderState::detect();

    // Try to connect to VoxDB to enable budget routing and history
    let db = vox_db::VoxDb::connect_default().await.ok();
    let tracker = db.as_ref().map(|d| vox_orchestrator::usage::UsageTracker::new_ref(d));

    // Display startup banner
    println!();
    println!("  \x1b[1;36m╔══════════════════════════════════════════╗\x1b[0m");
    println!("  \x1b[1;36m║          Vox AI Chat (native)            ║\x1b[0m");
    println!("  \x1b[1;36m╚══════════════════════════════════════════╝\x1b[0m");
    println!();

    let mut history = Vec::new();

    let effective_session = if let Some(sid) = session {
        let sid = if sid == "last" {
            // Find most recent session ID
            if let Some(ref d) = db {
                let recs = d.recall_memory("cli_chat", None, 1).await.unwrap_or_default();
                if let Some(r) = recs.first() {
                    r.session_id.clone()
                } else {
                    generate_session_id()
                }
            } else {
                generate_session_id()
            }
        } else {
            sid.to_string()
        };

        println!("  Resuming session: \x1b[33m{sid}\x1b[0m");

        // Load history
        if let Some(ref d) = db {
            let mut msgs = d.recall_memory("cli_chat", Some(sid.as_str()), 50).await.unwrap_or_default();
            msgs.reverse(); // oldest first
            for m in msgs {
                let j: serde_json::Value = serde_json::from_str(&m.content).unwrap_or(json!({}));
                if j["role"].is_string() {
                    history.push(j);
                }
            }
        }
        sid
    } else {
        generate_session_id()
    };

    // If no provider is available, help the user get started
    if providers.best_provider() == "none" {
        println!("  \x1b[33mNo AI provider configured.\x1b[0m");
        println!();
        println!("  The easiest way to get started (free, no credit card):");
        println!();
        println!("    1. Visit \x1b[4;36mhttps://aistudio.google.com/apikey\x1b[0m");
        println!("    2. Click \"Create API Key\" (just needs a Google account)");
        println!("    3. Paste the key below:");
        println!();

        // Prompt for key
        print!("  API Key: ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let mut key_input = String::new();
        std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut key_input)?;
        let key_input = key_input.trim().to_string();

        if key_input.starts_with("AIza") || key_input.len() > 20 {
            // Save via the unified vox login auth system
            super::login::run(Some(&key_input), Some("google"), None).await?;
            providers.google_key = Some(key_input);
        } else if !key_input.is_empty() && (key_input.starts_with("sk-or-") || key_input.starts_with("sk-")) {
            super::login::run(Some(&key_input), Some("openrouter"), None).await?;
            providers.openrouter_key = Some(key_input);
        } else {
            println!("\n  \x1b[31mNo valid key provided.\x1b[0m");
            println!("  Run: \x1b[1mvox login --registry google YOUR_KEY\x1b[0m");
            println!("  Or set: \x1b[1mGEMINI_API_KEY=YOUR_KEY\x1b[0m\n");
            return Ok(());
        }
    }

    // Determine the active provider using budget-aware routing if available
    let (mut provider_name, mut default_model) = if let Some(ref t) = tracker {
        if let Ok(rec) = t.best_available_provider(
            providers.google_key.is_some(),
            providers.openrouter_key.is_some(),
            providers.ollama_available,
        ).await {
            (rec.provider, rec.model)
        } else {
            (providers.best_provider().to_string(), DEFAULT_GOOGLE_MODEL.to_string())
        }
    } else {
        (providers.best_provider().to_string(), DEFAULT_GOOGLE_MODEL.to_string())
    };

    if provider_name == "none" {
        // Fallback if everything is exhausted
        provider_name = providers.best_provider().to_string();
    }

    if provider_name != "google" {
        default_model = match provider_name.as_str() {
            "openrouter" => "mistral/devstral-2-2512:free".to_string(),
            "ollama" => "llama3.2".to_string(),
            _ => "none".to_string(),
        };
    }

    let mut effective_model = model.unwrap_or(&default_model).to_string();

    match provider_name.as_str() {
        "google" => {
            println!("  Provider: \x1b[32mGoogle AI Studio\x1b[0m (free, no credit card)");
            println!("  Model   : \x1b[32m{effective_model}\x1b[0m");
            println!("  Cascade : flash-lite → 2.5-flash → 2.5-pro");
        }
        "openrouter" => {
            if free {
                println!("  Provider: \x1b[33mOpenRouter\x1b[0m (free tier)");
                println!("  Model   : \x1b[33m{effective_model}\x1b[0m");
            } else {
                println!("  Provider: \x1b[33mOpenRouter\x1b[0m (API key detected)");
                println!("  Model   : \x1b[33m{effective_model}\x1b[0m");
                println!("  \x1b[2mPaid SOTA models available. Use --free to restrict.\x1b[0m");
            }
        }
        "ollama" => {
            println!("  Provider: \x1b[36mOllama\x1b[0m (local, zero-auth)");
            println!("  Model   : \x1b[36m{effective_model}\x1b[0m");
        }
        _ => {}
    }

    println!();
    repl_loop(
        &providers,
        &mut provider_name,
        &mut effective_model,
        &effective_session,
        &mut history,
        db.as_ref(), // to store history
        tracker.as_ref(), // to track usage
        free,
    ).await
}

async fn repl_loop(
    providers: &ProviderState,
    active_provider: &mut String,
    active_model: &mut String,
    session_id: &str,
    history: &mut Vec<serde_json::Value>,
    db: Option<&vox_db::VoxDb>,
    tracker: Option<&vox_orchestrator::usage::UsageTracker<'_>>,
    free_only: bool,
) -> Result<()> {
    use std::io::{self, BufRead, Write};

    println!("  \x1b[1mType your message and press Enter. Type 'exit' to quit.\x1b[0m");
    println!();

    let stdin = io::stdin();
    let mut attempt: u32 = 0;

    loop {
        print!("  \x1b[36m[you]\x1b[0m ");
        io::stdout().flush().ok();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() || line.trim().is_empty() {
            continue;
        }

        let prompt = line.trim();
        if prompt.eq_ignore_ascii_case("exit") || prompt.eq_ignore_ascii_case("quit") {
            println!("\n  \x1b[2mBye!\x1b[0m\n");
            break;
        }

        attempt += 1;

        let selected = escalate_model(active_provider, active_model, attempt, free_only);

        println!("  \x1b[2m[{} · {} · attempt {}]\x1b[0m", active_provider, selected, attempt);

        // Build messages
        let mut req_history = history.clone();
        req_history.push(json!({"role": "user", "content": prompt}));

        match call_provider(providers, active_provider, selected, &req_history).await {
            Ok(response) => {
                println!("  \x1b[1;32m[vox]\x1b[0m {response}");
                attempt = 0; // reset on success

                if let Some(t) = tracker {
                    // Record successful call
                    // We don't have exact token counts parsing here, mock as 128 in, 256 out for now
                    let _ = t.record_call(active_provider, selected, 128, 256, 0.0).await;
                }

                // Append to history
                let umsg = json!({"role": "user", "content": prompt});
                let amsg = json!({"role": "assistant", "content": response});

                history.push(umsg.clone());
                history.push(amsg.clone());

                // Persist session
                if let Some(d) = db {
                    let _ = d.store_memory(
                        "cli_chat", session_id, "session_turn",
                        &umsg.to_string(), None, 1.0
                    ).await;
                    let _ = d.store_memory(
                        "cli_chat", session_id, "session_turn",
                        &amsg.to_string(), None, 1.0
                    ).await;
                }
            }
            Err(e) => {
                let err_str = format!("{e}");

                // On rate limit, mark and try next
                if err_str.contains("429") || err_str.contains("rate") || err_str.contains("Quota") {
                    println!("  \x1b[33m[rate limited]\x1b[0m Marking provider as exhausted. Trying fallback...");
                    if let Some(t) = tracker {
                        let _ = t.mark_rate_limited(active_provider, selected).await;
                    }

                    // Try escalated model
                    let fallback = escalate_model(active_provider, active_model, attempt + 1, free_only);

                    match call_provider(providers, active_provider, fallback, &req_history).await {
                        Ok(r) => {
                            println!("  \x1b[1;32m[vox]\x1b[0m {r}");
                            attempt = 0;

                            // History
                            history.push(json!({"role": "user", "content": prompt}));
                            let amsg = json!({"role": "assistant", "content": r});
                            history.push(amsg.clone());

                            if let Some(d) = db {
                                let _ = d.store_memory("cli_chat", session_id, "session_turn", &json!({"role": "user", "content": prompt}).to_string(), None, 1.0).await;
                                let _ = d.store_memory("cli_chat", session_id, "session_turn", &amsg.to_string(), None, 1.0).await;
                            }
                            if let Some(t) = tracker {
                                let _ = t.record_call(active_provider, fallback, 128, 256, 0.0).await;
                            }
                        }
                        Err(e2) => {
                            eprintln!("  \x1b[31m[error]\x1b[0m {e2}");
                            // Also rate limited? Try rotating provider fully next time.
                            if let Some(t) = tracker {
                                if let Ok(rec) = t.best_available_provider(
                                    providers.google_key.is_some(),
                                    providers.openrouter_key.is_some(),
                                    providers.ollama_available
                                ).await {
                                    if rec.provider != *active_provider {
                                        println!("  \x1b[33mRotating to {} provider\x1b[0m", rec.provider);
                                        *active_provider = rec.provider;
                                        *active_model = rec.model;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    eprintln!("  \x1b[31m[error]\x1b[0m {e}");
                }

                if attempt >= 5 {
                    eprintln!("  \x1b[31mAll fallbacks exhausted. Check network or API key.\x1b[0m");
                    attempt = 0;
                }
            }
        }

        println!();
    }

    Ok(())
}

/// Escalate model tier based on retry attempt number.
fn escalate_model<'a>(
    provider: &str,
    default: &'a str,
    attempt: u32,
    _free_only: bool,
) -> &'a str {
    match provider {
        "google" => {
            match attempt {
                1 => default,
                _ => {
                    let idx = (attempt as usize - 1).min(GOOGLE_ESCALATION.len() - 1);
                    GOOGLE_ESCALATION[idx]
                }
            }
        }
        "openrouter" => {
            if attempt <= 1 {
                default
            } else {
                let idx = ((attempt as usize) - 1).min(OPENROUTER_FREE_ESCALATION.len() - 1);
                OPENROUTER_FREE_ESCALATION[idx]
            }
        }
        _ => default,
    }
}

/// Route to the correct provider and call the model.
async fn call_provider(
    providers: &ProviderState,
    provider: &str,
    model: &str,
    history: &[serde_json::Value],
) -> Result<String> {
    match provider {
        "google" => call_google(providers.google_key.as_deref().unwrap_or(""), model, history).await,
        "openrouter" => call_openrouter(providers.openrouter_key.as_deref().unwrap_or(""), model, history).await,
        "ollama" => call_ollama(model, history).await,
        _ => anyhow::bail!("No provider available"),
    }
}

/// Call Google AI Studio's Gemini API directly (free, no credit card needed).
async fn call_google(api_key: &str, model: &str, history: &[serde_json::Value]) -> Result<String> {
    let url = format!(
        "{}/models/{}:generateContent?key={}",
        GOOGLE_API_BASE, model, api_key
    );

    // Convert OpenAI format to Gemini format
    let mut contents = Vec::new();
    for msg in history {
        let role = if msg["role"] == "assistant" { "model" } else { "user" };
        contents.push(json!({
            "role": role,
            "parts": [{"text": msg["content"]}]
        }));
    }

    let body = serde_json::json!({
        "contents": contents,
        "generationConfig": {"maxOutputTokens": 2048}
    });

    let client = build_client()?;
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Google AI Studio {}: {}", status, &text[..text.len().min(300)]);
    }

    let json: serde_json::Value = resp.json().await?;
    let content = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("[empty response]")
        .to_string();
    Ok(content)
}

/// Call OpenRouter API (requires API key, works with :free models).
async fn call_openrouter(api_key: &str, model: &str, history: &[serde_json::Value]) -> Result<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": history,
        "max_tokens": 2048,
    });

    let client = build_client()?;
    let resp = client
        .post(OPENROUTER_API_URL)
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/brbrainerd/vox")
        .header("X-Title", "Vox CLI Chat")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenRouter {}: {}", status, &text[..text.len().min(300)]);
    }

    let json: serde_json::Value = resp.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[empty response]")
        .to_string();
    Ok(content)
}

/// Call local Ollama instance (zero-auth, always available when running).
async fn call_ollama(model: &str, history: &[serde_json::Value]) -> Result<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": history,
        "stream": false,
    });

    let client = build_client()?;
    let resp = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama {}: {}", status, &text[..text.len().min(300)]);
    }

    let json: serde_json::Value = resp.json().await?;
    let content = json["message"]["content"]
        .as_str()
        .unwrap_or("[empty response]")
        .to_string();
    Ok(content)
}

/// Shared HTTP client builder.
fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("vox-cli/0.2.0 (https://github.com/brbrainerd/vox)")
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_escalation_chain_is_valid() {
        assert_eq!(GOOGLE_ESCALATION.len(), 3);
        assert_eq!(GOOGLE_ESCALATION[0], "gemini-2.0-flash-lite");
        assert_eq!(GOOGLE_ESCALATION[2], "gemini-2.5-pro");
    }

    #[test]
    fn openrouter_escalation_never_panics() {
        for attempt in 1..=20 {
            let _ = escalate_model("openrouter", "test", attempt, true);
        }
    }

    #[test]
    fn google_escalation_never_panics() {
        for attempt in 1..=20 {
            let _ = escalate_model("google", "gemini-2.5-flash-preview", attempt, true);
        }
    }

    #[test]
    fn provider_state_detection_does_not_panic() {
        let _state = ProviderState::detect();
    }

    #[test]
    fn escalate_returns_default_on_first_attempt() {
        assert_eq!(
            escalate_model("google", "my-default", 1, false),
            "my-default"
        );
        assert_eq!(
            escalate_model("openrouter", "my-default", 1, false),
            "my-default"
        );
        assert_eq!(
            escalate_model("ollama", "my-default", 1, false),
            "my-default"
        );
    }
}
