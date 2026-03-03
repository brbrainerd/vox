use anyhow::Result;

/// `vox info <package>` — display package information.
pub async fn run(package_name: &str, registry_url: Option<&str>) -> Result<()> {
    let url = registry_url.unwrap_or("https://raw.githubusercontent.com/brbrainerd/vox/main/registry");
    let client = vox_pm::RegistryClient::new(url);

    match client.info(package_name).await {
        Ok(info) => {
            println!("┌─────────────────────────────────────────┐");
            println!("│ {} v{}", info.name, info.latest_version);
            println!("├─────────────────────────────────────────┤");
            if let Some(desc) = &info.description {
                println!("│ {}", desc);
            }
            println!("│");
            println!("│ Kind:      {}", info.kind);
            if let Some(author) = &info.author {
                println!("│ Author:    {}", author);
            }
            if let Some(license) = &info.license {
                println!("│ License:   {}", license);
            }
            println!("│ Downloads: {}", info.downloads);
            println!("│");
            println!("│ Versions:");
            for v in &info.versions {
                let marker = if *v == info.latest_version {
                    " (latest)"
                } else {
                    ""
                };
                println!("│   {}{}", v, marker);
            }
            println!("└─────────────────────────────────────────┘");
        }
        Err(e) => {
            // Fall back to local store
            println!("⚠ Registry unavailable ({e}), checking locally...");

            let store_path = ".vox_modules/local_store.db";
            if let Ok(store) = vox_pm::CodeStore::open(store_path).await {
                let versions = store
                    .get_package_versions(package_name)
                    .await
                    .unwrap_or_default();
                if versions.is_empty() {
                    println!("Package `{package_name}` not found.");
                } else {
                    println!("Local package: {package_name}");
                    for (ver, hash) in &versions {
                        println!("  {ver} (hash: {})", &hash[..8.min(hash.len())]);
                    }
                }
            } else {
                println!("Package `{package_name}` not found.");
            }
        }
    }

    Ok(())
}
