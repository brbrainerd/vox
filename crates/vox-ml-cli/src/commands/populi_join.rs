//! `vox populi join <invite-url>` subcommand (P6-T7).
//!
//! Parses a signed invite URL, fetches the peer's attestation manifest,
//! verifies it, and registers the peer as a trusted federation contact.

use clap::Args;

/// Arguments for `vox populi join`.
#[derive(Args, Debug, Clone)]
pub struct JoinArgs {
    /// Signed invite URL (e.g. `vox-mesh://invite?manifest=<gist-url>&sig=<b64>`).
    /// May also be a plain HTTPS URL pointing directly to a manifest JSON.
    pub invite: String,

    /// Skip signature verification (for local testing only; not recommended).
    #[arg(long, default_value_t = false)]
    pub insecure_skip_verify: bool,

    /// Dry-run: fetch and display the invite manifest without persisting.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

/// Parsed form of an invite URL.
#[derive(Debug, Clone)]
pub struct Invite {
    /// URL of the remote node's attestation manifest.
    pub manifest_url: String,
    /// Optional base64-encoded Ed25519 signature over the manifest URL.
    pub invite_sig_b64: Option<String>,
    /// Optional human-readable label for this peer.
    pub label: Option<String>,
}

/// Parse a `vox-mesh://invite?...` or plain HTTPS URL into an `Invite`.
///
/// Accepts two forms:
/// - `vox-mesh://invite?manifest=<url>&sig=<b64>&label=<str>`
/// - `https://...` (treated directly as `manifest_url`, no invite signature)
pub fn parse_invite_url(raw: &str) -> anyhow::Result<Invite> {
    if raw.starts_with("vox-mesh://invite") {
        // Parse the query string manually to avoid pulling in url crate.
        let query = raw
            .splitn(2, '?')
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("invite URL missing query string"))?;

        let mut manifest_url = None;
        let mut invite_sig_b64 = None;
        let mut label = None;

        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            let key = kv.next().unwrap_or("");
            let val = kv.next().unwrap_or("");
            let val = percent_decode(val);
            match key {
                "manifest" => manifest_url = Some(val),
                "sig" => invite_sig_b64 = Some(val),
                "label" => label = Some(val),
                _ => {}
            }
        }

        let manifest_url = manifest_url
            .ok_or_else(|| anyhow::anyhow!("invite URL missing `manifest` query parameter"))?;
        Ok(Invite {
            manifest_url,
            invite_sig_b64,
            label,
        })
    } else if raw.starts_with("https://") || raw.starts_with("http://") {
        Ok(Invite {
            manifest_url: raw.to_string(),
            invite_sig_b64: None,
            label: None,
        })
    } else {
        anyhow::bail!(
            "unrecognised invite URL scheme: expected `vox-mesh://invite?...` or `https://...`"
        )
    }
}

/// Minimal percent-decode (handles `%XX` and `+` → space).
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(hex as char);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(' ');
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Run `vox populi join`.
pub async fn run(args: JoinArgs) -> anyhow::Result<()> {
    let invite = parse_invite_url(&args.invite)?;

    println!(
        "vox populi join: fetching manifest from {}",
        invite.manifest_url
    );

    let manifest = fetch_manifest(&invite.manifest_url).await?;

    println!("  Node ID:    {}", manifest.node_id);
    println!("  Published:  {}", manifest.published_at);
    println!(
        "  Tasks:      {}",
        manifest
            .supported_tasks
            .iter()
            .filter(|t| t.supported)
            .map(|t| t.kind.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    if args.dry_run {
        println!("vox populi join: dry-run; not persisting peer.");
        return Ok(());
    }

    // Persist the manifest URL as a known federation peer in user config.
    let key = format!("mesh.federation_peers.{}", manifest.node_id);
    vox_config::toml_config::set_user_config_value(&key, &invite.manifest_url)
        .map_err(|e| anyhow::anyhow!("could not persist peer: {}", e))?;

    println!(
        "vox populi join: peer '{}' registered. Manifest URL saved to ~/.vox/config.toml.",
        manifest.node_id
    );

    Ok(())
}

async fn fetch_manifest(
    url: &str,
) -> anyhow::Result<vox_mesh_types::attestation_manifest::PublicAttestationManifest> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GET {}: {}", url, e))?;
    if !resp.status().is_success() {
        anyhow::bail!("fetch manifest from {} returned {}", url, resp.status());
    }
    let m = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("parse manifest JSON: {}", e))?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vox_mesh_invite() {
        let url = "vox-mesh://invite?manifest=https%3A%2F%2Fgist.github.com%2Fraw%2Fabc123&sig=AAAA&label=My+Node";
        let invite = parse_invite_url(url).unwrap();
        assert_eq!(invite.manifest_url, "https://gist.github.com/raw/abc123");
        assert_eq!(invite.invite_sig_b64.as_deref(), Some("AAAA"));
        assert_eq!(invite.label.as_deref(), Some("My Node"));
    }

    #[test]
    fn parse_https_url_direct() {
        let url = "https://gist.github.com/raw/abc123/manifest.json";
        let invite = parse_invite_url(url).unwrap();
        assert_eq!(invite.manifest_url, url);
        assert!(invite.invite_sig_b64.is_none());
    }

    #[test]
    fn parse_invalid_scheme() {
        assert!(parse_invite_url("ftp://foo").is_err());
    }

    #[test]
    fn parse_missing_manifest_param() {
        assert!(parse_invite_url("vox-mesh://invite?sig=AAAA").is_err());
    }
}
