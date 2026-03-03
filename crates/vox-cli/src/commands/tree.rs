use anyhow::{Context, Result};
use std::path::PathBuf;

/// `vox tree` — display the dependency tree for the current project.
pub async fn run() -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let manifest = vox_pm::VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Run `vox init` first.")?;

    println!(
        "{} v{} ({})",
        manifest.package.name, manifest.package.version, manifest.package.kind
    );

    if manifest.dependencies.is_empty() {
        println!("  (no dependencies)");
        return Ok(());
    }

    let dep_count = manifest.dependencies.len();
    for (i, (name, spec)) in manifest.dependencies.iter().enumerate() {
        let is_last = i == dep_count - 1;
        let prefix = if is_last { "└── " } else { "├── " };
        let ver = spec.version_req().unwrap_or("*");

        let kind_tag = if spec.is_path() {
            format!(" (path: {})", spec.path().unwrap_or("?"))
        } else {
            String::new()
        };

        println!("{prefix}{name} {ver}{kind_tag}");

        // If we have a lockfile, show resolved version
        let lock_path = PathBuf::from("vox.lock");
        if lock_path.exists() {
            if let Ok(lockfile) = vox_pm::Lockfile::load(&lock_path) {
                if let Some(locked_ver) = lockfile.get_locked_version(name) {
                    let indent = if is_last { "    " } else { "│   " };
                    println!("{indent}→ resolved: {locked_ver}");

                    // Show transitive deps from lockfile
                    if let Some(pkg) = lockfile.packages.get(name) {
                        for dep in &pkg.dependencies {
                            println!("{indent}└── {dep}");
                        }
                    }
                }
            }
        }
    }

    if !manifest.dev_dependencies.is_empty() {
        println!("\n[dev-dependencies]");
        for (name, spec) in &manifest.dev_dependencies {
            let ver = spec.version_req().unwrap_or("*");
            println!("  └── {name} {ver}");
        }
    }

    Ok(())
}
