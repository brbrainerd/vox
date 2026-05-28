use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

use crate::commands::extras::ludus::LudusContext;

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_API_URL: &str = "https://api.github.com/user";

#[derive(Debug, Serialize)]
struct DeviceCodeRequest<'a> {
    client_id: &'a str,
    scope: &'a str,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Serialize)]
struct TokenRequest<'a> {
    client_id: &'a str,
    device_code: &'a str,
    grant_type: &'a str,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
}

pub async fn auth_command(provider: &str) -> Result<()> {
    let ctx = LudusContext::load().await?;

    if provider.to_lowercase() != "github" {
        anyhow::bail!(
            "Unsupported provider '{}'. Only 'github' is supported.",
            provider
        );
    }

    // Resolve Client ID from Clavis
    let client_id =
        match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGithubClientId).expose() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => {
                // Fallback for development if not configured
                "Iv1.6a0e696f4e1f7d4e".to_string()
            }
        };

    println!(
        "{}",
        "=== Ludus Identity Federation ===".bright_cyan().bold()
    );
    println!("Authenticating with provider: {}", provider.bright_green());
    println!("Local User ID: {}", ctx.user_id.bright_yellow());
    println!();

    let client = reqwest::Client::new();

    // 1. Request Device Code
    let res = client
        .post(GITHUB_DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .json(&DeviceCodeRequest {
            client_id: &client_id,
            scope: "read:user",
        })
        .send()
        .await?;

    if !res.status().is_success() {
        anyhow::bail!(
            "Failed to request device code from GitHub: {}",
            res.status()
        );
    }

    let device_resp: DeviceCodeResponse = res.json().await?;

    println!(
        "Please open this URL in your browser: {}",
        device_resp.verification_uri.bright_blue().underline()
    );
    println!(
        "And enter the following code:       {}",
        device_resp.user_code.bright_yellow().bold()
    );
    println!();
    println!("{}", "Waiting for authorization...".dimmed());

    // 2. Poll for Access Token
    let mut interval = device_resp.interval;
    if interval == 0 {
        interval = 5;
    }
    let mut expires_in = device_resp.expires_in;

    let access_token = loop {
        if expires_in == 0 {
            anyhow::bail!("Device code expired. Please try again.");
        }

        sleep(Duration::from_secs(interval)).await;
        expires_in = expires_in.saturating_sub(interval);

        let res = client
            .post(GITHUB_TOKEN_URL)
            .header("Accept", "application/json")
            .json(&TokenRequest {
                client_id: &client_id,
                device_code: &device_resp.device_code,
                grant_type: "urn:ietf:params:oauth:grant-type:device_code",
            })
            .send()
            .await?;

        let token_resp: TokenResponse = res.json().await?;

        if let Some(token) = token_resp.access_token {
            break token;
        }

        if let Some(err) = token_resp.error {
            match err.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += 5;
                    continue;
                }
                "expired_token" => anyhow::bail!("The device code has expired. Please try again."),
                "access_denied" => anyhow::bail!("Authorization was denied by the user."),
                _ => anyhow::bail!(
                    "GitHub OAuth error: {} - {}",
                    err,
                    token_resp.error_description.unwrap_or_default()
                ),
            }
        }
    };

    // 3. Fetch GitHub User Info
    let user_res = client
        .get(GITHUB_USER_API_URL)
        .header("User-Agent", "VoxGamifyCLI/1.0")
        .header("Authorization", format!("token {}", access_token))
        .send()
        .await?;

    if !user_res.status().is_success() {
        anyhow::bail!("Failed to fetch GitHub user info: {}", user_res.status());
    }

    let gh_user: GitHubUser = user_res.json().await?;

    // 4. Save to Clavis and DB
    vox_secrets::set_registry_token("github.com", &access_token, Some(gh_user.login.clone()))?;

    ctx.db
        .upsert_vox_identity(
            &ctx.user_id,
            "github",
            &gh_user.id.to_string(),
            Some(&gh_user.login),
            Some("VoxGithubOauthToken"),
        )
        .await?;

    println!();
    println!("{}", "✅ Authentication successful!".bright_green().bold());
    println!(
        "Linked GitHub account {} (ID: {}) to Vox profile.",
        gh_user.login.bright_cyan(),
        gh_user.id.to_string().bright_magenta()
    );
    println!(
        "Your gamification progress can now be tracked and rewarded via GitHub contributions."
    );

    Ok(())
}
