use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::io::{self, Write};

/// Authentication configuration stored in ~/.vox/auth.json
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub registries: HashMap<String, RegistryAuth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub token: String,
    pub username: Option<String>,
}

/// `vox login` — authenticate with Vox AI providers, VoxPM, or other OCI registries.
///
/// If token is absent, launches an interactive wizard.
pub async fn run(token: Option<&str>, registry: Option<&str>, username: Option<&str>) -> Result<()> {
    let (final_registry, final_token, final_user) = match token {
        Some(t) => {
            // Non-interactive mode
            let reg = registry.unwrap_or("voxpm").to_string();
            (reg, t.trim().to_string(), username.map(|s| s.to_string()))
        }
        None => {
            // Interactive wizard
            interactive_wizard(registry.unwrap_or("google")).await?
        }
    };

    let config_dir = dirs_path();
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create {}", config_dir.display()))?;
    }

    let auth_path = config_dir.join("auth.json");
    let mut config = if auth_path.exists() {
        let content = std::fs::read_to_string(&auth_path)?;
        serde_json::from_str::<AuthConfig>(&content).unwrap_or_default()
    } else {
        AuthConfig::default()
    };

    config.registries.insert(
        final_registry.clone(),
        RegistryAuth {
            token: final_token.clone(),
            username: final_user.clone(),
        },
    );

    let content = serde_json::to_string_pretty(&config)?;
    std::fs::write(&auth_path, content).with_context(|| "Failed to save auth config")?;

    println!("\n  \x1b[32m✓\x1b[0m Successfully logged in to: \x1b[1;36m{}\x1b[0m", final_registry);
    println!("    Credentials saved to {}", auth_path.display());

    // Hint for next steps after first setup
    if final_registry == "google" || final_registry == "openrouter" {
        println!("\n  You are ready to use Vox AI! Try running: \x1b[1mvox chat\x1b[0m or \x1b[1mvox doctor\x1b[0m");
    }

    Ok(())
}

async fn interactive_wizard(default_registry: &str) -> Result<(String, String, Option<String>)> {
    println!();
    println!("  \x1b[1;36m╔══════════════════════════════════════════╗\x1b[0m");
    println!("  \x1b[1;36m║          Vox Authentication Setup        ║\x1b[0m");
    println!("  \x1b[1;36m╚══════════════════════════════════════════╝\x1b[0m");
    println!();
    println!("  Which service do you want to configure?");
    println!("    \x1b[1m1.\x1b[0m Google AI Studio \x1b[2m(Free Gemini tier, recommended)\x1b[0m");
    println!("    \x1b[1m2.\x1b[0m OpenRouter \x1b[2m(Free & Paid models, diverse)\x1b[0m");
    println!("    \x1b[1m3.\x1b[0m VoxPM Registry \x1b[2m(For publishing Vox packages)\x1b[0m");
    println!("    \x1b[1m4.\x1b[0m Custom Registry UI");
    println!();

    let mut choice = String::new();
    print!("  Select an option [1-4, or type text]: ");
    io::stdout().flush()?;
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    let registry = match choice {
        "1" | "google" => "google".to_string(),
        "2" | "openrouter" => "openrouter".to_string(),
        "3" | "voxpm" => "voxpm".to_string(),
        c if !c.is_empty() => c.to_string(),
        _ => default_registry.to_string()
    };

    println!();

    if registry == "google" {
        println!("  \x1b[1mGoogle AI Studio\x1b[0m offers the most generous free tier.");
        println!("  Get a key here: \x1b[36mhttps://aistudio.google.com/apikey\x1b[0m");
    } else if registry == "openrouter" {
        println!("  \x1b[1mOpenRouter\x1b[0m aggregates dozens of models.");
        println!("  Get a key here: \x1b[36mhttps://openrouter.ai/settings/keys\x1b[0m");
    } else if registry == "voxpm" {
        println!("  \x1b[1mVoxPM\x1b[0m requires an API token to publish packages.");
        println!("  Copy token from: \x1b[36mhttps://github.com/brbrainerd/vox/settings\x1b[0m");
    }

    println!();
    print!("  Paste your API key/token: ");
    io::stdout().flush()?;

    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();

    if token.is_empty() {
        anyhow::bail!("Login cancelled: no token provided.");
    }

    let mut username = None;
    if registry == "voxpm" || registry.contains("cr.io") {
        print!("  Username (optional): ");
        io::stdout().flush()?;
        let mut u = String::new();
        io::stdin().read_line(&mut u)?;
        let u = u.trim();
        if !u.is_empty() {
            username = Some(u.to_string());
        }
    }

    Ok((registry, token, username))
}

pub fn get_auth(registry: &str) -> Option<RegistryAuth> {
    let config_dir = dirs_path();
    let auth_path = config_dir.join("auth.json");
    if !auth_path.exists() {
        // Fallback to legacy auth_token if it exists and we're looking for voxpm
        if registry == "voxpm" {
            let legacy_path = config_dir.join("auth_token");
            if let Ok(token) = std::fs::read_to_string(legacy_path) {
                return Some(RegistryAuth {
                    token: token.trim().to_string(),
                    username: None,
                });
            }
        }
        return None;
    }

    let content = std::fs::read_to_string(auth_path).ok()?;
    let config = serde_json::from_str::<AuthConfig>(&content).ok()?;
    config.registries.get(registry).map(|a| RegistryAuth {
        token: a.token.clone(),
        username: a.username.clone(),
    })
}

/// Get the VoxPM config directory (~/.vox/).
pub fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".vox")
}
