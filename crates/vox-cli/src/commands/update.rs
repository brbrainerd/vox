use anyhow::{Context, Result};
use std::path::PathBuf;

/// `vox update` — update dependencies to latest compatible versions.
pub async fn run() -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let manifest = vox_pm::VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Run `vox init` first.")?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies to update.");
        return Ok(());
    }

    println!(
        "Updating dependencies for {} v{}...",
        manifest.package.name, manifest.package.version
    );

    // Open the local store
    let store_dir = PathBuf::from(".vox_modules");
    if !store_dir.exists() {
        std::fs::create_dir_all(&store_dir).with_context(|| "Failed to create .vox_modules/")?;
    }

    let db_path = store_dir.join("local_store.db");
    let store = vox_pm::CodeStore::open(db_path.to_str().expect("valid utf8 path"))
        .await
        .with_context(|| "Failed to open local store")?;

    let mut updated = 0;
    let lock_path = PathBuf::from("vox.lock");
    let mut lockfile = if lock_path.exists() {
        vox_pm::Lockfile::load(&lock_path)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .unwrap_or_else(|_| vox_pm::Lockfile::new())
    } else {
        vox_pm::Lockfile::new()
    };

    for (name, spec) in &manifest.dependencies {
        let ver_req_str = spec.version_req().unwrap_or("*");

        // Check if there's a newer compatible version in the local registry
        let versions = store.get_package_versions(name).await.unwrap_or_default();

        if versions.is_empty() {
            println!("  ⚠ {name}: not in local registry, skipping");
            continue;
        }

        // Parse and find the best matching version
        if let Ok(req) = vox_pm::VersionReq::parse(ver_req_str) {
            let mut candidates: Vec<vox_pm::SemVer> = versions
                .iter()
                .filter_map(|(v, _)| vox_pm::SemVer::parse(v).ok())
                .filter(|v| req.matches(v))
                .collect();
            candidates.sort();

            if let Some(best) = candidates.last() {
                let hash = versions
                    .iter()
                    .find(|(v, _)| vox_pm::SemVer::parse(v).ok().as_ref() == Some(best))
                    .map(|(_, h)| h.clone())
                    .unwrap_or_default();

                let current_locked = lockfile.get_locked_version(name);
                if current_locked.as_ref() != Some(best) {
                    let old_ver = current_locked
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    println!("  ✓ {name}: {old_ver} → {best}");
                    lockfile.add(
                        name,
                        best,
                        &hash,
                        vox_pm::lockfile::PackageSource::Registry,
                        vec![],
                        vec![],
                    );
                    updated += 1;
                } else {
                    println!("  • {name}: up to date ({best})");
                }
            }
        }
    }

    // Save lockfile
    lockfile
        .save(&lock_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "Failed to save vox.lock")?;

    if updated > 0 {
        println!("\n✓ Updated {updated} package(s). Lockfile saved.");
    } else {
        println!("\n• All dependencies are up to date.");
    }

    Ok(())
}
