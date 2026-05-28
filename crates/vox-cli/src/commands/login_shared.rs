//! Canonical `vox login` / `vox auth connect` / `vox secrets login` implementation.
//!
//! Persists Turso vault credentials to the OS keyring (legacy compat) and writes
//! `~/.vox/login.toml` for `VOX_ACCOUNT_ID` / `VOX_SECRETS_BACKEND` so operators can
//! `source` or align shell profiles without hard-coding secrets.

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::PathBuf;

const LOGIN_PROFILE_BASENAME: &str = "login.toml";

/// Clap arguments shared by `vox login`, `vox auth login`, `vox auth connect`, and `vox secrets login`.
#[derive(Args, Clone, Debug, Default)]
pub struct LoginArgs {
    /// Remote VoxDB / Turso database URL (overrides prompt).
    #[arg(long, env = "VOX_DB_URL")]
    pub vault_url: Option<String>,
    /// Turso auth token (overrides prompt; prefer `vox secrets set` for vault-synced secrets).
    #[arg(long, env = "VOX_DB_TOKEN")]
    pub vault_token: Option<String>,
    /// Account id for multi-device Secrets vault isolation (`VOX_ACCOUNT_ID`).
    #[arg(long = "account")]
    pub account_id: Option<String>,
    /// Secrets backend mode hint, e.g. `vox_cloud` (`VOX_SECRETS_BACKEND`).
    #[arg(long)]
    pub backend: Option<String>,
    /// Replace existing vault URL/token in keyring without prompting.
    #[arg(long, default_value_t = false)]
    pub force: bool,
    /// Require `--vault-url` and `--vault-token` (no stdin prompts).
    #[arg(long, default_value_t = false)]
    pub non_interactive: bool,
}

/// Options for [`run_login`] (same shape as [`LoginArgs`]).
#[derive(Clone, Debug, Default)]
pub struct LoginOpts {
    pub vault_url: Option<String>,
    pub vault_token: Option<String>,
    pub account_id: Option<String>,
    pub backend: Option<String>,
    pub force: bool,
    pub non_interactive: bool,
}

impl From<LoginArgs> for LoginOpts {
    fn from(a: LoginArgs) -> Self {
        Self {
            vault_url: a.vault_url,
            vault_token: a.vault_token,
            account_id: a.account_id,
            backend: a.backend,
            force: a.force,
            non_interactive: a.non_interactive,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct LoginProfileToml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account_id: Option<String>,
    /// Secrets backend mode (TOML key kept as `clavis_backend` for backward compat of existing login.toml files).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "clavis_backend"
    )]
    secrets_backend: Option<String>,
}

fn login_profile_path() -> PathBuf {
    vox_config::paths::dot_vox_user_dir().join(LOGIN_PROFILE_BASENAME)
}

#[cfg(feature = "keyring-store")]
fn keyring_has_vault_config() -> bool {
    let url_ok = keyring::Entry::new("vox-secrets-env", "turso-url")
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.trim().is_empty())
        .is_some();
    let tok_ok = keyring::Entry::new("vox-secrets-env", "turso-token")
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.trim().is_empty())
        .is_some();
    url_ok && tok_ok
}

#[cfg(not(feature = "keyring-store"))]
fn keyring_has_vault_config() -> bool {
    false
}

fn read_login_profile_toml() -> Option<LoginProfileToml> {
    let path = login_profile_path();
    let raw = std::fs::read_to_string(&path).ok()?;
    toml::from_str::<LoginProfileToml>(&raw).ok()
}

/// Summarize persisted login state for `vox doctor` (no secrets).
#[must_use]
pub fn login_status_summary() -> String {
    let profile = read_login_profile_toml();
    let acct = profile
        .as_ref()
        .and_then(|p| p.account_id.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| {
            std::env::var(vox_secrets::OPERATOR_ACCOUNT_ID)
                .ok()
                .filter(|s| !s.trim().is_empty())
        });
    let backend = profile
        .as_ref()
        .and_then(|p| p.secrets_backend.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let vault_ok = keyring_has_vault_config();
    let handshake = match vox_secrets::backend::vox_vault::VoxCloudBackend::new() {
        Ok(_) => "reachable".to_string(),
        Err(e) => format!("unreachable ({e:?})"),
    };

    format!(
        "login.toml_profile={}; vault_keyring_configured={vault_ok}; account_id={}; secrets_backend={}; vault_handshake={handshake}",
        login_profile_path().display(),
        acct.as_deref().unwrap_or("(none)"),
        backend.as_deref().unwrap_or("(none)"),
    )
}

fn write_login_profile(opts: &LoginOpts) -> Result<()> {
    let path = login_profile_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let profile = LoginProfileToml {
        account_id: opts.account_id.clone(),
        secrets_backend: opts.backend.clone(),
    };
    let body = toml::to_string_pretty(&profile).context("serialize login.toml")?;
    std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    println!("Wrote {}", path.display());
    println!(
        "Hint: export VOX_ACCOUNT_ID and VOX_SECRETS_BACKEND from this file or merge into your shell profile."
    );
    Ok(())
}

/// Vault handshake: ensures local cloudless schema exists when backend is cloud-capable.
fn validate_vault_handshake() -> Result<()> {
    let _ = vox_secrets::backend::vox_vault::VoxCloudBackend::new()
        .map_err(|e| anyhow::anyhow!("Secrets vault handshake failed: {e:?}"))?;
    Ok(())
}

/// Canonical login: store Turso URL/token in keyring, optional account/backend in `login.toml`.
pub async fn run_login(opts: LoginOpts) -> Result<()> {
    if keyring_has_vault_config() && !opts.force && !opts.non_interactive {
        print!(
            "Vault URL/token already set in keyring. Re-configure? [y/N] (use --force to skip): "
        );
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        if !matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("Login unchanged.");
            return Ok(());
        }
    } else if keyring_has_vault_config() && !opts.force && opts.non_interactive {
        println!("Vault already configured (keyring). Use --force to replace.");
        write_login_profile(&opts)?;
        return Ok(());
    }

    let mut url = opts.vault_url.clone();
    let mut token = opts.vault_token.clone();

    if opts.non_interactive {
        url = Some(
            url.filter(|s| !s.trim().is_empty())
                .context("--non-interactive requires --vault-url")?,
        );
        token = Some(
            token
                .filter(|s| !s.trim().is_empty())
                .context("--non-interactive requires --vault-token")?,
        );
    } else {
        if url.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            println!("Connecting zero-knowledge VoxDB Vault (formerly `vox auth connect`)...");
            print!("Vault DB URL: ");
            io::stdout().flush()?;
            let mut s = String::new();
            io::stdin().read_line(&mut s)?;
            url = Some(s.trim().to_string());
        }
        if token.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            print!("Vault Auth Token: ");
            io::stdout().flush()?;
            let mut s = String::new();
            io::stdin().read_line(&mut s)?;
            token = Some(s.trim().to_string());
        }
    }

    let url = url.as_deref().unwrap_or("").trim();
    let token = token.as_deref().unwrap_or("").trim();
    if url.is_empty() || token.is_empty() {
        anyhow::bail!("Vault URL and token must be non-empty");
    }

    #[cfg(feature = "keyring-store")]
    {
        let keyring = keyring::Entry::new("vox-secrets-env", "turso-url").context(
            "Failed to instantiate keyring for turso-url. Keyring may not be available.",
        )?;
        keyring
            .set_password(url)
            .context("Failed to set turso-url in keyring.")?;

        let keyring_token = keyring::Entry::new("vox-secrets-env", "turso-token")
            .context("Failed to instantiate keyring for turso-token.")?;
        keyring_token
            .set_password(token)
            .context("Failed to set turso-token in keyring.")?;
    }
    #[cfg(not(feature = "keyring-store"))]
    {
        anyhow::bail!(
            "This build was compiled without keyring support. \
             Set VOX_DB_URL and VOX_DB_TOKEN environment variables instead."
        );
    }

    if let Err(e) = validate_vault_handshake() {
        tracing::warn!(error = %e, "post-login Secrets vault handshake failed (vault may still be usable)");
    }

    write_login_profile(&opts)?;

    println!("Vault configuration complete.");
    println!("Run `vox config sync --pull` to synchronize configurations from the vault.");
    Ok(())
}

/// Best-effort logout: clear keyring entries and remove `login.toml`.
pub async fn run_logout() -> Result<()> {
    #[cfg(feature = "keyring-store")]
    for (service, user) in [
        ("vox-secrets-env", "turso-url"),
        ("vox-secrets-env", "turso-token"),
    ] {
        if let Ok(e) = keyring::Entry::new(service, user) {
            let _ = e.delete_credential();
        }
    }
    let path = login_profile_path();
    if path.exists() {
        std::fs::remove_file(&path).ok();
        println!("Removed {}", path.display());
    }
    println!("Logged out of local vault keyring entries.");
    Ok(())
}
