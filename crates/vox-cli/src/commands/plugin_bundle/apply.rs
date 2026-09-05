//! `vox bundle apply <id>` — install every plugin in a bundle.

use crate::commands::plugin::{install, list::installed_version, list::plugins_root};
use anyhow::{Context, Result};

pub async fn run(id: &str, yes: bool) -> Result<()> {
    let plugins = vox_plugin_catalog::bundle_resolved(id)
        .with_context(|| format!("resolving bundle '{}'", id))?;

    println!("Bundle '{}' — {} plugin(s) to apply:", id, plugins.len());
    for p in &plugins {
        println!("  - {}", p.id);
    }
    println!();

    let root = plugins_root();
    let mut installed_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for p in &plugins {
        if installed_version(&root, &p.id).is_some() {
            println!("  skip  {} (already installed)", p.id);
            skipped_count += 1;
            continue;
        }

        println!("  installing {} …", p.id);
        match install::run(Some(&p.id), None, None, yes, false).await {
            Ok(()) => {
                installed_count += 1;
            }
            Err(e) => {
                eprintln!("  ✗ failed to install {}: {}", p.id, e);
                failed.push(p.id.clone());
            }
        }
    }

    println!();
    println!(
        "Summary: {} installed, {} skipped, {} failed.",
        installed_count,
        skipped_count,
        failed.len()
    );

    if !failed.is_empty() {
        anyhow::bail!(
            "bundle apply partially failed — {} plugin(s) could not be installed: {}",
            failed.len(),
            failed.join(", ")
        );
    }

    Ok(())
}
