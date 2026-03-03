use anyhow::{Context, Result};

/// `vox logout` — remove stored credentials for a registry.
pub async fn run(registry: Option<&str>) -> Result<()> {
    use crate::commands::login::{AuthConfig, dirs_path};

    let config_dir = dirs_path();
    let auth_path = config_dir.join("auth.json");

    if !auth_path.exists() {
        println!("No credentials found. Nothing to remove.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&auth_path).context("Failed to read auth config")?;
    let mut config: AuthConfig =
        serde_json::from_str(&content).unwrap_or_default();

    let reg_name = registry.unwrap_or("voxpm");

    if config.registries.remove(reg_name).is_some() {
        let updated = serde_json::to_string_pretty(&config)?;
        std::fs::write(&auth_path, updated).context("Failed to update auth config")?;
        println!("✓ Logged out from: {}", reg_name);
    } else {
        println!("No credentials stored for: {}", reg_name);
    }

    Ok(())
}
