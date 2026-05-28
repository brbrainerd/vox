use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub fn run(repo_root: &PathBuf) -> Result<()> {
    tracing::info!("Running gui-catalog-parity check...");

    let catalog = crate::command_catalog::build_catalog();
    if catalog.entries.is_empty() {
        anyhow::bail!("CommandCatalog has zero entries");
    }

    for entry in &catalog.entries {
        if entry.path.is_empty() {
            anyhow::bail!("CommandCatalog contains entry with empty path");
        }
        if entry.about == "(no description)" {
            anyhow::bail!("Command 'vox {}' has placeholder about string '(no description)'. All commands must have meaningful descriptions.", entry.path.join(" "));
        }
    }

    let ts_path = repo_root.join("crates/vox-gui/ui/src/types/catalog.ts");
    if !ts_path.exists() {
        anyhow::bail!("TypeScript catalog types file missing at: {:?}", ts_path);
    }
    let ts_content = fs::read_to_string(&ts_path).context("Failed to read catalog.ts")?;
    if !ts_content.contains("CommandCatalogEntry") {
        anyhow::bail!("CommandCatalogEntry missing from catalog.ts");
    }

    let tauri_conf_path = repo_root.join("crates/vox-gui/tauri.conf.json");
    let cargo_toml_path = repo_root.join("Cargo.toml");

    let tauri_conf_content = fs::read_to_string(&tauri_conf_path).context("Failed to read tauri.conf.json")?;
    let tauri_conf: serde_json::Value = serde_json::from_str(&tauri_conf_content).context("Failed to parse tauri.conf.json")?;
    let tauri_version = tauri_conf.get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Version missing or not a string in tauri.conf.json"))?;

    let cargo_toml_content = fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;
    let mut workspace_version = None;
    let mut in_workspace_package = false;
    for line in cargo_toml_content.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_workspace_package = true;
        } else if trimmed.starts_with('[') {
            in_workspace_package = false;
        } else if in_workspace_package && trimmed.starts_with("version") {
            if let Some(v) = trimmed.split('=').nth(1) {
                workspace_version = Some(v.trim().trim_matches('"').to_string());
                break;
            }
        }
    }

    let workspace_version = workspace_version.ok_or_else(|| anyhow::anyhow!("Could not find version under [workspace.package] in Cargo.toml"))?;

    if tauri_version != workspace_version {
        anyhow::bail!(
            "Version mismatch: tauri.conf.json has '{}' but Cargo.toml [workspace.package] has '{}'",
            tauri_version,
            workspace_version
        );
    }

    tracing::info!("gui-catalog-parity check passed.");
    Ok(())
}
