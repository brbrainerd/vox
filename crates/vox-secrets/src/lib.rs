//! Secret resolution + redaction (env, vault, infisical, vox-vault backends).

pub mod backend;
pub mod errors;
pub mod policy;
pub mod redact;
pub mod resolver;
#[cfg(test)]
mod semcov_wave45_tests;
pub mod sources;
pub mod spec;
#[cfg(test)]
mod tests;
mod types;

pub use backend::vox_vault::{VaultHealth, cloudless_vault_env_diagnostic, probe_vault_health};
pub use errors::SecretError;
pub use policy::{MissingBehavior, SecretPolicy};
pub use resolver::ResolveProfile;
use resolver::{ResolveOptions, SecretResolver};
pub use spec::{
    Capability, Profile, RequirementMode, RequirementSet, RotationPolicy, SecretBundle,
    SecretClass, SecretId, SecretMaterialKind, SecretMetadata, SecretSpec, Workflow,
    WorkflowRequirements, all_bundle_doc_names, all_specs, capabilities_for_secret,
    managed_secret_env_names, required_for, required_for_profile, requirements_for_bundle,
    requirements_for_profile, requirements_for_profile_mode,
};
pub use types::{ResolutionStatus, ResolvedSecret, SecretSource};

pub const OPERATOR_SECRETS_CUTOVER_PHASE: &str = "VOX_SECRETS_CUTOVER_PHASE";
pub const OPERATOR_SECRETS_MIGRATION_PHASE: &str = "VOX_SECRETS_MIGRATION_PHASE";
pub const OPERATOR_SECRETS_HARD_CUT: &str = "VOX_SECRETS_HARD_CUT";
pub const OPERATOR_SECRETS_AUTO_PREFER_VAULT: &str = "VOX_SECRETS_AUTO_PREFER_VAULT";
pub const OPERATOR_SECRETS_KEK_REF: &str = "VOX_SECRETS_KEK_REF";
pub const OPERATOR_SECRETS_KEK_VERSION: &str = "VOX_SECRETS_KEK_VERSION";
pub const OPERATOR_SECRETS_AUTO_VAULT: &str = "VOX_SECRETS_AUTO_VAULT";
pub const OPERATOR_SECRETS_VAULT_URL: &str = "VOX_SECRETS_VAULT_URL";
pub const OPERATOR_SECRETS_VAULT_PATH: &str = "VOX_SECRETS_VAULT_PATH";
pub const OPERATOR_SECRETS_VAULT_TOKEN: &str = "VOX_SECRETS_VAULT_TOKEN";
pub const OPERATOR_ACCOUNT_ID: &str = "VOX_ACCOUNT_ID";
pub const OPERATOR_SECRETS_PROFILE: &str = "VOX_SECRETS_PROFILE";
pub const OPERATOR_SECRETS_BACKEND: &str = "VOX_SECRETS_BACKEND";
pub const OPERATOR_INFISICAL_TOKEN: &str = "INFISICAL_TOKEN";
pub const OPERATOR_INFISICAL_SERVICE_TOKEN: &str = "INFISICAL_SERVICE_TOKEN";
pub const OPERATOR_VAULT_ADDR: &str = "VAULT_ADDR";
pub const OPERATOR_VAULT_TOKEN: &str = "VAULT_TOKEN";
pub const OPERATOR_TURSO_URL: &str = "VOX_TURSO_URL";
pub const OPERATOR_TURSO_TOKEN: &str = "VOX_TURSO_TOKEN";

pub const OPERATOR_SCIENTIA_CROSSREF_MAILTO: &str = "VOX_SCIENTIA_CROSSREF_MAILTO";
pub const OPERATOR_SCHOLARLY_ADAPTER: &str = "VOX_SCHOLARLY_ADAPTER";
pub const OPERATOR_SCHOLARLY_JOB_LOCK_OWNER: &str = "VOX_SCHOLARLY_JOB_LOCK_OWNER";
pub const OPERATOR_ZENODO_HTTP_MAX_ATTEMPTS: &str = "VOX_ZENODO_HTTP_MAX_ATTEMPTS";
pub const OPERATOR_ZENODO_API_BASE: &str = "VOX_ZENODO_API_BASE";
pub const OPERATOR_OPENREVIEW_HTTP_MAX_ATTEMPTS: &str = "VOX_OPENREVIEW_HTTP_MAX_ATTEMPTS";
pub const OPERATOR_ZENODO_STAGING_DIR: &str = "VOX_ZENODO_STAGING_DIR";
pub const OPERATOR_ZENODO_UPLOAD_ALLOWLIST: &str = "VOX_ZENODO_UPLOAD_ALLOWLIST";
pub const OPERATOR_SYNDICATION_TEMPLATE_PROFILE: &str = "VOX_SYNDICATION_TEMPLATE_PROFILE";
pub const OPERATOR_NEWS_PUBLISH_ARMED: &str = "VOX_NEWS_PUBLISH_ARMED";
pub const OPERATOR_NEWS_SITE_BASE_URL: &str = "VOX_NEWS_SITE_BASE_URL";
pub const OPERATOR_NEWS_RSS_FEED_PATH: &str = "VOX_NEWS_RSS_FEED_PATH";
pub const OPERATOR_SCIENTIA_RESEARCH_MESH_INTAKE_WRITER: &str =
    "VOX_SCIENTIA_RESEARCH_MESH_INTAKE_WRITER";
pub const OPERATOR_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL: &str =
    "VOX_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL";
pub const OPERATOR_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL_INTERVAL_MS: &str =
    "VOX_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL_INTERVAL_MS";

/// Array of system operator tuning environment variables.
pub const OPERATOR_TUNING_ENVS: &[&str] = &[
    OPERATOR_SECRETS_CUTOVER_PHASE,
    OPERATOR_SECRETS_MIGRATION_PHASE,
    OPERATOR_SECRETS_HARD_CUT,
    OPERATOR_SECRETS_AUTO_PREFER_VAULT,
    OPERATOR_SECRETS_KEK_REF,
    OPERATOR_SECRETS_KEK_VERSION,
    OPERATOR_SECRETS_AUTO_VAULT,
    OPERATOR_SECRETS_VAULT_URL,
    OPERATOR_SECRETS_VAULT_PATH,
    OPERATOR_SECRETS_VAULT_TOKEN,
    OPERATOR_ACCOUNT_ID,
    OPERATOR_SECRETS_PROFILE,
    OPERATOR_SECRETS_BACKEND,
    OPERATOR_INFISICAL_TOKEN,
    OPERATOR_INFISICAL_SERVICE_TOKEN,
    OPERATOR_VAULT_ADDR,
    OPERATOR_VAULT_TOKEN,
    OPERATOR_TURSO_URL,
    OPERATOR_TURSO_TOKEN,
    OPERATOR_SCIENTIA_CROSSREF_MAILTO,
    OPERATOR_SCHOLARLY_ADAPTER,
    OPERATOR_SCHOLARLY_JOB_LOCK_OWNER,
    OPERATOR_ZENODO_HTTP_MAX_ATTEMPTS,
    OPERATOR_ZENODO_API_BASE,
    OPERATOR_OPENREVIEW_HTTP_MAX_ATTEMPTS,
    OPERATOR_ZENODO_STAGING_DIR,
    OPERATOR_ZENODO_UPLOAD_ALLOWLIST,
    OPERATOR_SYNDICATION_TEMPLATE_PROFILE,
    OPERATOR_NEWS_PUBLISH_ARMED,
    OPERATOR_NEWS_SITE_BASE_URL,
    OPERATOR_NEWS_RSS_FEED_PATH,
    OPERATOR_SCIENTIA_RESEARCH_MESH_INTAKE_WRITER,
    OPERATOR_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL,
    OPERATOR_SCIENTIA_RESEARCH_MESH_CONSUMER_POLL_INTERVAL_MS,
    "VOX_DB_URL",
    "VOX_APP_DB_URL",
    "VOX_DB_TOKEN",
    "VOX_ACCOUNT_ID",
    "VOX_MODEL",
    "VOX_BUDGET_USD",
    "VOX_DATA_DIR",
    "VOX_MCP_BINARY",
    "VOX_GAMIFY_ENABLED",
    "VOX_GAMIFY_MODE",
    "VOX_WEB_RUN_MODE",
    "VOX_WEB_TANSTACK_START",
    "VOX_WEB_TANSTACK_START",
    "VOX_MESH_ENABLED",
    "VOX_MESH_MODE",
    "VOX_MESH_NODE_ID",
    "VOX_MESH_LABELS",
    "VOX_MESH_CONTROL_ADDR",
    "VOX_MESH_REGISTRY_PATH",
    "VOX_MESH_ADVERTISE_GPU",
    "VOX_MESH_SCOPE_ID",
    "VOX_MESH_BOOTSTRAP_TOKEN",
    "VOX_MESH_BOOTSTRAP_EXPIRES_UNIX_MS",
    "VOX_MESH_SERVER_STALE_PRUNE_MS",
    "VOX_MESH_A2A_MAX_MESSAGES",
    "VOX_MESH_A2A_LEASE_MS",
    "VOX_ORCHESTRATOR_MESH_CONTROL_URL",
    "VOX_OPENCLAW_URL",
    "VOX_OPENCLAW_WS_URL",
    "OPENROUTER_MODEL",
    "OPENAI_MODEL",
    "OPENAI_BASE_URL",
    "GEMINI_MODEL",
    "OLLAMA_URL",
    "OLLAMA_MODEL",
    "TOGETHER_FINETUNE_MODEL",
    "GEMINI_DIRECT_MODEL",
    "OPENROUTER_GEMINI_MODEL",
    "VOX_POPULI_LOCAL_OLLAMA_URL",
    "VOX_ORCHESTRATOR_PLAN_LLM_SYNTHESIS",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMode {
    Auto,
    EnvOnly,
    Infisical,
    Vault,
    VoxCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CutoverPhase {
    #[default]
    Shadow,
    Canary,
    Enforce,
    Decommission,
}

impl CutoverPhase {
    #[must_use]
    fn from_env() -> Self {
        match std::env::var(crate::OPERATOR_SECRETS_CUTOVER_PHASE)
            .or_else(|_| std::env::var(crate::OPERATOR_SECRETS_MIGRATION_PHASE))
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("shadow") => Self::Shadow,
            Some("canary") => Self::Canary,
            Some("enforce") => Self::Enforce,
            Some("decommission") => Self::Decommission,
            _ => Self::Shadow,
        }
    }

    #[must_use]
    const fn legacy_sources_allowed(self, profile: ResolveProfile) -> bool {
        match self {
            CutoverPhase::Shadow => true,
            CutoverPhase::Canary => !profile.is_strict(),
            CutoverPhase::Enforce | CutoverPhase::Decommission => false,
        }
    }

    #[must_use]
    const fn force_vox_cloud_backend(self) -> bool {
        matches!(self, CutoverPhase::Decommission)
    }
}

impl BackendMode {
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(crate::OPERATOR_SECRETS_BACKEND)
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("env_only") | Some("env") => Self::EnvOnly,
            Some("infisical") => Self::Infisical,
            Some("vault") => Self::Vault,
            Some("vox_cloud") | Some("voxcloud") => Self::VoxCloud,
            _ => Self::Auto,
        }
    }
}

fn resolve_with_backend<B: backend::SecretBackend>(
    backend: B,
    id: SecretId,
    options: ResolveOptions,
) -> ResolvedSecret {
    SecretResolver::new(backend).resolve(id, &options)
}

#[must_use]
pub fn resolve_secret(id: SecretId) -> ResolvedSecret {
    resolve_secret_with_context(id, "process")
}

/// Persist a managed secret to the Clavis vault at runtime (account/profile-scoped).
///
/// Writes the plaintext into the cloudless vault keyed by the secret's canonical
/// env name under the active `VOX_ACCOUNT_ID`. Pass `profile` to write a
/// profile-scoped override; `None` writes the account-level canonical record.
///
/// # Errors
/// Returns [`SecretError`] if the vault backend cannot be initialized (e.g. the
/// keyring-backed master key is unavailable) or the write fails.
pub fn store_secret(
    id: SecretId,
    plaintext: &str,
    profile: Option<&str>,
) -> Result<(), SecretError> {
    let backend = backend::vox_vault::VoxCloudBackend::new()?;
    backend.write_secret_v2(
        id.spec().canonical_env,
        plaintext,
        profile,
        "create",
        Some("programmatic-store"),
        "process",
        backend::vox_vault::DEFAULT_HISTORY_DEPTH,
    )
}

/// Find a managed secret by its canonical environment name.
///
/// This is the shared lookup used by every user-facing secret writer so the
/// CLI and GUI cannot drift from the registry metadata.
#[must_use]
pub fn secret_id_for_canonical_env(canonical_env: &str) -> Option<SecretId> {
    all_specs()
        .iter()
        .find(|spec| spec.canonical_env == canonical_env)
        .map(|spec| spec.id)
}

/// Delete a managed secret from the active Clavis account.
///
/// The caller supplies a registry-defined [`SecretId`], preventing arbitrary
/// vault keys from becoming an alternate secret-management surface.
///
/// # Errors
/// Returns [`SecretError`] when the Clavis vault cannot be initialized or the
/// delete operation fails.
pub fn delete_secret(id: SecretId) -> Result<bool, SecretError> {
    let backend = backend::vox_vault::VoxCloudBackend::new()?;
    backend.delete_secret(id.spec().canonical_env)
}
#[must_use]
pub fn resolve_secret_for_cli(id: SecretId) -> ResolvedSecret {
    resolve_secret_with_context(id, "cli")
}

#[must_use]
pub fn resolve_secret_with_context(id: SecretId, context: &str) -> ResolvedSecret {
    let normalized_context = match context {
        "cli" | "mcp" | "api" => context,
        c if c.starts_with("agent:") && c.len() <= 134 => c,
        _ => "process",
    };

    let profile = resolve_profile_from_env();
    let phase = CutoverPhase::from_env();
    let legacy_allowed = phase.legacy_sources_allowed(profile);
    let options = ResolveOptions {
        include_env: legacy_allowed,
        include_auth_json: legacy_allowed,
        include_populi_env: legacy_allowed,
        profile,
        caller_context: normalized_context.to_string(),
    };

    resolve_secret_internal(id, options)
}

fn resolve_secret_internal(id: SecretId, options: ResolveOptions) -> ResolvedSecret {
    let phase = CutoverPhase::from_env();
    if phase.force_vox_cloud_backend() {
        return resolve_vox_cloud(id, options);
    }

    match BackendMode::from_env() {
        BackendMode::EnvOnly => resolve_with_backend(backend::NoopBackend, id, options),
        BackendMode::Infisical => resolve_infisical(id, options.profile, &options.caller_context),
        BackendMode::Vault => resolve_vault(id, options.profile, &options.caller_context),
        BackendMode::VoxCloud => resolve_vox_cloud(id, options),
        BackendMode::Auto => {
            let profile = options.profile;

            if std::env::var(crate::OPERATOR_INFISICAL_TOKEN).is_ok()
                || std::env::var(crate::OPERATOR_INFISICAL_SERVICE_TOKEN).is_ok()
            {
                return resolve_infisical(id, profile, &options.caller_context);
            }
            if std::env::var(crate::OPERATOR_VAULT_ADDR).is_ok()
                && std::env::var(crate::OPERATOR_VAULT_TOKEN).is_ok()
            {
                return resolve_vault(id, profile, &options.caller_context);
            }
            // The local Clavis vault is the default managed store. Its master
            // key has an encrypted file fallback when the OS keyring is
            // unavailable, so keyring presence must not gate normal
            // resolution. The resolver still falls through to legacy sources
            // only after the vault has no value.
            resolve_vox_cloud(id, options)
        }
    }
}

fn resolve_vox_cloud(id: SecretId, options: ResolveOptions) -> ResolvedSecret {
    match backend::vox_vault::VoxCloudBackend::new() {
        Ok(backend) => resolve_with_backend(backend, id, options),
        Err(e) => resolve_with_backend(
            backend::UnavailableBackend {
                reason: format!("VoxCloud backend failed to init: {}", e),
            },
            id,
            options,
        ),
    }
}

#[must_use]
pub fn resolve_env_only(id: SecretId) -> ResolvedSecret {
    SecretResolver::new(backend::NoopBackend).resolve(id, &ResolveOptions::default())
}

fn resolve_infisical(id: SecretId, profile: ResolveProfile, context: &str) -> ResolvedSecret {
    #[cfg(feature = "secrets-infisical")]
    {
        return resolve_with_backend(
            backend::infisical::InfisicalBackend,
            id,
            ResolveOptions {
                include_env: true,
                include_auth_json: true,
                include_populi_env: true,
                profile,
                caller_context: context.to_string(),
            },
        );
    }
    #[cfg(not(feature = "secrets-infisical"))]
    {
        resolve_with_backend(
            backend::UnavailableBackend {
                reason: "secrets-infisical feature is not enabled".to_string(),
            },
            id,
            ResolveOptions {
                include_env: true,
                include_auth_json: true,
                include_populi_env: true,
                profile,
                caller_context: context.to_string(),
            },
        )
    }
}

fn resolve_vault(id: SecretId, profile: ResolveProfile, context: &str) -> ResolvedSecret {
    #[cfg(feature = "secrets-vault")]
    {
        return resolve_with_backend(
            backend::vault::VaultBackend,
            id,
            ResolveOptions {
                include_env: true,
                include_auth_json: true,
                include_populi_env: true,
                profile,
                caller_context: context.to_string(),
            },
        );
    }
    #[cfg(not(feature = "secrets-vault"))]
    {
        resolve_with_backend(
            backend::UnavailableBackend {
                reason: "secrets-vault feature is not enabled".to_string(),
            },
            id,
            ResolveOptions {
                include_env: true,
                include_auth_json: true,
                include_populi_env: true,
                profile,
                caller_context: context.to_string(),
            },
        )
    }
}

fn resolve_profile_from_env() -> ResolveProfile {
    match std::env::var("VOX_SECRETS_PROFILE")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("ci") | Some("cistrict") | Some("ci_strict") => ResolveProfile::CiStrict,
        Some("prod") | Some("prodstrict") | Some("prod_strict") => ResolveProfile::ProdStrict,
        Some("hardcut") | Some("hard_cut") | Some("hard_cut_strict") | Some("hardcutstrict") => {
            ResolveProfile::HardCutStrict
        }
        _ => ResolveProfile::DevLenient,
    }
}

pub fn set_registry_token(
    registry: &str,
    token: &str,
    username: Option<String>,
) -> Result<std::path::PathBuf, SecretError> {
    sources::auth_json::write_registry_token(registry, token, username)
}

#[must_use]
pub fn get_registry_token(registry: &str) -> Option<String> {
    sources::auth_json::read_registry_token(registry)
        .map(|(s, _)| secrecy::ExposeSecret::expose_secret(&s).to_string())
}

/// Read the username (e.g. GitHub login) stored alongside a registry token.
#[must_use]
pub fn get_registry_username(registry: &str) -> Option<String> {
    sources::auth_json::read_registry_username(registry)
}

/// Remove a registry token from both the secure store and `auth.json`.
/// Returns `true` if an entry existed. Never returns the token material.
pub fn remove_registry_token(registry: &str) -> Result<bool, SecretError> {
    sources::auth_json::remove_registry_token(registry)
}

pub fn migrate_auth_store_to_secure_store() -> Result<usize, SecretError> {
    sources::auth_json::migrate_to_secure_store()
}

#[cfg(test)]
mod managed_secret_tests {
    use super::*;

    #[test]
    fn canonical_env_lookup_returns_registered_secret_id() {
        assert_eq!(
            secret_id_for_canonical_env("OPENROUTER_API_KEY"),
            Some(SecretId::OpenRouterApiKey)
        );
        assert_eq!(secret_id_for_canonical_env("VOX_OPENROUTER_API_KEY"), None);
        assert_eq!(secret_id_for_canonical_env("NOT_A_SECRET"), None);
    }

    /// Round-trips `delete_secret` through the same store→delete→delete-again
    /// shape as `vox_vault::write_present_then_delete_absent_round_trips`,
    /// but through the `lib.rs`-level `SecretId` API (`store_secret`/
    /// `delete_secret`) that the CLI and GUI actually call, rather than the
    /// backend directly — this is the function that previously had no test
    /// of its own.
    #[test]
    #[allow(unsafe_code)]
    fn delete_secret_removes_a_stored_value_and_is_idempotent() {
        use std::sync::Mutex;
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().expect("env lock");

        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = tmp_dir.path().join("delete_secret_vault.db");
        unsafe {
            std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
            std::env::set_var("VOX_ACCOUNT_ID", "delete-secret-test-account");
        }

        let id = SecretId::OpenRouterApiKey;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("tokio rt");
        let outcome = rt.block_on(async {
            tokio::task::spawn_blocking(move || {
                if store_secret(id, "sk-delete-secret-test-0123456789", None).is_err() {
                    // Sandbox has no usable keyring/vault backend — skip cleanly,
                    // matching the sibling backend-level round-trip test.
                    return None;
                }
                let deleted_first = delete_secret(id).expect("first delete");
                let deleted_again = delete_secret(id).expect("second delete");
                Some((deleted_first, deleted_again))
            })
            .await
            .expect("join")
        });

        unsafe {
            std::env::remove_var("VOX_SECRETS_VAULT_PATH");
            std::env::remove_var("VOX_ACCOUNT_ID");
        }

        if let Some((deleted_first, deleted_again)) = outcome {
            assert!(
                deleted_first,
                "delete should report the stored row was removed"
            );
            assert!(
                !deleted_again,
                "deleting an already-absent secret must report no row removed, not error"
            );
        }
    }
}

/// A redaction-safe summary row for one managed secret.
///
/// SECURITY: every field here is non-sensitive presence/metadata. The actual
/// secret value is NEVER included — only the `head4…tail2` `redacted` preview
/// from [`ResolvedSecret::redacted`]. Safe to serialize across an IPC / GUI
/// boundary.
#[derive(Debug, Clone)]
pub struct SecretStatusRow {
    /// `Debug` form of the `SecretId` enum variant (stable identifier).
    pub id: String,
    pub canonical_env: &'static str,
    pub scope_description: &'static str,
    /// Taxonomy class slug (e.g. `"llm"`, `"platform"`).
    pub taxonomy_slug: &'static str,
    /// Registry name for auth.json-backed tokens, if any.
    pub auth_registry: Option<&'static str>,
    pub required: bool,
    pub is_present: bool,
    /// `Debug` form of the resolution status.
    pub status: String,
    /// `head4…tail2 (redacted)` preview or `(missing)` — never the raw value.
    pub redacted: String,
    /// `Debug` form of the resolution source, if resolved.
    pub source: Option<String>,
    pub remediation: &'static str,
}

/// Build a redaction-safe status row for every real (non config-only) managed
/// secret. Iterates [`all_specs`], filters out operator-tuning config, and
/// resolves each. The returned rows carry only presence + a redacted preview.
#[must_use]
pub fn list_secret_status() -> Vec<SecretStatusRow> {
    let mut out = Vec::new();
    for spec in all_specs() {
        if !crate::spec::is_user_facing_secret(spec.id) {
            continue;
        }
        let resolved = resolve_secret(spec.id);
        out.push(SecretStatusRow {
            id: format!("{:?}", spec.id),
            canonical_env: spec.canonical_env,
            scope_description: spec.scope_description,
            taxonomy_slug: crate::spec::taxonomy_class_for(spec.id).slug(),
            auth_registry: spec.auth_registry,
            required: spec.policy.required,
            is_present: resolved.is_present(),
            status: format!("{:?}", resolved.status),
            redacted: resolved.redacted(),
            source: resolved.source.map(|s| format!("{s:?}")),
            remediation: spec.remediation,
        });
    }
    out
}

/// The currently-active secret resolution profile, derived from
/// `VOX_SECRETS_PROFILE` (mirrors the precedence in [`resolve_secret`]).
///
/// Non-sensitive — exposes only the profile selector, never any material.
#[must_use]
pub fn active_resolve_profile() -> ResolveProfile {
    resolve_profile_from_env()
}

/// Probe whether the active secrets backend is reachable.
///
/// Resolves managed specs until one reports [`ResolutionStatus::BackendUnavailable`];
/// returns the first such backend detail (if any). Mirrors the logic behind the
/// CLI `vox secrets backend-status` command. Never exposes secret material.
#[must_use]
pub fn backend_unavailable_detail() -> Option<String> {
    for spec in all_specs() {
        let res = resolve_secret(spec.id);
        if matches!(res.status, ResolutionStatus::BackendUnavailable) {
            return Some(res.detail.unwrap_or_else(|| "no detail".to_string()));
        }
    }
    None
}

/// One managed secret recognised inside a `.env` file during import.
///
/// SECURITY: carries only the source key NAME, the canonical env it maps to, and
/// a `head4…tail2` redacted preview of the value — NEVER the raw value.
#[derive(Debug, Clone)]
pub struct ImportEnvEntry {
    /// The key as written in the `.env` file (may be an alias).
    pub source_key: String,
    /// The canonical managed env name it resolves to.
    pub canonical_env: &'static str,
    /// `head4…tail2 (redacted)` preview of the value — never the raw value.
    pub redacted: String,
}

/// Result of an `.env` import (dry-run or applied).
///
/// SECURITY: `applied == false` means a dry-run that only reports recognised key
/// NAMES; `applied == true` means the values were written to the vault and only a
/// count is returned. No raw value is ever surfaced.
#[derive(Debug, Clone)]
pub struct ImportEnvResult {
    /// `true` if the values were written to the vault; `false` for a dry-run preview.
    pub applied: bool,
    /// Managed secrets recognised in the file (names + redacted preview only).
    pub entries: Vec<ImportEnvEntry>,
}

impl ImportEnvResult {
    /// Number of managed secrets recognised / imported.
    #[must_use]
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Parse a `.env` file and either preview (dry-run) or import its managed secrets
/// into the Clavis vault.
///
/// Single source of truth shared by the CLI `vox secrets import-env` command and
/// the GUI `import_env` Tauri command. Lines are simple `KEY=VALUE` pairs;
/// comments (`#`) and blanks are skipped, and surrounding quotes are stripped.
/// Only keys matching a managed [`SecretSpec`] (canonical, alias, or deprecated
/// alias) are considered.
///
/// When `apply` is `false` the values are read but NEVER stored or returned —
/// only key names + a redacted preview. When `apply` is `true` the values are
/// written to the vault and the same redaction-safe entries are returned.
///
/// # Errors
/// Returns [`SecretError`] if the file cannot be read, or (when `apply` is true)
/// if the vault backend cannot be initialized or a write fails.
pub fn import_env_from_path(
    path: &std::path::Path,
    apply: bool,
) -> Result<ImportEnvResult, SecretError> {
    // Bounded read (scaling-policy capped) instead of an unbounded `read_to_string`; still
    // surfaces a missing/unreadable file as an error so the existing error path is preserved.
    let content = vox_bounded_fs::read_utf8_path_capped(path)
        .map_err(|e| SecretError::Io(format!("could not read {}: {e}", path.display())))?;

    let backend = if apply {
        Some(backend::vox_vault::VoxCloudBackend::new()?)
    } else {
        None
    };

    let specs = all_specs();
    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim().trim_matches(|c| c == '"' || c == '\'');

        // An empty value is the `.env.example` placeholder, not a configured
        // secret. Importing it writes a blank into the vault, which then reads
        // back as "configured": an empty VOX_DB_URL masks the local SQLite
        // default ("Database not available"), and an empty GEMINI_API_KEY makes
        // `vox doctor` report "configured — (empty)".
        if val.is_empty() {
            continue;
        }

        let Some(spec) = specs.iter().find(|s| {
            s.canonical_env == key
                || s.aliases.contains(&key)
                || s.deprecated_aliases.contains(&key)
        }) else {
            continue;
        };

        if let Some(b) = &backend {
            let backend_key = spec.backend_key.unwrap_or(spec.canonical_env);
            b.write_secret(backend_key, val)?;
        }

        // On dry-run (apply == false) we are previewing raw, not-yet-imported
        // .env contents. A head4…tail2 preview can leak provider prefixes
        // (sk-…, ghp_…, AKIA…) — so for dry-run we emit a length-only
        // placeholder that reveals ZERO characters of the source value.
        let redacted = if apply {
            redact_preview(val)
        } else {
            redact_length_only(val)
        };

        entries.push(ImportEnvEntry {
            source_key: key.to_string(),
            canonical_env: spec.canonical_env,
            redacted,
        });
    }

    Ok(ImportEnvResult {
        applied: apply,
        entries,
    })
}

/// Length-only placeholder for dry-run preview — reveals NO characters of the
/// source value, only how many characters it has. Used when previewing raw,
/// not-yet-imported .env contents (which may be shown during a screen-share).
fn redact_length_only(value: &str) -> String {
    format!("•• {} chars (redacted)", value.chars().count())
}

/// `head4…tail2 (redacted)` preview of a value — never the raw value.
fn redact_preview(value: &str) -> String {
    if value.chars().count() > 6 {
        let head: String = value.chars().take(4).collect();
        let tail: String = value
            .chars()
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{head}…{tail} (redacted)")
    } else {
        "*** (redacted)".to_string()
    }
}

#[cfg(test)]
mod import_env_tests {
    use super::*;

    #[test]
    fn dry_run_recognizes_managed_keys_without_values() {
        // Pick a real managed canonical env to guarantee recognition.
        let canonical = all_specs()
            .first()
            .expect("at least one managed secret spec")
            .canonical_env;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("vox_import_env_test_{}.env", std::process::id()));
        std::fs::write(
            &path,
            format!("# comment\n\nUNMANAGED_FOO=bar\n{canonical}=supersecretvalue\n"),
        )
        .unwrap();

        let res = import_env_from_path(&path, false).expect("dry-run import");
        let _ = std::fs::remove_file(&path);

        assert!(!res.applied);
        assert_eq!(res.count(), 1, "only the managed key is recognised");
        let entry = &res.entries[0];
        assert_eq!(entry.canonical_env, canonical);
        // Dry-run redaction must reveal ZERO characters of the source value:
        // not the full value, not the head4 prefix, not the tail2 suffix.
        // "supersecretvalue" → head4 = "supe", tail2 = "ue".
        assert!(!entry.redacted.contains("supersecretvalue"));
        assert!(
            !entry.redacted.contains("supe"),
            "dry-run must not leak head4 prefix; got {:?}",
            entry.redacted
        );
        assert!(
            !entry.redacted.contains("ue"),
            "dry-run must not leak tail2 suffix; got {:?}",
            entry.redacted
        );
        // Length-only placeholder still conveys the value length + redacted marker.
        assert!(entry.redacted.contains("redacted"));
        assert!(
            entry.redacted.contains("16 chars"),
            "dry-run should report length; got {:?}",
            entry.redacted
        );
    }

    #[test]
    fn missing_file_is_an_error() {
        let path = std::path::Path::new("definitely-does-not-exist-xyz.env");
        assert!(import_env_from_path(path, false).is_err());
    }
}
