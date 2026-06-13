use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};
use tracing::{error, info};
use vox_identity::trust::TrustedNodeRegistry;
use vox_mesh_types::A2ADeliverRequest;
use vox_mesh_types::SecretsSyncEnvelope;

fn redact_value(value: &str) -> String {
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
        "***".to_string()
    }
}

fn local_inference_allows_no_cloud_key() -> bool {
    match std::env::var("vox_populi::inference_PROFILE")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("cloud_openai_compatible") | Some("cloud") => false,
        Some("mobile_litert") | Some("litert") => false,
        Some("mobile_coreml") | Some("coreml") => false,
        Some("lan_gateway") | Some("lan") => true,
        Some("desktop_ollama") | Some("ollama") | None => true,
        _ => true,
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DoctorModeArg {
    Auto,
    Local,
    Cloud,
}

impl From<DoctorModeArg> for vox_secrets::RequirementMode {
    fn from(value: DoctorModeArg) -> Self {
        match value {
            DoctorModeArg::Auto => Self::Auto,
            DoctorModeArg::Local => Self::Local,
            DoctorModeArg::Cloud => Self::Cloud,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BundleArg {
    MinimalLocalDev,
    MinimalCloudDev,
    GpuCloud,
    PublishReview,
    MeshRoles,
}

impl From<BundleArg> for vox_secrets::SecretBundle {
    fn from(value: BundleArg) -> Self {
        match value {
            BundleArg::MinimalLocalDev => Self::MinimalLocalDev,
            BundleArg::MinimalCloudDev => Self::MinimalCloudDev,
            BundleArg::GpuCloud => Self::GpuCloud,
            BundleArg::PublishReview => Self::PublishReview,
            BundleArg::MeshRoles => Self::MeshRoles,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WorkflowArg {
    Chat,
    Mcp,
    Publish,
    Review,
    DbRemote,
    MensMesh,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProfileArg {
    Dev,
    Ci,
    Mobile,
    Prod,
}

impl From<WorkflowArg> for vox_secrets::Workflow {
    fn from(value: WorkflowArg) -> Self {
        match value {
            WorkflowArg::Chat => Self::Chat,
            WorkflowArg::Mcp => Self::Mcp,
            WorkflowArg::Publish => Self::Publish,
            WorkflowArg::Review => Self::Review,
            WorkflowArg::DbRemote => Self::DbRemote,
            WorkflowArg::MensMesh => Self::MensMesh,
        }
    }
}

impl From<ProfileArg> for vox_secrets::Profile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Dev => Self::Dev,
            ProfileArg::Ci => Self::Ci,
            ProfileArg::Mobile => Self::Mobile,
            ProfileArg::Prod => Self::Prod,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Human,
    JsonV1,
}

#[derive(Subcommand, Debug)]
pub enum SecretsCmd {
    /// Sign in: configure vault URL/token and optional Secrets account/backend.
    #[command(name = "login")]
    Login {
        #[command(flatten)]
        args: crate::commands::login_shared::LoginArgs,
    },
    /// Show secret readiness for a workflow (credentials / env resolution).
    /// Compatibility: `vox secrets doctor` remains accepted but hidden from help (use top-level `vox doctor` for toolchain checks).
    #[command(name = "status", alias = "doctor")]
    Status {
        #[arg(long, value_enum, default_value_t = WorkflowArg::Chat)]
        workflow: WorkflowArg,
        #[arg(long, value_enum, default_value_t = ProfileArg::Dev)]
        profile: ProfileArg,
        #[arg(long, value_enum, default_value_t = DoctorModeArg::Auto)]
        mode: DoctorModeArg,
        #[arg(long, value_enum)]
        bundle: Option<BundleArg>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Store a registry token in ~/.vox/auth.json (compat mode).
    Set {
        registry: String,
        token: String,
        #[arg(long)]
        username: Option<String>,
    },
    /// Read a registry token from resolution sources.
    Get { registry: String },
    /// Show backend mode and current availability state.
    BackendStatus,
    /// Migrate plaintext `auth.json` tokens into secure local store.
    MigrateAuthStore,
    /// Fetch unmanaged legacy signals (.env) and inject into secure storage.
    #[command(name = "import-env")]
    ImportEnv {
        /// Path to a specific .env file to import. Defaults to `.env` in current directory.
        #[arg(long)]
        file: Option<std::path::PathBuf>,
        /// If set, preview the import without writing to the vault.
        #[arg(long)]
        dry_run: bool,
    },
    /// Sync shareable secrets across the mesh.
    Sync {
        /// Sync with other nodes in the mesh.
        #[arg(long)]
        mesh: bool,
        /// If set, preview which secrets would be synced.
        #[arg(long)]
        dry_run: bool,
    },
}

pub async fn run(cmd: SecretsCmd) -> Result<()> {
    match cmd {
        SecretsCmd::Login { args } => crate::commands::login_shared::run_login(args.into()).await,
        SecretsCmd::Status {
            workflow,
            profile,
            mode,
            bundle,
            format,
        } => run_doctor(workflow, profile, mode, bundle, format).await,
        SecretsCmd::Set {
            registry,
            token,
            username,
        } => {
            let path = vox_secrets::set_registry_token(&registry, &token, username)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("Stored token for `{registry}` in {}", path.display());
            Ok(())
        }
        SecretsCmd::Get { registry } => {
            match vox_secrets::get_registry_token(&registry) {
                Some(token) => {
                    println!("{registry}: {}", redact_value(&token));
                }
                None => println!("{registry}: (missing)"),
            }
            Ok(())
        }
        SecretsCmd::BackendStatus => {
            let mode = vox_secrets::BackendMode::from_env();
            println!("secrets backend mode: {mode:?}");
            for spec in vox_secrets::all_specs() {
                let res = vox_secrets::resolve_secret(spec.id);
                if matches!(
                    res.status,
                    vox_secrets::ResolutionStatus::BackendUnavailable
                ) {
                    println!(
                        "backend status: unavailable ({})",
                        res.detail.unwrap_or_else(|| "no detail".to_string())
                    );
                    return Ok(());
                }
            }
            println!("backend status: available or env-only fallback");
            Ok(())
        }
        SecretsCmd::MigrateAuthStore => {
            let moved = vox_secrets::migrate_auth_store_to_secure_store()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("migrated {moved} auth entries to secure store");
            Ok(())
        }
        SecretsCmd::ImportEnv { file, dry_run } => {
            let path = file.unwrap_or_else(|| std::path::PathBuf::from(".env"));
            if !path.exists() {
                return Err(anyhow::anyhow!("File not found: {}", path.display()));
            }
            if dry_run {
                println!(
                    "(dry-run) Scanning {} for managed secrets...",
                    path.display()
                );
            }
            // Shared SSOT import logic lives in vox_secrets so the GUI and CLI
            // never diverge. dry_run => apply == false (names only, no writes).
            let result = vox_secrets::import_env_from_path(&path, !dry_run)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            for entry in &result.entries {
                if dry_run {
                    println!(
                        "(dry-run) Found {} -> {} (val: {})",
                        entry.source_key, entry.canonical_env, entry.redacted
                    );
                } else {
                    println!("Imported {} -> {}", entry.source_key, entry.canonical_env);
                }
            }
            if dry_run {
                println!(
                    "Dry-run complete: {} managed secrets identified in {}",
                    result.count(),
                    path.display()
                );
            } else {
                println!(
                    "Import complete: {} managed secrets injected into vault from {}",
                    result.count(),
                    path.display()
                );
            }
            Ok(())
        }
        SecretsCmd::Sync { mesh, dry_run } => run_sync(mesh, dry_run).await,
    }
}

async fn run_doctor(
    workflow: WorkflowArg,
    profile_arg: ProfileArg,
    mode: DoctorModeArg,
    bundle: Option<BundleArg>,
    format: OutputFormat,
) -> Result<()> {
    let wf = vox_secrets::Workflow::from(workflow);
    let profile = vox_secrets::Profile::from(profile_arg);
    let resolved_mode = match mode {
        DoctorModeArg::Auto if local_inference_allows_no_cloud_key() => DoctorModeArg::Local,
        DoctorModeArg::Auto => DoctorModeArg::Cloud,
        m => m,
    };
    let requirements = if let Some(bundle) = bundle {
        vox_secrets::requirements_for_bundle(vox_secrets::SecretBundle::from(bundle))
    } else {
        vox_secrets::requirements_for_profile_mode(
            wf,
            profile,
            vox_secrets::RequirementMode::from(resolved_mode),
        )
    };

    if matches!(format, OutputFormat::JsonV1) {
        emit_doctor_json_v1(wf, profile, mode, resolved_mode)
    } else {
        emit_doctor_human(workflow, profile_arg, resolved_mode, bundle, requirements)
    }
}

#[derive(serde::Serialize)]
struct DoctorJsonV1 {
    schema: &'static str,
    generated_at_ms: i64,
    workflow: String,
    profile: String,
    mode: String,
    vault_diagnostic: String,
    backend_mode: String,
    rollout_flags: vox_config::RolloutFlagSnapshot,
    secrets: Vec<DoctorSecretRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_warning: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suggested_migrations: Vec<String>,
}

#[derive(serde::Serialize)]
struct DoctorSecretRow {
    id: String,
    canonical_env: String,
    status: String,
    source: String,
    class: String,
    material_kind: String,
    capabilities: Vec<String>,
    bundle_membership: Vec<String>,
    is_present: bool,
    redacted_value: String,
    remediation: Option<String>,
    deprecated_alias_in_use: Option<String>,
    feature_gate_missing: bool,
    /// Informational note from the resolver — e.g. an env var shadowing
    /// an `auth.json` value stored via `vox secrets set`.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

fn emit_doctor_json_v1(
    workflow: vox_secrets::Workflow,
    profile: vox_secrets::Profile,
    mode: DoctorModeArg,
    _resolved_mode: DoctorModeArg,
) -> Result<()> {
    let mut secrets = Vec::new();
    let _bundles = vox_secrets::all_bundle_doc_names();

    // Mapping from SecretId to list of bundle doc names they belong to
    let mut ms: std::collections::BTreeMap<vox_secrets::SecretId, Vec<&'static str>> =
        std::collections::BTreeMap::new();
    for spec in vox_secrets::all_specs() {
        ms.insert(spec.id, Vec::new());
    }

    for &b in vox_secrets::SecretBundle::variants() {
        let reqs = vox_secrets::requirements_for_bundle(b);
        let b_name = b.doc_name();

        let mut ids = std::collections::BTreeSet::new();
        for r in &reqs.blocking {
            match r {
                vox_secrets::RequirementSet::AllOf(list)
                | vox_secrets::RequirementSet::AnyOf(list) => {
                    for &id in *list {
                        ids.insert(id);
                    }
                }
            }
        }
        for &id in &reqs.optional {
            ids.insert(id);
        }

        for id in ids {
            if let Some(list) = ms.get_mut(&id) {
                list.push(b_name);
            }
        }
    }

    for spec in vox_secrets::all_specs() {
        let resolved = vox_secrets::resolve_secret(spec.id);

        let memberships = ms
            .get(&spec.id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        let is_deprecated_alias = matches!(
            resolved.status,
            vox_secrets::ResolutionStatus::DeprecatedAliasUsed
        );

        secrets.push(DoctorSecretRow {
            id: format!("{:?}", spec.id),
            canonical_env: spec.canonical_env.to_string(),
            status: format!("{:?}", resolved.status),
            source: format!("{:?}", resolved.source),
            class: format!("{:?}", spec.id.metadata().class),
            material_kind: format!("{:?}", spec.id.metadata().material_kind),
            capabilities: vox_secrets::capabilities_for_secret(spec.id)
                .iter()
                .map(|c| format!("{:?}", c))
                .collect(),
            bundle_membership: memberships,
            is_present: resolved.is_present(),
            redacted_value: resolved.redacted(),
            remediation: if resolved.is_present() {
                None
            } else {
                Some(spec.remediation.to_string())
            },
            deprecated_alias_in_use: if is_deprecated_alias {
                Some(spec.canonical_env.to_string())
            } else {
                None
            },
            feature_gate_missing: matches!(
                resolved.status,
                vox_secrets::ResolutionStatus::BackendUnavailable
            ),
            detail: resolved.detail.clone(),
        });
    }

    let account_id = std::env::var(vox_secrets::OPERATOR_ACCOUNT_ID).unwrap_or_default();
    let account_warning = if account_id == "default-account" || account_id.is_empty() {
        Some("VOX_ACCOUNT_ID is using a default or empty value. This can cause multi-device vault conflicts.".to_string())
    } else {
        None
    };

    let mut suggested_migrations = Vec::new();
    if let Ok(content) = std::fs::read_to_string(".env") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, _)) = line.split_once('=') {
                let key = key.trim();
                if let Some(spec) = vox_secrets::all_specs()
                    .iter()
                    .find(|s| s.canonical_env == key || s.aliases.contains(&key))
                {
                    let res = vox_secrets::resolve_secret(spec.id);
                    if !matches!(res.source, Some(vox_secrets::SecretSource::SecureStore)) {
                        suggested_migrations.push(format!(
                            "migrate `{}` from .env to secure vault via `vox secrets import-env`",
                            key
                        ));
                    }
                }
            }
        }
    }

    let report = DoctorJsonV1 {
        schema: "contracts/reports/secrets-doctor.v1.json",
        generated_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        workflow: format!("{:?}", workflow),
        profile: format!("{:?}", profile),
        mode: format!("{:?}", mode),
        vault_diagnostic: vox_secrets::backend::vox_vault::cloudless_vault_env_diagnostic()
            .to_string(),
        backend_mode: format!("{:?}", vox_secrets::BackendMode::from_env()),
        rollout_flags: vox_config::rollout_flag_snapshot(),
        secrets,
        account_warning,
        suggested_migrations,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn emit_doctor_human(
    workflow: WorkflowArg,
    profile: ProfileArg,
    resolved_mode: DoctorModeArg,
    bundle: Option<BundleArg>,
    requirements: vox_secrets::WorkflowRequirements,
) -> Result<()> {
    println!("secrets doctor ({workflow:?}, {profile:?})");
    println!(
        "cloudless_vault_store: {}",
        vox_secrets::backend::vox_vault::cloudless_vault_env_diagnostic()
    );
    println!("active_mode: {resolved_mode:?}");

    let account_id = std::env::var(vox_secrets::OPERATOR_ACCOUNT_ID)
        .unwrap_or_else(|_| "default-account".to_string());
    if account_id == "default-account" {
        println!(
            "warning: VOX_ACCOUNT_ID is default-account; use a unique identifier for vault isolation"
        );
    }

    if let Ok(content) = std::fs::read_to_string(".env") {
        let mut count = 0;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, _)) = line.split_once('=') {
                let key = key.trim();
                if let Some(spec) = vox_secrets::all_specs()
                    .iter()
                    .find(|s| s.canonical_env == key || s.aliases.contains(&key))
                {
                    let res = vox_secrets::resolve_secret(spec.id);
                    if !matches!(res.source, Some(vox_secrets::SecretSource::SecureStore)) {
                        if count == 0 {
                            println!("suggested migrations (unmanaged .env keys detected):");
                        }
                        println!(
                            "  - migrate `{}` to vault via `vox secrets import-env`",
                            key
                        );
                        count += 1;
                    }
                }
            }
        }
    }
    if let Some(bundle) = bundle {
        println!("bundle: {bundle:?}");
    }

    println!("blocking_requirements:");
    for req in requirements.blocking {
        match req {
            vox_secrets::RequirementSet::AllOf(ids) => {
                let mut ok = true;
                for id in ids {
                    let resolved = vox_secrets::resolve_secret(*id);
                    println!(
                        "  - {:?}: {:?} via {:?} {}",
                        id,
                        resolved.status,
                        resolved.source,
                        resolved.redacted()
                    );
                    if let Some(detail) = &resolved.detail {
                        println!("    detail: {detail}");
                    }
                    if !resolved.is_present() {
                        ok = false;
                        println!("    remediation: {}", resolved.remediation);
                    }
                }
                println!(
                    "  => group ALL_OF status: {}",
                    if ok { "satisfied" } else { "missing" }
                );
            }
            vox_secrets::RequirementSet::AnyOf(ids) => {
                let mut ok = false;
                let mut any_present: Option<vox_secrets::SecretId> = None;
                for id in ids {
                    let resolved = vox_secrets::resolve_secret(*id);
                    println!(
                        "  - {:?}: {:?} via {:?} {}",
                        id,
                        resolved.status,
                        resolved.source,
                        resolved.redacted()
                    );
                    if resolved.is_present() {
                        ok = true;
                        any_present = Some(*id);
                    } else if let Some(detail) = &resolved.detail {
                        println!("    detail: {detail}");
                    }
                    if matches!(
                        resolved.status,
                        vox_secrets::ResolutionStatus::DeprecatedAliasUsed
                    ) {
                        println!(
                            "    warning: deprecated alias in use; migrate to `{}`",
                            id.spec().canonical_env
                        );
                    }
                }
                println!(
                    "  => group ANY_OF status: {}{}",
                    if ok { "satisfied" } else { "missing" },
                    any_present
                        .map(|id| format!(" (selected {:?})", id))
                        .unwrap_or_default()
                );
                if !ok {
                    println!(
                        "    remediation: set at least one key in this group or use local profile"
                    );
                }
            }
        }
    }

    if !requirements.optional.is_empty() {
        println!("optional_capabilities:");
        for id in requirements.optional {
            let resolved = vox_secrets::resolve_secret(id);
            println!(
                "  - {:?}: {:?} via {:?} {}",
                id,
                resolved.status,
                resolved.source,
                resolved.redacted()
            );
            if matches!(
                resolved.status,
                vox_secrets::ResolutionStatus::DeprecatedAliasUsed
            ) {
                println!(
                    "    warning: deprecated alias in use; migrate to `{}`",
                    id.spec().canonical_env
                );
            }
        }
    }

    let rf = vox_config::rollout_flag_snapshot();
    println!(
        "rollout_flags: lineage_persist={} workflow_journal_codex_persist={} db_circuit_breaker_env={} db_sync_remote_it_gate={} db_embedded_replica_it_gate={}",
        rf.orchestration_lineage_persist,
        rf.workflow_journal_codex_persist,
        rf.db_circuit_breaker_env,
        rf.db_sync_remote_integration_gate,
        rf.db_embedded_replica_integration_gate
    );
    if !rf.workflow_journal_codex_persist {
        println!(
            "warning: VOX_WORKFLOW_JOURNAL_CODEX_OFF disables Codex workflow journal append (durable replay still depends on workflow_activity_log)"
        );
    }
    if rf.db_circuit_breaker_env {
        println!(
            "warning: VOX_DB_CIRCUIT_BREAKER may gate workflow durability writes under DB stress"
        );
    }
    Ok(())
}

async fn run_sync(mesh: bool, dry_run: bool) -> Result<()> {
    if !mesh {
        println!("secrets sync: no targets specified (use --mesh)");
        return Ok(());
    }

    println!("secrets sync: identifying shareable secrets...");
    let mut shareable_secrets = Vec::new();
    for spec in vox_secrets::all_specs() {
        if spec.id.metadata().shareable {
            let res = vox_secrets::resolve_secret(spec.id);
            if let Some(val) = res.expose() {
                shareable_secrets.push((spec, val.to_string()));
            }
        }
    }

    if shareable_secrets.is_empty() {
        println!("no shareable secrets found in vault.");
        return Ok(());
    }

    println!("found {} shareable secrets.", shareable_secrets.len());

    let registry = TrustedNodeRegistry::new();
    let trusted_nodes = registry.list().context("Failed to list trusted nodes")?;

    if trusted_nodes.is_empty() {
        println!("no trusted nodes found in registry. secret sync requires established trust.");
        return Ok(());
    }

    println!("broadcasting to {} trusted nodes...", trusted_nodes.len());

    let sender_node_id = vox_config::local_user_id();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Use the local registry to find control plane URLs for trusted nodes
    let local_reg_path = vox_populi::local_registry_path();
    let populi_reg = vox_populi::LocalRegistry::new(local_reg_path)
        .load()
        .context("Failed to load populi registry")?;

    let mut success_count = 0;
    let mut fail_count = 0;

    let p_env = vox_populi::populi_env();
    for node in trusted_nodes {
        println!(
            "  syncing to node: {} ({})",
            node.node_id,
            node.label.as_deref().unwrap_or("unlabeled")
        );

        let node_record = populi_reg.nodes.iter().find(|n| n.id == node.node_id);
        let control_url = node_record
            .and_then(|n| n.listen_addr.as_ref())
            .or_else(|| {
                // Fallback to local control plane if node_id matches local or if it's the only one known
                if node.node_id == sender_node_id {
                    p_env.control_addr.as_ref()
                } else {
                    None
                }
            });

        let Some(url) = control_url else {
            println!(
                "    warning: no control plane URL found for node {}; skipping",
                node.node_id
            );
            fail_count += 1;
            continue;
        };

        let mut pk_bytes = [0u8; 32];
        hex::decode_to_slice(&node.pubkey_hex, &mut pk_bytes)
            .context("Failed to decode node public key")?;
        let pk = vox_crypto::facades::encryption_public_key_from_bytes(pk_bytes);

        let url_owned = url.to_string();

        for (spec, secret_val) in &shareable_secrets {
            if dry_run {
                println!("    [dry-run] would seal and push {}", spec.canonical_env);
                continue;
            }

            let sealed = vox_crypto::facades::seal(&pk, secret_val.as_bytes())
                .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

            let envelope = SecretsSyncEnvelope {
                secret_id: spec.id.to_string(),
                sealed_payload: sealed,
                sender_node_id: sender_node_id.clone(),
                timestamp_unix_ms: now_ms,
            };

            let payload =
                serde_json::to_string(&envelope).context("Failed to serialize envelope")?;

            let deliver_req = A2ADeliverRequest {
                sender_agent_id: "0".to_string(),
                receiver_agent_id: "0".to_string(),
                message_type: "secrets_sync".to_string(),
                payload,
                idempotency_key: Some(format!(
                    "secrets_sync:{}:{}:{}",
                    node.node_id, spec.canonical_env, now_ms
                )),
                privacy_class: Some("trusted".to_string()),
                payload_blake3_hex: None,
                worker_ed25519_sig_b64: None,
                jwe_payload: None,
                priority: 255,
                task_kind: Some("secrets_sync".to_string()),
                model_id: None,
                traceparent: None,
            };

            let request_json = serde_json::to_string(&deliver_req)
                .context("Failed to serialize A2ADeliverRequest")?;

            let url_for_task = url_owned.clone();
            let spec_env: String = spec.canonical_env.to_string();
            let dispatch_result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
                let plugin = vox_plugin_host::cached_code_plugin("populi-mesh")
                    .map_err(|e| anyhow::anyhow!("populi-mesh plugin: {e}"))?;
                let driver = plugin
                    .plugin
                    .as_mesh_driver()
                    .into_option()
                    .ok_or_else(|| {
                        anyhow::anyhow!("populi-mesh plugin missing MeshDriver accessor")
                    })?;
                driver
                    .relay_a2a(url_for_task.as_str().into(), request_json.as_str().into())
                    .into_result()
                    .map(|s| s.to_string())
                    .map_err(|e| anyhow::anyhow!("relay_a2a: {e}"))
            })
            .await
            .map_err(|e| anyhow::anyhow!("spawn_blocking join: {e}"))?;

            match dispatch_result {
                Ok(_) => {
                    success_count += 1;
                    info!(
                        target_node = %node.node_id,
                        secret = %spec_env,
                        "Secrets secret sync successful"
                    );
                }
                Err(e) => {
                    error!(
                        target_node = %node.node_id,
                        secret = %spec_env,
                        error = %e,
                        "Secrets secret sync failed"
                    );
                    println!(
                        "    error: failed to deliver {} to {}: {}",
                        spec_env, url_owned, e
                    );
                    fail_count += 1;
                }
            }
        }
    }

    println!(
        "sync complete. {} successes, {} failures.",
        success_count, fail_count
    );
    Ok(())
}
