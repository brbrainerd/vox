use anyhow::Result;

/// `vox search <query>` — search the VoxPM registry for packages.
pub async fn run(query: &str, registry_url: Option<&str>) -> Result<()> {
    let url = registry_url.unwrap_or("https://raw.githubusercontent.com/brbrainerd/vox/main/registry");
    let client = vox_pm::RegistryClient::new(url);

    println!("Searching for `{query}`...\n");

    match client.search(query, 20, 0).await {
        Ok(results) => {
            if results.packages.is_empty() {
                println!("No packages found matching `{query}`.");
                return Ok(());
            }

            println!(
                "{:<25} {:<10} {:<12} DESCRIPTION",
                "NAME", "VERSION", "KIND"
            );
            println!("{}", "-".repeat(80));

            for pkg in &results.packages {
                let desc = pkg
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(40)
                    .collect::<String>();
                println!(
                    "{:<25} {:<10} {:<12} {}",
                    pkg.name, pkg.latest_version, pkg.kind, desc
                );
            }

            println!("\n{} packages found (showing top 20)", results.total);
        }
        Err(e) => {
            // If registry is not available, search locally
            println!("⚠ Registry unavailable ({e}), searching locally...");

            let store_path = ".vox_modules/local_store.db";
            if let Ok(store) = vox_pm::CodeStore::open(store_path).await {
                let artifacts = store.search_artifacts(query).await.unwrap_or_default();
                if artifacts.is_empty() {
                    println!("No local packages found matching `{query}`.");
                } else {
                    println!(
                        "{:<25} {:<10} {:<12} DESCRIPTION",
                        "NAME", "VERSION", "TYPE"
                    );
                    println!("{}", "-".repeat(80));
                    for art in &artifacts {
                        let desc = art
                            .description
                            .as_deref()
                            .unwrap_or("")
                            .chars()
                            .take(40)
                            .collect::<String>();
                        println!(
                            "{:<25} {:<10} {:<12} {}",
                            art.name, art.version, art.artifact_type, desc
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
