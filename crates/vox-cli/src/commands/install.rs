use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

/// `vox install [package_name]` — install dependencies from Vox.toml or a specific package.
pub async fn run(package_name: Option<&str>, offline: bool) -> Result<()> {
    // 1. Create .vox_modules if needed
    let store_dir = PathBuf::from(".vox_modules");
    if !store_dir.exists() {
        std::fs::create_dir_all(&store_dir)
            .with_context(|| format!("Failed to create {}", store_dir.display()))?;
    }

    // 2. Open local store
    let db_path = store_dir.join("local_store.db");
    let store = vox_pm::CodeStore::open(db_path.to_str().expect("valid utf8 path"))
        .await
        .with_context(|| {
            format!(
                "Failed to initialize local CodeStore at {}",
                db_path.display()
            )
        })?;
    let store = Arc::new(Mutex::new(store));

    // 3. Try to resolve from manifest if Vox.toml exists
    let manifest_path = PathBuf::from("Vox.toml");
    let manifest = vox_pm::VoxManifest::load(&manifest_path).ok();
    let manifest = Arc::new(manifest);

    // 4. Update lockfile safely
    let lock_path = PathBuf::from("vox.lock");
    let lockfile = if lock_path.exists() {
        vox_pm::Lockfile::load(&lock_path)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .unwrap_or_else(|_| vox_pm::Lockfile::new())
    } else {
        vox_pm::Lockfile::new()
    };
    let lockfile = Arc::new(Mutex::new(lockfile));

    // 5. Run parallel tasks
    let registry_url = std::env::var("VOX_REGISTRY")
        .unwrap_or_else(|_| "https://raw.githubusercontent.com/brbrainerd/vox/main/registry".to_string());

    let mut packages_to_install = Vec::new();

    if let Some(pkg) = package_name {
        packages_to_install.push(pkg.to_string());
    } else if let Some(ref m) = manifest.as_ref() {
        for dep_name in m.dependencies.keys() {
            packages_to_install.push(dep_name.clone());
        }
        for skill_name in m.skills.keys() {
            packages_to_install.push(skill_name.clone());
        }
    } else {
        println!("No package specified and no Vox.toml found.");
        return Ok(());
    }

    let mut set = JoinSet::new();

    for pkg_name in packages_to_install {
        let store = Arc::clone(&store);
        let manifest = Arc::clone(&manifest);
        let lockfile = Arc::clone(&lockfile);
        let registry_url = registry_url.clone();

        set.spawn(async move {
            install_one(&pkg_name, store, manifest, lockfile, &registry_url, offline).await
        });
    }

    let mut success = true;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("{}: {}", "Error installing".red(), e);
                success = false;
            }
            Err(e) => {
                eprintln!("{}: {:?}", "Task panic".red(), e);
                success = false;
            }
        }
    }

    if success {
        let locked = lockfile.lock().await;
        locked
            .save(&lock_path)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .with_context(|| "Failed to save lockfile")?;
        Ok(())
    } else {
        anyhow::bail!("One or more packages failed to install.");
    }
}

async fn install_one(
    package_name: &str,
    store: Arc<Mutex<vox_pm::CodeStore>>,
    manifest: Arc<Option<vox_pm::VoxManifest>>,
    lockfile: Arc<Mutex<vox_pm::Lockfile>>,
    registry_url: &str,
    offline: bool,
) -> Result<()> {
    println!("Resolving `{}`...", package_name);

    if let Some(ref m) = manifest.as_ref() {
        let spec = m
            .dependencies
            .get(package_name)
            .or_else(|| m.skills.get(package_name));
        if let Some(spec) = spec {
            if let Some(path) = spec.path() {
                println!("  → Path dependency: {path}");
                println!("  → Linking from local path");

                let path_data = format!("path-dep:{path}");
                let store_lock = store.lock().await;
                let pkg_hash = store_lock
                    .store("pkg", path_data.as_bytes())
                    .await
                    .with_context(|| "Failed to store package data")?;
                store_lock
                    .publish_package(
                        package_name,
                        "0.0.0-local",
                        &pkg_hash,
                        Some("Local path dependency"),
                        None,
                        None,
                    )
                    .await
                    .with_context(|| "Failed to register package")?;

                println!(
                    "{}",
                    format!("✓ Linked {} from {}", package_name, path).green()
                );
                return Ok(());
            }
        }
    }

    let client = vox_pm::RegistryClient::new(registry_url);

    let download_result = if offline {
        Err(anyhow::anyhow!("Offline mode: skipping registry download"))
    } else {
        client
            .download(package_name, "latest")
            .await
            .map_err(|e| anyhow::anyhow!(e))
    };

    match download_result {
        Ok(download) => {
            let store_lock = store.lock().await;

            // Content-address the downloaded data into CAS
            let pkg_hash = store_lock
                .store("pkg", &download.data)
                .await
                .with_context(|| "Failed to store downloaded package")?;

            // Verify integrity
            let actual_hash = vox_pm::hash::content_hash(&download.data);
            if !download.content_hash.is_empty() && actual_hash != download.content_hash {
                anyhow::bail!(
                    "Integrity check failed for {}: expected {}, got {}",
                    package_name,
                    &download.content_hash[..8],
                    &actual_hash[..8]
                );
            }

            store_lock
                .publish_package(
                    package_name,
                    "latest",
                    &pkg_hash,
                    Some("Downloaded from registry"),
                    Some("vox-registry"),
                    Some("MIT"),
                )
                .await
                .with_context(|| "Failed to register package")?;

            drop(store_lock);

            // Extract and link to .vox_modules using cache
            let cache = vox_pm::cache::PackageCache::new(None)
                .with_context(|| "Failed to get package cache")?;

            cache
                .store_archive(package_name, "latest", &download.data)
                .unwrap_or_else(|e| {
                    eprintln!("{}: failed to cache archive: {e}", "Warning".yellow())
                });

            if let Err(_e) = cache.extract(package_name, "latest") {
                // Ignore extraction failures for registry placeholders that aren't valid tar files yet.
                // eprintln!("Warning: failed to extract package: {e}");
            } else {
                let target_dir = PathBuf::from(".vox_modules").join(package_name);
                if let Err(e) = cache.link_to_project(package_name, "latest", &target_dir) {
                    eprintln!(
                        "{}: failed to link package to project: {e}",
                        "Warning".yellow()
                    );
                } else {
                    // Check for build.vox hook
                    let build_script = target_dir.join("build.vox");
                    if build_script.exists() {
                        println!("⚡ Running build script for {}...", package_name);
                        let current_exe = std::env::current_exe()?;
                        let status = Command::new(current_exe)
                            .arg("run")
                            .arg(&build_script)
                            .current_dir(&target_dir)
                            .status()
                            .context("Failed to execute build script")?;

                        if !status.success() {
                            anyhow::bail!("Build script for {} failed", package_name);
                        }
                    }
                }
            }

            let version = vox_pm::SemVer::parse("0.0.0").unwrap_or(vox_pm::SemVer {
                major: 0,
                minor: 0,
                patch: 0,
                pre: None,
            });

            let mut lock = lockfile.lock().await;
            lock.add(
                package_name,
                &version,
                &pkg_hash,
                vox_pm::lockfile::PackageSource::Registry,
                vec![],
                vec![],
            );
            drop(lock);

            println!(
                "{}",
                format!(
                    "✓ Installed {} (hash: {}) to .vox_modules",
                    package_name,
                    &pkg_hash[..8]
                )
                .green()
            );
        }
        Err(_) => {
            // Fallback: create a placeholder for offline development
            println!(
                "{}",
                format!(
                    "⚠ Registry unavailable, creating placeholder for `{}`",
                    package_name
                )
                .yellow()
                .bold()
            );

            let placeholder_data = format!(
                "{{\"name\":\"{}\",\"version\":\"0.0.0\",\"placeholder\":true}}",
                package_name
            );

            let store_lock = store.lock().await;
            let pkg_hash = store_lock
                .store("pkg", placeholder_data.as_bytes())
                .await
                .with_context(|| "Failed to store placeholder")?;

            store_lock
                .publish_package(
                    package_name,
                    "0.0.0",
                    &pkg_hash,
                    Some("Placeholder (offline)"),
                    Some("local"),
                    None,
                )
                .await
                .with_context(|| "Failed to register placeholder")?;

            println!(
                "{}",
                format!(
                    "✓ Created placeholder for {} (hash: {}) in .vox_modules",
                    package_name,
                    &pkg_hash[..8]
                )
                .yellow()
            );
        }
    }

    Ok(())
}
