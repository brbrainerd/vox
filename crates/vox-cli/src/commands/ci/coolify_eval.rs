//! Coolify eval sandbox — HTTP API discovery and compose sync (no SSH).
//!
//! Uses Clavis [`SecretId::CoolifyBaseUrl`], [`SecretId::CoolifyToken`],
//! [`SecretId::CoolifyReadToken`], [`SecretId::CoolifyAppUuid`].

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::CoolifyEvalCmd;
use super::repo_root;

fn require_secret(id: vox_secrets::SecretId) -> Result<String> {
    let r = vox_secrets::resolve_secret(id);
    let v = r.expose().filter(|s| !s.is_empty());
    v.map(str::to_string)
        .with_context(|| format!("missing or empty secret {:?}", id))
}

fn base_url() -> Result<String> {
    require_secret(vox_secrets::SecretId::CoolifyBaseUrl)
        .map(|s| s.trim_end_matches('/').to_string())
}

fn bearer_read() -> Result<String> {
    let read = vox_secrets::resolve_secret(vox_secrets::SecretId::CoolifyReadToken)
        .expose()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(t) = read {
        return Ok(t);
    }
    require_secret(vox_secrets::SecretId::CoolifyToken)
}

fn bearer_write() -> Result<String> {
    require_secret(vox_secrets::SecretId::CoolifyToken)
}

fn default_app_uuid() -> Result<String> {
    require_secret(vox_secrets::SecretId::CoolifyAppUuid)
}

fn applications_slice(body: &Value) -> Vec<&Value> {
    match body {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => {
            if let Some(Value::Array(a)) = o.get("data") {
                return a.iter().collect();
            }
            if let Some(Value::Array(a)) = o.get("applications") {
                return a.iter().collect();
            }
            vec![]
        }
        _ => vec![],
    }
}

/// Run `vox ci coolify-eval …`.
pub async fn run(cmd: CoolifyEvalCmd) -> Result<()> {
    let root = repo_root();
    match cmd {
        CoolifyEvalCmd::Discover => discover().await,
        CoolifyEvalCmd::SyncCompose {
            compose,
            app_uuid,
            deploy,
            domains,
        } => sync_compose(&root, compose, app_uuid, deploy, domains).await,
    }
}

async fn discover() -> Result<()> {
    let base = base_url()?;
    let token = bearer_read()?;
    let client = vox_http_client::client();

    let ver_url = format!("{base}/api/v1/version");
    match client
        .get(&ver_url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let t = resp.text().await.unwrap_or_default();
            println!(
                "Coolify GET /api/v1/version (truncated): {}",
                &t[..t.len().min(400)]
            );
        }
        Ok(resp) => {
            println!(
                "Coolify version probe HTTP {} (endpoint may not exist on this install)",
                resp.status()
            );
        }
        Err(e) => {
            println!("Coolify version probe failed (non-fatal): {e}");
        }
    }

    let list_url = format!("{base}/api/v1/applications");
    let resp = client
        .get(&list_url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .context("GET /api/v1/applications")?;
    let status = resp.status();
    let text = resp.text().await.context("read applications body")?;
    if !status.is_success() {
        if status.as_u16() == 403 {
            println!(
                "Coolify applications list HTTP 403 (missing read scope). \
Proceeding without discovery output. Body head: {}",
                &text[..text.len().min(400)]
            );
            return Ok(());
        }
        anyhow::bail!(
            "GET /api/v1/applications HTTP {status}: {}",
            &text[..text.len().min(800)]
        );
    }
    let body: Value = serde_json::from_str(&text).with_context(|| {
        format!(
            "applications JSON parse failed; body head: {}",
            &text[..text.len().min(200)]
        )
    })?;

    println!(
        "Applications (uuid | name | fqdn | has_compose): {}",
        applications_slice(&body).len()
    );
    for row in applications_slice(&body) {
        let uuid = row
            .get("uuid")
            .and_then(Value::as_str)
            .unwrap_or("(no uuid)");
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("(no name)");
        let fqdn = row.get("fqdn").and_then(Value::as_str).unwrap_or("");
        let raw = row
            .get("docker_compose_raw")
            .and_then(Value::as_str)
            .unwrap_or("");
        let has = if raw.is_empty() { "no" } else { "yes" };
        println!("  {uuid} | {name} | {fqdn} | compose_raw={has}");
    }

    println!(
        "\nGate 3 default health URL should match the eval app FQDN; see docs/src/ci/deploy-contract.md."
    );
    Ok(())
}

async fn sync_compose(
    root: &Path,
    compose: PathBuf,
    app_uuid: Option<String>,
    deploy: bool,
    domains: Option<String>,
) -> Result<()> {
    let base = base_url()?;
    let read_tok = bearer_read()?;
    let write_tok = bearer_write()?;
    let uuid = match app_uuid {
        Some(u) if !u.is_empty() => u,
        _ => default_app_uuid()?,
    };

    let compose_path = if compose.is_absolute() {
        compose
    } else {
        root.join(compose)
    };
    let raw = std::fs::read_to_string(&compose_path)
        .with_context(|| format!("read compose {}", compose_path.display()))?;

    let client = vox_http_client::client();

    // Verify UUID exists and is compose-capable (read token).
    let get_url = format!("{base}/api/v1/applications/{uuid}");
    let cur = client
        .get(&get_url)
        .header("Authorization", format!("Bearer {read_tok}"))
        .send()
        .await
        .context("GET application")?;
    let cur_status = cur.status();
    let cur_text = cur.text().await?;
    if !cur_status.is_success() {
        if cur_status.as_u16() == 403 {
            println!(
                "GET application HTTP 403 (missing read scope); continuing to PATCH with write token. \
Body head: {}",
                &cur_text[..cur_text.len().min(400)]
            );
        } else {
            anyhow::bail!(
                "GET application HTTP {cur_status}: {}",
                &cur_text[..cur_text.len().min(1200)]
            );
        }
    }

    let mut patch = json!({
        "docker_compose_raw": raw,
        "instant_deploy": deploy,
    });
    if let Some(ref d) = domains {
        if !d.is_empty() {
            patch["domains"] = Value::String(d.clone());
        }
    }

    let patch_url = format!("{base}/api/v1/applications/{uuid}");
    let patched = client
        .patch(&patch_url)
        .header("Authorization", format!("Bearer {write_tok}"))
        .header("Accept", "application/json")
        .json(&patch)
        .send()
        .await
        .context("PATCH application")?;
    let ps = patched.status();
    let body = patched.text().await.unwrap_or_default();
    if !ps.is_success() {
        anyhow::bail!(
            "PATCH /api/v1/applications/{{uuid}} HTTP {ps}: {}",
            &body[..body.len().min(2000)]
        );
    }
    println!("PATCH OK (HTTP {ps}): {}", &body[..body.len().min(300)]);

    if deploy {
        let deploy_url = format!("{base}/api/v1/deploy?uuid={uuid}");
        let dresp = client
            .get(&deploy_url)
            .header("Authorization", format!("Bearer {write_tok}"))
            .send()
            .await
            .context("GET deploy")?;
        let ds = dresp.status();
        let dtxt = dresp.text().await.unwrap_or_default();
        println!("Deploy trigger HTTP {ds}: {}", &dtxt[..dtxt.len().min(400)]);
        if !ds.is_success() {
            anyhow::bail!("deploy trigger HTTP {ds}");
        }
    }

    Ok(())
}
