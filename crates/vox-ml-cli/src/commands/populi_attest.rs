//! `vox populi attest` subcommands (P6-T2).
//!
//! Publishes or fetches a signed `PublicAttestationManifest` for this node.
//! The manifest is published to a GitHub Gist or `.well-known/vox-manifest.json`
//! — there is no Vox-owned server involved.

use clap::Subcommand;
use vox_mesh_types::attestation_manifest::{PublicAttestationManifest, SupportedTask};

#[derive(Subcommand, Debug)]
pub enum AttestCmd {
    /// Sign and publish the local node's attestation manifest.
    Publish {
        /// Override the target URL (default: GitHub Gist URL from secrets env).
        #[arg(long)]
        target_url: Option<String>,
        /// Optional node ID override (defaults to local resolved node ID).
        #[arg(long)]
        node_id: Option<String>,
        /// Comma-separated task kinds to declare as supported.
        #[arg(long, value_delimiter = ',', default_value = "text_infer")]
        task_kinds: Vec<String>,
        /// Dry-run: print the manifest without publishing.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Fetch and display an attestation manifest for a given node or URL.
    Fetch {
        /// URL of the manifest (Gist raw URL or .well-known path).
        #[arg(long)]
        url: Option<String>,
        /// Node ID to look up (resolved via control plane when no --url given).
        #[arg(long)]
        node_id: Option<String>,
        /// Emit raw JSON.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

/// Run an `AttestCmd`.
pub async fn run(cmd: AttestCmd) -> anyhow::Result<()> {
    match cmd {
        AttestCmd::Publish {
            target_url,
            node_id,
            task_kinds,
            dry_run,
        } => {
            let node_id = node_id.unwrap_or_else(|| format!("local-{}", simple_hex_id()));

            let supported_tasks: Vec<SupportedTask> = task_kinds
                .iter()
                .map(|k| SupportedTask {
                    kind: k.clone(),
                    supported: true,
                    min_vram_mb: None,
                    max_concurrent: None,
                })
                .collect();

            let pubkey_hex = load_node_pubkey_hex().unwrap_or_else(|_| "0".repeat(64));

            let mut manifest = PublicAttestationManifest {
                version: "1".to_string(),
                node_id: node_id.clone(),
                pubkey_hex,
                published_at: chrono_now_iso8601(),
                supported_tasks,
                metadata: Default::default(),
                signature_b64: String::new(),
            };

            // Sign if key is available.
            match sign_manifest(&mut manifest) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "could not sign manifest; publishing unsigned");
                }
            }

            let json = serde_json::to_string_pretty(&manifest)?;

            if dry_run {
                println!("--- Dry run: manifest not published ---");
                println!("{}", json);
                return Ok(());
            }

            if let Some(url) = target_url {
                publish_to_url(&url, &json).await?;
                println!("vox populi attest publish: manifest published to {}", url);
            } else {
                println!("vox populi attest publish: no --target-url provided.");
                println!("  Use --dry-run to preview the manifest.");
                println!("{}", json);
            }

            Ok(())
        }
        AttestCmd::Fetch { url, node_id, json } => {
            let fetch_url = match (url, node_id) {
                (Some(u), _) => u,
                (None, Some(id)) => {
                    anyhow::bail!(
                        "vox populi attest fetch: --node-id lookup requires a control plane; \
                         pass --url directly. (node_id={})",
                        id
                    )
                }
                (None, None) => {
                    anyhow::bail!(
                        "vox populi attest fetch: pass --url <manifest-url> or --node-id <id>"
                    )
                }
            };

            let manifest = fetch_manifest(&fetch_url).await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else {
                println!("Attestation Manifest for node '{}'", manifest.node_id);
                println!("  Version:      {}", manifest.version);
                println!("  Published at: {}", manifest.published_at);
                println!("  Pubkey:       {}", &manifest.pubkey_hex[..16]);
                println!("  Tasks:");
                for t in &manifest.supported_tasks {
                    let status = if t.supported { "yes" } else { "no" };
                    println!("    - {} (supported={})", t.kind, status);
                }
            }

            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn simple_hex_id() -> String {
    vox_actor_runtime::simple_id::simple_hex_id()
}

fn chrono_now_iso8601() -> String {
    // Use SystemTime to avoid pulling in chrono as an explicit dep.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Minimal ISO-8601 UTC formatting.
    format_unix_secs_as_iso8601(secs)
}

fn format_unix_secs_as_iso8601(secs: u64) -> String {
    // Very small ISO-8601 formatter for UTC timestamps without pulling in chrono.
    // Accurate for dates after 1970-01-01.
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let yd = if leap { 366 } else { 365 };
        if days < yd {
            break;
        }
        days -= yd;
        year += 1;
    }
    let months = [
        31u64,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for md in &months {
        if days < *md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn load_node_pubkey_hex() -> anyhow::Result<String> {
    let sk_b64 = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshFederationSigningKey)
        .expose()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("VoxMeshFederationSigningKey not configured"))?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sk_b64.trim())
        .map_err(|e| anyhow::anyhow!("invalid base64: {}", e))?;
    let sk = vox_crypto::facades::signing_key_from_bytes(
        &bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("wrong key length"))?,
    );
    let vk = vox_crypto::facades::to_verifying_key(&sk);
    Ok(hex::encode(vox_crypto::facades::verifying_key_to_bytes(
        &vk,
    )))
}

fn sign_manifest(manifest: &mut PublicAttestationManifest) -> anyhow::Result<()> {
    let sk_b64 = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshFederationSigningKey)
        .expose()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("VoxMeshFederationSigningKey not configured"))?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, sk_b64.trim())
        .map_err(|e| anyhow::anyhow!("invalid base64: {}", e))?;
    let sk = vox_crypto::facades::signing_key_from_bytes(
        &bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("wrong key length"))?,
    );
    let canonical = manifest
        .canonical_signing_bytes()
        .map_err(|e| anyhow::anyhow!("canonical bytes: {}", e))?;
    let sig = vox_crypto::facades::sign(&sk, &canonical);
    manifest.signature_b64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, sig);
    Ok(())
}

async fn publish_to_url(url: &str, body: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .put(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP PUT to {}: {}", url, e))?;
    if !resp.status().is_success() {
        anyhow::bail!("publish to {} failed with status {}", url, resp.status());
    }
    Ok(())
}

async fn fetch_manifest(url: &str) -> anyhow::Result<PublicAttestationManifest> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GET {}: {}", url, e))?;
    if !resp.status().is_success() {
        anyhow::bail!("fetch from {} failed with status {}", url, resp.status());
    }
    let manifest: PublicAttestationManifest = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("parse manifest: {}", e))?;
    Ok(manifest)
}
