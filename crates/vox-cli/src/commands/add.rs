use anyhow::{Context, Result};
use std::path::PathBuf;

/// `vox add <dep> [--version <ver>] [--path <path>]` — add a dependency to Vox.toml.
pub async fn run(dep_name: &str, version: Option<&str>, path: Option<&str>) -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let mut manifest = vox_pm::VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Run `vox init` first.")?;

    let spec = if let Some(p) = path {
        vox_pm::DependencySpec::Detailed(vox_pm::manifest::DetailedDependency {
            version: version.map(|v| v.to_string()),
            path: Some(p.to_string()),
            git: None,
            branch: None,
            features: Vec::new(),
            optional: false,
            skills: Vec::new(),
        })
    } else {
        let ver = version.unwrap_or("*");
        vox_pm::DependencySpec::Simple(ver.to_string())
    };

    manifest.add_dependency(dep_name, spec);

    let toml_content = manifest
        .to_toml_string()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    std::fs::write(&manifest_path, &toml_content).with_context(|| "Failed to write Vox.toml")?;

    let ver_display = version.unwrap_or("*");
    println!("✓ Added `{dep_name}` ({ver_display}) to [dependencies]");

    Ok(())
}
