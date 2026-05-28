//! `vox plugin publish <id> [--gateway <url>] [--api-key <key>]`
//!
//! Publish a locally installed skill plugin to an OpenClaw-compatible gateway
//! (e.g. ClawHub). The SKILL.md is read from the install directory; the name
//! and version come from Plugin.toml. Authentication resolves in this order:
//!
//! 1. `--api-key` flag passed on the command line
//! 2. `OPENCLAW_API_KEY` (or alias `CLAWHUB_API_KEY`) via vox-secrets
//! 3. `OPENCLAW_TOKEN` via vox-secrets (existing session token)
//!
//! On success, the gateway-assigned slug and canonical URL are printed.

use super::list::{installed_version, plugins_root};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Plugin.toml head (minimal; only fields needed for publish)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PluginHead {
    plugin: PluginSection,
    #[serde(default)]
    skill: Option<SkillSection>,
}

#[derive(Debug, Deserialize)]
struct PluginSection {
    id: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SkillSection {
    /// Relative path to the SKILL.md inside the install directory.
    #[serde(default = "default_skill_md")]
    skill_md: String,
}

fn default_skill_md() -> String {
    "SKILL.md".to_string()
}

// ---------------------------------------------------------------------------
// Gateway response shape
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PublishResult {
    slug: String,
    url: String,
    #[serde(default)]
    revision: Option<String>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run `vox plugin publish`.
pub async fn run(
    plugin_id: &str,
    gateway: Option<String>,
    api_key_arg: Option<String>,
) -> Result<()> {
    // 1. Resolve API key: flag > secrets OpenClawApiKey > secrets OpenClawToken.
    let api_key = if let Some(k) = api_key_arg.and_then(|k| {
        let t = k.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    }) {
        Some(k)
    } else {
        // Try publish-specific key first, fall back to existing session token.
        let publish_key = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawApiKey)
            .expose()
            .map(str::to_string);
        if publish_key.is_some() {
            publish_key
        } else {
            vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
                .expose()
                .map(str::to_string)
        }
    };

    if api_key.is_none() {
        bail!(
            "No OpenClaw API key found. Set OPENCLAW_API_KEY env var, \
             pass --api-key, or configure it in vox-secrets."
        );
    }

    // 2. Locate the installed plugin.
    let root = plugins_root();
    let version = installed_version(&root, plugin_id)
        .with_context(|| format!("Plugin '{plugin_id}' is not installed"))?;
    let install_dir: PathBuf = root.join(plugin_id).join(&version);

    // 3. Read Plugin.toml.
    let plugin_toml_path = install_dir.join("Plugin.toml");
    let raw_toml = std::fs::read_to_string(&plugin_toml_path)
        .with_context(|| format!("reading {}", plugin_toml_path.display()))?;
    let head: PluginHead = toml::from_str(&raw_toml)
        .with_context(|| format!("parsing {}", plugin_toml_path.display()))?;

    // 4. Find SKILL.md path.
    let skill_md_rel = head
        .skill
        .as_ref()
        .map(|s| s.skill_md.clone())
        .unwrap_or_else(default_skill_md);
    let skill_md_path = install_dir.join(&skill_md_rel);
    if !skill_md_path.exists() {
        bail!(
            "Plugin '{plugin_id}' has no SKILL.md at {}. \
             Only skill plugins can be published.",
            skill_md_path.display()
        );
    }
    let skill_md = std::fs::read_to_string(&skill_md_path)
        .with_context(|| format!("reading {}", skill_md_path.display()))?;

    // 5. POST to the gateway.
    let gateway_url = gateway
        .as_deref()
        .unwrap_or("https://api.clawhub.ai")
        .trim_end_matches('/')
        .to_string();
    let endpoint = format!("{gateway_url}/v1/skills");

    let body = serde_json::json!({
        "name": head.plugin.id,
        "version": head.plugin.version,
        "description": head.plugin.description,
        "skill_md": skill_md,
    });

    let client = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("building HTTP client")?;

    println!("Publishing '{plugin_id}' v{version} to {gateway_url} …");

    let mut req = client
        .post(&endpoint)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(&body);
    if let Some(ref key) = api_key {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.context("POST /v1/skills")?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body_text = resp.text().await.unwrap_or_default();
        bail!("Gateway returned HTTP {status}: {body_text}");
    }

    let result: PublishResult = resp
        .json()
        .await
        .context("parsing publish response from gateway")?;

    println!("Published successfully.");
    println!("  slug : {}", result.slug);
    println!("  url  : {}", result.url);
    if let Some(rev) = &result.revision {
        println!("  rev  : {rev}");
    }

    Ok(())
}
