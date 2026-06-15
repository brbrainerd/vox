//! One-shot vision completion to OpenRouter using a base64 PNG image part.
use base64::Engine as _;
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cost_usd: Option<f64>,
}

#[derive(Deserialize)]
struct OrUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_cost: Option<f64>,
    #[serde(default)]
    cost: Option<f64>,
}
#[derive(Deserialize)]
struct OrMsg {
    content: String,
}
#[derive(Deserialize)]
struct OrChoice {
    message: OrMsg,
}
#[derive(Deserialize)]
struct OrResp {
    choices: Vec<OrChoice>,
    #[serde(default)]
    usage: Option<OrUsage>,
}

pub async fn call_vision_model(
    model: &str,
    system: &str,
    user_text: &str,
    png_bytes: &[u8],
) -> Result<(String, Usage), String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    let data_url = format!("data:image/png;base64,{b64}");
    let url = vox_config::openrouter_chat_completions_url();
    // Sanctioned resolver reads OPENROUTER_API_KEY via vox-secrets.
    let resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey);
    let key = resolved
        .expose()
        .ok_or_else(|| format!("no OpenRouter key: {}", resolved.remediation))?
        .to_string();
    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": [
                { "type": "text", "text": user_text },
                { "type": "image_url", "image_url": { "url": data_url } }
            ] }
        ],
        "temperature": 0.2,
        "usage": { "include": true }
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(key)
        .header("HTTP-Referer", "https://github.com/vox-foundation/vox")
        .header("X-Title", "vox-gui-visual-review")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "openrouter {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    let parsed: OrResp = resp.json().await.map_err(|e| e.to_string())?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default();
    let usage = parsed
        .usage
        .map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            cost_usd: u.total_cost.or(u.cost),
        })
        .unwrap_or_default();
    Ok((content, usage))
}
