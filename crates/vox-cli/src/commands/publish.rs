use anyhow::{Context, Result};
use std::path::PathBuf;

/// `vox publish` — publish the current package to the VoxPM registry.
pub async fn run(registry_url: Option<&str>) -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let manifest = vox_pm::VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Nothing to publish.")?;

    let url = registry_url.unwrap_or("https://raw.githubusercontent.com/brbrainerd/vox/main/registry");

    // Read auth token
    let token_path = dirs_path().join("auth_token");
    let token = std::fs::read_to_string(&token_path)
        .with_context(|| "Not logged in. Run `vox login` first.")?;
    let token = token.trim();

    let client = vox_pm::RegistryClient::with_auth(url, token);

    println!(
        "Publishing {}@{} ({})...",
        manifest.package.name, manifest.package.version, manifest.package.kind
    );

    // Collect package data (simplified: serialize the manifest + source)
    let data = manifest
        .to_toml_string()
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .into_bytes();

    let content_hash = vox_pm::hash::content_hash(&data);

    let deps: Vec<vox_pm::registry::PublishDependency> = manifest
        .dependencies
        .iter()
        .map(|(name, spec)| vox_pm::registry::PublishDependency {
            name: name.clone(),
            version_req: spec.version_req().unwrap_or("*").to_string(),
        })
        .collect();

    let req = vox_pm::registry::PublishRequest {
        name: manifest.package.name.clone(),
        version: manifest.package.version.clone(),
        kind: manifest.package.kind.clone(),
        description: manifest.package.description.clone(),
        license: manifest.package.license.clone(),
        content_hash,
        data,
        dependencies: deps,
    };

    match client.publish(req).await {
        Ok(()) => {
            println!(
                "✓ Published {}@{} successfully!",
                manifest.package.name, manifest.package.version
            );
        }
        Err(e) => {
            anyhow::bail!("Publish failed: {e}");
        }
    }

    Ok(())
}

/// Get the VoxPM config directory (~/.vox/).
fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".vox")
}
