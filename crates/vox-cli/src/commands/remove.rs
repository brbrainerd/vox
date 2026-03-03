use anyhow::{Context, Result};
use std::path::PathBuf;

/// `vox remove <dep>` — remove a dependency from Vox.toml.
pub async fn run(dep_name: &str) -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let mut manifest = vox_pm::VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Run `vox init` first.")?;

    if manifest.remove_dependency(dep_name) {
        let toml_content = manifest
            .to_toml_string()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        std::fs::write(&manifest_path, &toml_content)
            .with_context(|| "Failed to write Vox.toml")?;
        println!("✓ Removed `{dep_name}` from [dependencies]");
    } else {
        println!("⚠ `{dep_name}` was not found in [dependencies]");
    }

    Ok(())
}
