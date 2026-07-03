//! `vox ci generate-plugin-catalog-docs`
//!
//! Regenerates the two auto-generated reference docs from
//! `crates/vox-plugin-catalog/catalog.toml`. CI calls this with `--check`
//! to fail on drift; humans call it with no `--check` flag to update.

use anyhow::{Context, Result};
use std::path::PathBuf;

const DEFAULT_CATALOG_OUT: &str = "docs/src/reference/plugin-catalog.generated.md";
const DEFAULT_BUNDLES_OUT: &str = "docs/src/reference/distribution-bundles.generated.md";

pub fn run(catalog_out: Option<PathBuf>, bundles_out: Option<PathBuf>, check: bool) -> Result<()> {
    let catalog_out = catalog_out.unwrap_or_else(|| PathBuf::from(DEFAULT_CATALOG_OUT));
    let bundles_out = bundles_out.unwrap_or_else(|| PathBuf::from(DEFAULT_BUNDLES_OUT));

    let cat = vox_plugin_catalog::docs::render_catalog_md();
    let bun = vox_plugin_catalog::docs::render_bundles_md();

    if check {
        let on_disk_cat = std::fs::read_to_string(&catalog_out)
            .with_context(|| format!("reading {}", catalog_out.display()))?;
        let on_disk_bun = std::fs::read_to_string(&bundles_out)
            .with_context(|| format!("reading {}", bundles_out.display()))?;
        if on_disk_cat != cat || on_disk_bun != bun {
            anyhow::bail!(
                "Generated catalog docs are out of date.\nRun: vox ci generate-plugin-catalog-docs"
            );
        }
        println!("✓ plugin catalog docs are up to date");
        return Ok(());
    }

    if let Some(parent) = catalog_out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Some(parent) = bundles_out.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&catalog_out, &cat)
        .with_context(|| format!("writing {}", catalog_out.display()))?;
    std::fs::write(&bundles_out, &bun)
        .with_context(|| format!("writing {}", bundles_out.display()))?;
    println!(
        "✓ wrote {} ({} bytes) and {} ({} bytes)",
        catalog_out.display(),
        cat.len(),
        bundles_out.display(),
        bun.len()
    );
    Ok(())
}
