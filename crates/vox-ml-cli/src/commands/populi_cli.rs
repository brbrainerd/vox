//! `vox populi …` — local registry and HTTP control plane (requires `--features populi`).

use std::net::SocketAddr;
use std::path::PathBuf;

use std::sync::Arc;

use crate::commands::populi_lifecycle::{
    OverlayProviderArg, PopuliConnectivityMode, PopuliLifecycleCmd,
};
use anyhow::Context;
use clap::{Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PopuliAdminSwitch {
    On,
    Off,
}

/// Operator HTTP actions (`POST /v1/populi/admin/*`; mesh or admin bearer via secrets env).
#[derive(Subcommand)]
pub enum PopuliAdminCmd {
    /// Cooperative drain (blocks new exec lease grant/renew and A2A claims for this node id).
    Maintenance {
        /// `NodeRecord.id`
        #[arg(long)]
        node: String,
        #[arg(long, value_enum)]
        state: PopuliAdminSwitch,
        /// Absolute Unix ms deadline when `--state on` (server drops drain after this time). Conflicts with `--for-minutes` (absolute wins).
        #[arg(long)]
        until_unix_ms: Option<u64>,
        /// When `--state on`, set deadline to now + this many minutes on the server (capped by server max).
        #[arg(long)]
        for_minutes: Option<u64>,
    },
    /// Quarantine (hard block on A2A claims for this node id).
    Quarantine {
        /// `NodeRecord.id`
        #[arg(long)]
        node: String,
        #[arg(long, value_enum)]
        state: PopuliAdminSwitch,
    },
    /// Force-remove a remote exec lease row by id (no holder cooperation; mesh/admin bearer).
    ExecLeaseRevoke {
        /// `RemoteExecLeaseGrantResponse.lease_id` from grant (or `GET /v1/populi/exec/leases`).
        #[arg(long)]
        lease_id: String,
    },
}

/// Populi mesh subcommands.
#[derive(Subcommand)]
pub enum PopuliCli {
    /// Initialize a new mesh environment (generate scope ID and mesh tokens).
    Init {
        /// Force overwrite existing env vars in .env or secrets.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Start a private populi network with secure defaults.
    Up {
        /// Connectivity strategy.
        #[arg(long, value_enum, default_value_t = PopuliConnectivityMode::Lan)]
        mode: PopuliConnectivityMode,
        /// Populi scope id (auto-generated when omitted).
        #[arg(long)]
        scope: Option<String>,
        /// GPU advertisement policy (`auto` uses probe defaults).
        #[arg(long, default_value = "auto")]
        gpus: String,
        /// Control-plane bind address.
        #[arg(long, default_value = "127.0.0.1:9847")]
        bind: String,
        /// Overlay provider selection (`auto` probes available providers).
        #[arg(long, value_enum, default_value_t = OverlayProviderArg::Auto)]
        overlay_provider: OverlayProviderArg,
        /// Allow local insecure mode (disables required mesh token).
        #[arg(long, default_value_t = false)]
        insecure_local: bool,
        /// Node visibility for task scheduling (`private`, `public`, `hybrid`).
        #[arg(long, default_value = "private")]
        visibility: String,
        /// Opt-in to processing public mesh tasks when idle.
        #[arg(long, default_value_t = false)]
        public_mesh: bool,
        /// Minimum priority for public mesh tasks (0-255).
        #[arg(long, default_value_t = 128)]
        donate_min_priority: u8,
        /// Task kinds allowed for donation (comma-separated, e.g. "text_infer,image_gen").
        #[arg(long, value_delimiter = ',')]
        donate_kinds: Vec<String>,
        /// Explicit whitelist of user IDs allowed to run tasks.
        #[arg(long, value_delimiter = ',')]
        allow_users: Vec<String>,
        /// Explicit blacklist of user IDs denied from running tasks.
        #[arg(long, value_delimiter = ',')]
        deny_users: Vec<String>,
        /// Explicit whitelist of federated mesh networks (scope IDs) to accept tasks from.
        #[arg(long, value_delimiter = ',')]
        allow_meshes: Vec<String>,
        /// Known peer mesh URLs to gossip federation status with (comma-separated).
        #[arg(long, value_delimiter = ',')]
        bootstrap_peers: Vec<String>,
    },
    /// Stop the populi process started by `vox populi up`.
    Down,
    /// Show populi network status, health, and overlay diagnostics.
    Status {
        /// Emit JSON (also implied by global `--json`).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Print local env and on-disk registry snapshot.
    #[command(name = "registry-snapshot", visible_alias = "local-status")]
    RegistrySnapshot {
        /// Override registry file (default: `VOX_MESH_REGISTRY_PATH` or `~/.vox/cache/mens/local-registry.json`).
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Emit JSON (also implied by global `--json`).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Run the HTTP populi control plane (`GET /v1/populi/nodes`, `POST` join/heartbeat).
    Serve {
        /// Explicitly opt-in to running a mesh control plane (required).
        /// On first run, a bearer token is auto-generated and saved to `~/.vox/config.toml`.
        #[arg(long, default_value_t = false)]
        enable: bool,
        /// Listen address (e.g. `127.0.0.1:9847` or `0.0.0.0:9847`).
        /// Defaults to `127.0.0.1:0` (OS-assigned free port).
        #[arg(long, default_value = "127.0.0.1:0")]
        bind: String,
        /// Seed in-memory state from this registry file on startup (optional).
        #[arg(long)]
        registry: Option<PathBuf>,
        /// Known peer mesh URLs to gossip federation status with (comma-separated).
        #[arg(long, value_delimiter = ',')]
        bootstrap_peers: Vec<String>,
        /// One-time bootstrap token for `vox populi pair` exchanges.
        /// When set, `POST /v1/populi/bootstrap/exchange` accepts this token once
        /// and returns the long-lived mesh token to the caller.
        #[arg(long)]
        bootstrap_token: Option<String>,
    },
    /// Inspect or validate the resolved mesh configuration.
    Config {
        #[command(subcommand)]
        cmd: PopuliConfigCmd,
    },
    /// Maintenance and quarantine toggles on a running control plane.
    Admin {
        /// Control plane base (`VOX_ORCHESTRATOR_MESH_CONTROL_URL`, then `VOX_MESH_CONTROL_ADDR`, when omitted).
        #[arg(long)]
        control_url: Option<String>,
        #[command(subcommand)]
        cmd: PopuliAdminCmd,
    },
    /// Mesh node management (join, leave, list).
    Node {
        #[command(subcommand)]
        cmd: PopuliNodeCmd,
    },
    /// Dispatch a script for remote execution on the mesh.
    Dispatch {
        /// Path to the .vox script or compiled bundle.
        script: PathBuf,
        /// Target node id (optional).
        #[arg(long)]
        node: Option<String>,
        /// Execution timeout in seconds.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
        /// Send as a pre-compiled bundle. If input is a .vox file, it will be bundled first.
        #[arg(long)]
        bundle: bool,
        /// Required capability labels for routing (k=v,...).
        #[arg(long, value_delimiter = ',')]
        routing_labels: Vec<String>,
        /// Return immediately and poll for results later.
        #[arg(long)]
        detach: bool,
        /// Task priority (0-255).
        #[arg(long, default_value_t = 128)]
        priority: u8,
        /// Task kind for filtering.
        #[arg(long)]
        task_kind: Option<String>,
        /// Target model id.
        #[arg(long)]
        model_id: Option<String>,
        /// Minimum VRAM in MB.
        #[arg(long)]
        min_vram: Option<u32>,
    },
    /// Retrieve the result of a detached dispatch by its unique id.
    Result {
        /// Dispatch ID returned from a detached execution.
        dispatch_id: String,
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
    },
    /// Show mesh queue stats (depth, priority, task kinds).
    Stats {
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
        /// Emit JSON (also implied by global `--json`).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Exchange a one-time bootstrap token for the long-lived mesh bearer token.
    ///
    /// The server operator enables bootstrap with `--bootstrap-token <TOKEN>` on `vox populi serve`.
    /// Run this on the client node to obtain and save the mesh token without sharing it out-of-band.
    Pair {
        /// Control plane base URL (e.g. `http://192.168.1.10:9847`).
        #[arg(long)]
        control_url: String,
        /// One-time bootstrap token provided by the server operator.
        #[arg(long)]
        bootstrap_token: String,
    },
    /// Mesh federation queries.
    Federation {
        #[command(subcommand)]
        cmd: PopuliFederationCmd,
    },
    /// Corpus management (extract, filter, stats).
    Corpus {
        #[command(subcommand)]
        cmd: PopuliCorpusCmd,
    },
    /// Manage mesh federation identity (Ed25519 keys).
    Identity {
        #[command(subcommand)]
        cmd: PopuliIdentityCmd,
    },
    /// Publish or fetch a signed public attestation manifest (P6-T2).
    Attest {
        #[command(subcommand)]
        cmd: crate::commands::populi_attest::AttestCmd,
    },
    /// Join the grand volunteer network via an invite URL (P6-T7).
    Join(crate::commands::populi_join::JoinArgs),
}

#[derive(Subcommand)]
pub enum PopuliIdentityCmd {
    /// Show the public Mesh Identity (Public Key).
    Show,
    /// Securely display the private key (base64) for backup.
    Export,
    /// Set the node visibility (public, private, hybrid).
    SetVisibility {
        /// Visibility mode (public, private, hybrid)
        mode: String,
    },
    /// Set whether the orchestrator should prefer routing to the local mesh over cloud APIs.
    PreferMesh {
        /// Preference (true or false)
        #[arg(action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Set the mesh worker donation policy via JSON.
    SetPolicy {
        /// The JSON payload representing the DonationPolicy
        json_payload: String,
    },
    /// Show the current reputation metrics for this node (success/fail/timeout).
    Reputation,
    /// Rotate the identity key pair. A new key pair will be generated and saved, overriding the old one.
    Rotate,
}

#[derive(Subcommand)]
pub enum PopuliConfigCmd {
    /// Print the resolved mesh configuration and the source of each value.
    Show,
    /// Validate the resolved config and report any conflicts or missing required values.
    Check,
}

#[derive(Subcommand)]
pub enum PopuliFederationCmd {
    /// List known federated mesh networks from the control plane directory.
    List {
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
        /// Emit JSON (also implied by global `--json`).
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum PopuliCorpusCmd {
    /// Walk examples/golden/, parse files, and populate extract.jsonl.
    Extract {
        /// Input directory (default: `examples/golden/`).
        #[arg(long, default_value = "examples/golden")]
        input: PathBuf,
        /// Output file (default: `target/dogfood/vox_corpus_extract.jsonl`).
        #[arg(long, default_value = "target/dogfood/vox_corpus_extract.jsonl")]
        output: PathBuf,
        /// Max files to process.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Generate DPO preference pairs (chosen/rejected) from an extracted corpus.
    Dpo {
        /// Input extracted JSONL file (default: `target/dogfood/vox_corpus_extract.jsonl`).
        #[arg(long, default_value = "target/dogfood/vox_corpus_extract.jsonl")]
        input: PathBuf,
        /// Output JSONL file (default: `target/dogfood/preference_pairs.jsonl`).
        #[arg(long, default_value = "target/dogfood/preference_pairs.jsonl")]
        output: PathBuf,
        /// Max pairs to generate.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Convert heal_pairs.jsonl into DPO preference pairs.
    HealToDpo {
        /// Input heal_pairs.jsonl (default: `~/.vox/corpus/heal_pairs.jsonl`).
        #[arg(long)]
        input: Option<PathBuf>,
        /// Output JSONL file (default: `target/dogfood/preference_pairs.jsonl`).
        #[arg(long, default_value = "target/dogfood/preference_pairs.jsonl")]
        output: PathBuf,
    },
    /// Run the fictional research chain generator for the research-expert domain.
    ResearchGen {
        /// Number of chains to generate.
        #[arg(long, default_value_t = 1000)]
        count: usize,
        /// Output file (default: `target/dogfood/research_chains.jsonl`).
        #[arg(long, default_value = "target/dogfood/research_chains.jsonl")]
        output: PathBuf,
    },
    /// Generate Rust-to-Vox translation pairs for cross-domain training.
    RustToVox {
        /// Number of pairs to generate.
        #[arg(long, default_value_t = 1000)]
        count: usize,
        /// Output file (default: `target/dogfood/rust_to_vox.jsonl`).
        #[arg(long, default_value = "target/dogfood/rust_to_vox.jsonl")]
        output: PathBuf,
    },
    /// Produce a frozen benchmark set from the extracted corpus.
    BenchmarkGen {
        /// Input extracted JSONL.
        #[arg(long, default_value = "target/dogfood/vox_corpus_extract.jsonl")]
        input: PathBuf,
        /// Output benchmark file.
        #[arg(long, default_value = "mens/bench/vox-lang-bench-v1.jsonl")]
        output: PathBuf,
        /// Number of samples to reserve.
        #[arg(long, default_value_t = 100)]
        count: usize,
    },
    FlywheelCheck {
        /// Optional domain name (resolves corpus path automatically).
        #[arg(long)]
        domain: Option<String>,
        /// Path to the mixed corpus (defaults to vox-lang if domain omitted).
        #[arg(long)]
        corpus: Option<PathBuf>,
    },
    /// Generate transplant pairs by injecting constructs from one sample into another.
    Transplant {
        /// Input extracted JSONL.
        #[arg(long, default_value = "target/dogfood/vox_corpus_extract.jsonl")]
        input: PathBuf,
        /// Output transplant file.
        #[arg(long, default_value = "target/dogfood/transplant_vox.jsonl")]
        output: PathBuf,
        /// Number of pairs to generate.
        #[arg(long, default_value_t = 500)]
        count: usize,
    },
    /// Apply semantic mutations to an existing corpus to increase diversity.
    Mutate {
        /// Input JSONL.
        #[arg(long, default_value = "target/dogfood/vox_corpus_extract.jsonl")]
        input: PathBuf,
        /// Output mutated JSONL.
        #[arg(long, default_value = "target/dogfood/mutated_vox.jsonl")]
        output: PathBuf,
        /// Number of mutants per input record.
        #[arg(long, default_value_t = 1)]
        factor: usize,
    },
    /// Ingest training logs to extract failure patterns as negative preference pairs.
    IngestLogs {
        /// Path to the error log.
        #[arg(long, default_value = "target/dogfood/train.err.log")]
        log: PathBuf,
        /// Output DPO file.
        #[arg(long, default_value = "target/dogfood/negative_preference_pairs.jsonl")]
        output: PathBuf,
    },
    /// Create a versioned snapshot of all current JSONL data for training reproducibility.
    Snapshot {
        /// Base directory for JSONL source data.
        #[arg(long, default_value = "target/dogfood")]
        src: PathBuf,
        /// Snapshots base directory.
        #[arg(long, default_value = "mens/data/snapshots")]
        dest: PathBuf,
    },
    /// Ingest persistent orchestration lineage to calculate NNT routing efficiency traces.
    IngestWorkflows {
        /// Repository ID to scan.
        #[arg(long, default_value = "vox")]
        repository: String,
        /// Output JSONL file (default: `target/dogfood/workflow_traces.jsonl`).
        #[arg(long, default_value = "target/dogfood/workflow_traces.jsonl")]
        output: PathBuf,
    },
}

/// Mesh node management subcommands.
#[derive(Subcommand)]
pub enum PopuliNodeCmd {
    /// Join the mesh and start a worker listener for dispatch requests.
    Join {
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
        /// Labels to advertise (k=v,...).
        #[arg(long, value_delimiter = ',')]
        labels: Vec<String>,
        /// Address to listen on for incoming dispatch requests (e.g. 0.0.0.0:9848).
        #[arg(long, default_value = "0.0.0.0:9848")]
        bind: String,
    },
    /// Leave the mesh.
    Leave {
        /// Node id (defaults to current process id from env).
        #[arg(long)]
        node: Option<String>,
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
    },
    /// List all nodes in the mesh.
    List {
        /// Control plane base URL.
        #[arg(long)]
        control_url: Option<String>,
    },
}

fn resolve_populi_control_base(url_override: Option<String>) -> anyhow::Result<String> {
    let raw = if let Some(u) = url_override {
        u
    } else if let Some(u) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorMeshControlUrl).expose()
    {
        u.trim().to_string()
    } else {
        anyhow::bail!(
            "set --control-url or VOX_ORCHESTRATOR_MESH_CONTROL_URL or VOX_MESH_CONTROL_ADDR"
        );
    };
    vox_populi::normalize_http_control_base(raw.trim())
        .ok_or_else(|| anyhow::anyhow!("invalid or bind-all control URL: {}", raw.trim()))
}

/// Run a `vox populi` subcommand.
pub async fn run(cmd: PopuliCli, global_json: bool) -> anyhow::Result<()> {
    match cmd {
        PopuliCli::Init { force: _ } => {
            let scope_id = format!("scope-{}", vox_actor_runtime::simple_id::simple_hex_id());
            let mesh_token = vox_actor_runtime::simple_id::simple_hex_id();

            println!("Initializing Populi Mesh Environment...");
            println!("  VOX_MESH_SCOPE_ID={}", scope_id);
            println!("  VOX_MESH_TOKEN={}", mesh_token);

            println!("\nRun the following to apply to your current session:");
            #[cfg(windows)]
            {
                println!("  $env:VOX_MESH_SCOPE_ID=\"{}\"", scope_id);
                println!("  $env:VOX_MESH_TOKEN=\"{}\"", mesh_token);
            }
            #[cfg(not(windows))]
            {
                println!("  export VOX_MESH_SCOPE_ID=\"{}\"", scope_id);
                println!("  export VOX_MESH_TOKEN=\"{}\"", mesh_token);
            }
            Ok(())
        }
        PopuliCli::Identity { cmd } => match cmd {
            PopuliIdentityCmd::Show => {
                let sk_b64 =
                    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshFederationSigningKey)
                        .expose()
                        .map(|s| s.to_string());
                if let Some(s) = sk_b64 {
                    let bytes = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        s.trim(),
                    )
                    .map_err(|e| anyhow::anyhow!("Invalid private key base64: {}", e))?;
                    let sk = vox_crypto::facades::signing_key_from_bytes(
                        &bytes
                            .try_into()
                            .map_err(|_| anyhow::anyhow!("Invalid private key length"))?,
                    );
                    let vk = vox_crypto::facades::to_verifying_key(&sk);
                    let vk_bytes = vox_crypto::facades::verifying_key_to_bytes(&vk);
                    println!("Mesh Federation Identity (Public Key):");
                    println!("  {}", hex::encode(vk_bytes));
                } else {
                    println!(
                        "No Mesh Federation Identity found. Run 'vox populi up' to generate one."
                    );
                }
                Ok(())
            }
            PopuliIdentityCmd::Export => {
                let sk_b64 =
                    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshFederationSigningKey)
                        .expose()
                        .map(|s| s.to_string());
                if let Some(s) = sk_b64 {
                    println!("!!! SECURE BACKUP - DO NOT SHARE THIS KEY !!!");
                    println!("Mesh Federation Private Key (base64):");
                    println!("  {}", s.trim());
                } else {
                    println!("No Mesh Federation Identity found.");
                }
                Ok(())
            }
            PopuliIdentityCmd::SetVisibility { mode } => {
                let valid_modes = ["public", "private", "hybrid"];
                if !valid_modes.contains(&mode.as_str()) {
                    anyhow::bail!(
                        "Invalid visibility mode: {}. Must be one of {:?}",
                        mode,
                        valid_modes
                    );
                }
                let auth_path = vox_secrets::set_registry_token("mesh_visibility", &mode, None)?;
                println!("Updated mesh visibility to '{}'", mode);
                println!("Wrote to secrets auth store at: {}", auth_path.display());
                Ok(())
            }
            PopuliIdentityCmd::PreferMesh { enabled } => {
                let val = if enabled { "true" } else { "false" };
                let auth_path = vox_secrets::set_registry_token("routing_prefer_mesh", val, None)?;
                println!("Updated mesh routing preference to '{}'", val);
                println!("Wrote to secrets auth store at: {}", auth_path.display());
                Ok(())
            }
            PopuliIdentityCmd::SetPolicy { json_payload } => {
                // Try parsing it first to validate
                let _parsed: vox_mesh_types::WorkerDonationPolicy =
                    serde_json::from_str(&json_payload)
                        .map_err(|e| anyhow::anyhow!("Invalid WorkerDonationPolicy JSON: {}", e))?;

                let auth_path =
                    vox_secrets::set_registry_token("mesh_donation_policy", &json_payload, None)?;
                println!("Updated mesh donation policy (valid JSON).");
                println!("Wrote to secrets auth store at: {}", auth_path.display());
                Ok(())
            }
            PopuliIdentityCmd::Reputation => {
                let db = vox_db::VoxDb::open_default().await?;
                let env = vox_populi::populi_env_resolved(None);
                if let Some(node_id) = env.node_id {
                    if let Some((s, f, t, i)) = db.get_peer_reputation(&node_id).await? {
                        println!("Local Node Reputation ({}):", node_id);
                        println!("  Successes:        {}", s);
                        println!("  Failures:         {}", f);
                        println!("  Timeouts:         {}", t);
                        println!("  Invalid Outputs:  {}", i);

                        let total_bad = f + t + i;
                        if total_bad > 3 && total_bad > s {
                            println!("  Status:           BLACKLISTED (too many failures)");
                        } else {
                            println!("  Status:           HEALTHY");
                        }
                    } else {
                        println!("No reputation data found for node '{}'", node_id);
                    }
                } else {
                    println!("No local node_id found. Run 'vox populi up' first.");
                }
                Ok(())
            }
            PopuliIdentityCmd::Rotate => {
                let db = vox_db::VoxDb::open_default().await?;
                let env = vox_populi::populi_env_resolved(None);

                // Generate new key pair
                let (new_sk, _) = vox_crypto::facades::generate_signing_keypair();
                let new_sk_bytes = vox_crypto::facades::signing_key_to_bytes(&new_sk);
                let new_sk_b64 = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    new_sk_bytes,
                );

                let new_vk = vox_crypto::facades::to_verifying_key(&new_sk);
                let new_vk_bytes = vox_crypto::facades::verifying_key_to_bytes(&new_vk);
                let new_node_id = hex::encode(&vox_crypto::secure_hash(&new_vk_bytes)[0..16]);

                if let Some(old_node_id) = env.node_id {
                    // Migrate reputation
                    db.migrate_peer_reputation(&old_node_id, &new_node_id)
                        .await?;
                    println!(
                        "Migrated reputation from node '{}' to '{}'",
                        old_node_id, new_node_id
                    );
                }

                let auth_path = vox_secrets::set_registry_token(
                    "mesh_federation_signing_key",
                    &new_sk_b64,
                    None,
                )?;
                println!("Rotated Mesh Federation Identity.");
                println!("New Public Key:");
                println!("  {}", hex::encode(new_vk_bytes));
                println!("Wrote to secrets auth store at: {}", auth_path.display());
                Ok(())
            }
        },
        PopuliCli::Pair {
            control_url,
            bootstrap_token,
        } => {
            let base = vox_populi::normalize_http_control_base(control_url.trim())
                .ok_or_else(|| anyhow::anyhow!("invalid control URL: {}", control_url))?;
            let client = vox_populi::http_client::PopuliHttpClient::new(&base);
            let resp = client
                .bootstrap_exchange(&bootstrap_token)
                .await
                .map_err(|e| anyhow::anyhow!("bootstrap exchange failed: {}", e))?;

            const MESH_TOKEN_KEY: &str = "mesh.token";
            const MESH_SCOPE_KEY: &str = "mesh.scope_id";

            if let Err(e) =
                vox_config::toml_config::set_user_config_value(MESH_TOKEN_KEY, &resp.mesh_token)
            {
                anyhow::bail!("failed to save mesh token: {}", e);
            }
            println!("vox populi pair: mesh token saved to ~/.vox/config.toml");
            println!(
                "  Set VOX_MESH_TOKEN={} in your environment to use it now.",
                resp.mesh_token
            );

            if let Some(scope) = &resp.scope_id {
                if !scope.is_empty() {
                    if let Err(e) =
                        vox_config::toml_config::set_user_config_value(MESH_SCOPE_KEY, scope)
                    {
                        tracing::warn!(error = %e, "failed to save mesh scope_id");
                    } else {
                        println!("  Scope ID: {}", scope);
                    }
                }
            }
            Ok(())
        }
        PopuliCli::Federation { cmd } => match cmd {
            PopuliFederationCmd::List { control_url, json } => {
                let base = resolve_populi_control_base(control_url)?;
                let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
                let dir = client.federation_directory().await.map_err(|e| {
                    anyhow::anyhow!("Failed to fetch federation directory from {}: {}", base, e)
                })?;

                if json || global_json {
                    println!("{}", serde_json::to_string_pretty(&dir)?);
                } else {
                    println!("Mesh Federation Directory");
                    println!("  Control Plane: {}", base);
                    println!("  Known Peers:   {}", dir.entries.len());
                    println!();
                    if dir.entries.is_empty() {
                        println!("  (No federated peers registered)");
                    } else {
                        for peer in dir.entries {
                            let pub_str = if peer.public { "Public" } else { "Private" };
                            let q_depth = peer
                                .current_queue_depth
                                .map_or("?".to_string(), |v| v.to_string());
                            let region = peer.region_label.as_deref().unwrap_or("unknown");
                            println!("  - [{}] {} ({})", pub_str, peer.scope_id, region);
                            println!("      URL:   {}", peer.control_url);
                            println!("      Queue: {}", q_depth);
                            let kinds: Vec<_> =
                                peer.task_kinds.iter().map(|k| k.to_string()).collect();
                            println!("      Kinds: {}", kinds.join(", "));
                        }
                    }
                }
                Ok(())
            }
        },
        PopuliCli::Stats { control_url, json } => {
            let base = resolve_populi_control_base(control_url)?;
            let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
            let stats = client
                .queue_stats()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to fetch mesh stats from {}: {}", base, e))?;

            if json || global_json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("Mesh Queue Statistics");
                println!("  Control Plane: {}", base);
                println!("  Pending Tasks: {}", stats.pending_count);
                println!();
                if !stats.pending_by_kind.is_empty() {
                    println!("By Task Kind:");
                    let mut kinds: Vec<_> = stats.pending_by_kind.iter().collect();
                    kinds.sort_by_key(|(k, _)| *k);
                    for (kind, count) in kinds {
                        println!("  {: <15} : {}", kind, count);
                    }
                    println!();
                }
                if !stats.pending_by_priority.is_empty() {
                    println!("By Priority:");
                    let mut prios: Vec<_> = stats.pending_by_priority.iter().collect();
                    prios.sort_by_key(|(p, _)| *p);
                    for (prio, count) in prios {
                        println!("  Priority {: >3}     : {}", prio, count);
                    }
                }
            }
            Ok(())
        }
        PopuliCli::Up {
            mode,
            scope,
            gpus,
            bind,
            overlay_provider,
            insecure_local,
            visibility,
            public_mesh,
            donate_min_priority,
            donate_kinds,
            allow_users,
            deny_users,
            allow_meshes,
            bootstrap_peers,
        } => {
            crate::commands::populi_lifecycle::run(
                PopuliLifecycleCmd::Up {
                    mode,
                    scope,
                    gpus,
                    bind,
                    overlay_provider,
                    insecure_local,
                    visibility,
                    public_mesh,
                    donate_min_priority,
                    donate_kinds,
                    allow_users,
                    deny_users,
                    allow_meshes,
                    bootstrap_peers,
                },
                global_json,
            )
            .await
        }
        PopuliCli::Down => {
            crate::commands::populi_lifecycle::run(PopuliLifecycleCmd::Down, global_json).await
        }
        PopuliCli::Status { json } => {
            crate::commands::populi_lifecycle::run(PopuliLifecycleCmd::Status { json }, global_json)
                .await
        }
        PopuliCli::RegistrySnapshot { registry, json } => {
            let path = registry.unwrap_or_else(vox_populi::local_registry_path);
            let reg = vox_populi::LocalRegistry::new(path.clone());
            let file = reg.load()?;
            let env = vox_populi::populi_env();
            let self_id = env.node_id.clone().unwrap_or_else(|| {
                format!("local-{}", vox_actor_runtime::simple_id::simple_hex_id())
            });
            let self_record =
                vox_populi::node_record_for_current_process(self_id, env.control_addr.clone());
            let as_json = json || global_json;
            if as_json {
                let v = serde_json::json!({
                    "populi_env": env,
                    "registry_path": reg.path().display().to_string(),
                    "registry": file,
                    "self_record": self_record,
                });
                println!("{}", serde_json::to_string_pretty(&v)?);
            } else {
                println!("Mesh env:");
                println!("  VOX_MESH_ENABLED: {}", env.enabled);
                if let Some(ref id) = env.node_id {
                    println!("  VOX_MESH_NODE_ID: {id}");
                }
                if !env.labels.is_empty() {
                    println!("  VOX_MESH_LABELS: {}", env.labels.join(","));
                }
                if let Some(ref a) = env.control_addr {
                    println!("  VOX_MESH_CONTROL_ADDR: {a}");
                }
                if let Some(ref p) = env.registry_path {
                    println!("  VOX_MESH_REGISTRY_PATH: {p}");
                }
                if let Some(ref s) = env.scope_id {
                    println!("  VOX_MESH_SCOPE_ID: {s}");
                }
                println!();
                println!("Registry file: {}", reg.path().display());
                println!("  nodes: {}", file.nodes.len());
                for n in &file.nodes {
                    println!(
                        "    - {} @ {} ms (caps cpu_cores={:?})",
                        n.id, n.last_seen_unix_ms, n.capabilities.cpu_cores
                    );
                }
                println!();
                println!("This process (probe) node id: {}", self_record.id);
            }
            Ok(())
        }
        PopuliCli::Serve {
            enable,
            bind,
            registry,
            bootstrap_peers,
            bootstrap_token,
        } => {
            if !enable {
                anyhow::bail!(
                    "Pass `--enable` to start the mesh control plane.\n\
                     On first run a bearer token is auto-generated and saved to ~/.vox/config.toml.\n\
                     See `vox populi config show` to view the resolved configuration.\n\
                     See docs/src/how-to/populi-quickstart.md for a step-by-step guide."
                );
            }

            // Token resolution: secrets → config (`mesh.token`) → auto-generate (then inject env).
            const MESH_TOKEN_KEY: &str = "mesh.token";
            let cfg_mesh = vox_config::toml_config::load_user_config();
            let saved_mesh = cfg_mesh
                .values
                .get(MESH_TOKEN_KEY)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resolved_mesh = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken)
                .expose()
                .map(|s| s.to_string())
                .or(saved_mesh);

            if resolved_mesh.is_none() {
                let raw = uuid::Uuid::new_v4().simple().to_string()
                    + &uuid::Uuid::new_v4().simple().to_string();
                let token = raw[..48].to_string(); // 48 hex chars = 192 bits
                if let Err(e) =
                    vox_config::toml_config::set_user_config_value(MESH_TOKEN_KEY, &token)
                {
                    tracing::warn!(error = %e, "failed to persist mesh.token to config");
                }
                println!("vox populi: generated mesh bearer token (saved to ~/.vox/config.toml):");
                println!("  VOX_MESH_TOKEN={token}");
                println!("  Keep this secret — it authenticates all control-plane requests.");
                #[allow(unsafe_code)]
                unsafe {
                    std::env::set_var("VOX_MESH_TOKEN", &token);
                }
            }

            let addr: SocketAddr = bind
                .parse()
                .with_context(|| format!("invalid --bind address: {bind}"))?;
            let mut state = if let Some(p) = registry {
                vox_populi::transport::PopuliTransportState::load_from_path(&p)
                    .await
                    .with_context(|| format!("load registry {}", p.display()))?
            } else {
                vox_populi::transport::PopuliTransportState::new_for_serve()
            };

            if !bootstrap_peers.is_empty() {
                state.bootstrap_peers = bootstrap_peers;
            } else if let Ok(peers) = std::env::var("VOX_MESH_FEDERATION_BOOTSTRAP_PEERS") {
                state.bootstrap_peers = peers.split(',').map(|s| s.to_string()).collect();
            }

            if let Some(token) = bootstrap_token {
                state = state.with_bootstrap_token(token);
            }

            // Optional: DB-backed mesh store + trust verifier + reputation decay
            if let Ok(db) = vox_db::VoxDb::connect_canonical().await {
                // Durable mesh store (write-through; warms in-memory caches from DB)
                let mesh_db = db.clone();
                state = state.with_mesh_store(Arc::new(
                    vox_populi::transport::store::VoxDbMeshStore::new(mesh_db),
                ));
                if let Err(e) = state.init_from_mesh_store().await {
                    tracing::warn!(error = %e, "mesh store warm-up failed; continuing with empty cache");
                }

                if let Some(self_id) = vox_populi::populi_env().node_id {
                    let db_for_verifier = Arc::new(db);
                    let db_for_decay = Arc::clone(&db_for_verifier);
                    let grantor_verifier = self_id.clone();
                    let grantor_decay = self_id.clone();

                    state.node_trust_verifier = Some(Arc::new(move |trusted_id| {
                        let db = Arc::clone(&db_for_verifier);
                        let grantor = grantor_verifier.clone();
                        Box::pin(async move {
                            db.is_node_trusted(&grantor, &trusted_id)
                                .await
                                .unwrap_or(false)
                        })
                    }));

                    // Spawn reputation decay worker
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(vox_config::timeouts::D_3600S); // Every hour
                        loop {
                            interval.tick().await;
                            // Threshold 10 severity sum within 24h
                            if let Ok(affected) = db_for_decay
                                .process_reputation_decay(&grantor_decay, 10)
                                .await
                            {
                                if affected > 0 {
                                    tracing::warn!(
                                        "Reputation decay: removed {} trust grants",
                                        affected
                                    );
                                }
                            }
                        }
                    });
                }
            }

            vox_populi::transport::serve(addr, state)
                .await
                .with_context(|| format!("populi HTTP serve on {addr}"))?;
            Ok(())
        }
        PopuliCli::Config { cmd } => {
            const MESH_TOKEN_KEY: &str = "mesh.token";
            let cfg = vox_config::toml_config::load_user_config();
            match cmd {
                PopuliConfigCmd::Show => {
                    println!("Resolved mesh configuration:");
                    println!();

                    // Bind address
                    println!("  bind           : 127.0.0.1:0 (default; override with --bind)");

                    // Token source
                    let token_source =
                        if vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken)
                            .expose()
                            .is_some()
                        {
                            "secrets-resolved mesh token (env / vault)"
                        } else if cfg.values.contains_key(MESH_TOKEN_KEY) {
                            "file: ~/.vox/config.toml (mesh.token)"
                        } else {
                            "unset (will be auto-generated on first `vox populi serve --enable`)"
                        };
                    println!("  mesh.token     : {token_source}");

                    // Bootstrap peers
                    let peers_source = if std::env::var("VOX_MESH_FEDERATION_BOOTSTRAP_PEERS")
                        .map(|v| !v.is_empty())
                        .unwrap_or(false)
                    {
                        "env: VOX_MESH_FEDERATION_BOOTSTRAP_PEERS"
                    } else {
                        "unset"
                    };
                    println!("  bootstrap_peers: {peers_source}");

                    // Config file path
                    if let Some(dir) = vox_config::dot_vox_user_dir().to_str() {
                        println!();
                        println!("  Config file: {dir}/config.toml");
                    }
                }
                PopuliConfigCmd::Check => {
                    let ok = true;
                    println!("Checking mesh configuration...");

                    let has_token =
                        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken)
                            .expose()
                            .is_some()
                            || cfg.values.contains_key(MESH_TOKEN_KEY);
                    if !has_token {
                        println!(
                            "  WARN  mesh.token not set — a token will be auto-generated on first serve"
                        );
                    } else {
                        println!("  OK    mesh.token is set");
                    }

                    // Check that the config file is writable
                    let config_path = vox_config::dot_vox_user_dir().join("config.toml");
                    let parent = config_path.parent().unwrap_or(&config_path);
                    if !parent.exists() {
                        println!(
                            "  WARN  config dir does not yet exist: {}",
                            parent.display()
                        );
                    } else {
                        println!("  OK    config dir exists: {}", parent.display());
                    }

                    if ok {
                        println!();
                        println!("Configuration OK — ready to run `vox populi serve --enable`");
                    } else {
                        println!();
                        println!("Configuration has issues — see above");
                    }
                    let _ = ok; // suppress unused warning; kept for future hard-failure checks
                }
            }
            Ok(())
        }
        PopuliCli::Admin { control_url, cmd } => {
            let base = resolve_populi_control_base(control_url)?;
            let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
            match cmd {
                PopuliAdminCmd::Maintenance {
                    node,
                    state,
                    until_unix_ms,
                    for_minutes,
                } => {
                    let node_id = node.trim().to_string();
                    if node_id.is_empty() {
                        anyhow::bail!("--node must be non-empty");
                    }
                    let on = matches!(state, PopuliAdminSwitch::On);
                    if !on && (until_unix_ms.is_some() || for_minutes.is_some()) {
                        anyhow::bail!("--until-unix-ms / --for-minutes require --state on");
                    }
                    if until_unix_ms.is_some() && for_minutes.is_some() {
                        anyhow::bail!("use only one of --until-unix-ms or --for-minutes");
                    }
                    let maintenance_for_ms = for_minutes.map(|m| m.saturating_mul(60_000));
                    client
                        .admin_maintenance(&vox_populi::transport::AdminMaintenanceRequest {
                            node_id,
                            maintenance: on,
                            maintenance_until_unix_ms: until_unix_ms,
                            maintenance_for_ms,
                        })
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                }
                PopuliAdminCmd::Quarantine { node, state } => {
                    let node_id = node.trim().to_string();
                    if node_id.is_empty() {
                        anyhow::bail!("--node must be non-empty");
                    }
                    client
                        .admin_quarantine(&vox_populi::transport::AdminQuarantineRequest {
                            node_id,
                            quarantined: matches!(state, PopuliAdminSwitch::On),
                        })
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                }
                PopuliAdminCmd::ExecLeaseRevoke { lease_id } => {
                    let lease_id = lease_id.trim().to_string();
                    if lease_id.is_empty() {
                        anyhow::bail!("--lease-id must be non-empty");
                    }
                    client
                        .admin_exec_lease_revoke(
                            &vox_populi::transport::AdminExecLeaseRevokeRequest { lease_id },
                        )
                        .await
                        .map_err(|e| anyhow::anyhow!(e))?;
                }
            }
            if global_json {
                println!("{}", serde_json::json!({ "ok": true }));
            } else {
                println!("ok");
            }
            Ok(())
        }
        PopuliCli::Node { cmd } => match cmd {
            PopuliNodeCmd::Join {
                control_url,
                labels,
                bind,
            } => {
                let base = resolve_populi_control_base(control_url)?;
                let addr: SocketAddr = bind
                    .parse()
                    .with_context(|| format!("invalid --bind address: {bind}"))?;

                // Build record
                let env = vox_populi::populi_env();
                let id = env.node_id.clone().unwrap_or_else(|| {
                    format!("node-{}", vox_foundation::primitives::id::simple_hex_id())
                });

                let mut record =
                    vox_populi::node_record_for_current_process(id, Some(bind.clone()));
                for lab in labels {
                    if !record.capabilities.labels.contains(&lab) {
                        record.capabilities.labels.push(lab);
                    }
                }

                // Join
                let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
                let updated = client
                    .join(&record)
                    .await
                    .map_err(|e| anyhow::anyhow!("Mesh join failed: {}", e))?;

                println!("Joined mesh at {} as node {}", base, updated.id);

                // Start heartbeat loop and worker listener
                let mut state = vox_populi::transport::PopuliTransportState::new_for_serve();

                // Optional: DB-backed trust verifier and reputation decay (hardens mesh from Sybil/poisoning)
                if let Ok(db) = vox_db::VoxDb::connect_canonical().await {
                    let db_for_verifier = Arc::new(db);
                    let db_for_decay = Arc::clone(&db_for_verifier);
                    let grantor_verifier = updated.id.clone();
                    let grantor_decay = updated.id.clone();

                    state.node_trust_verifier = Some(Arc::new(move |trusted_id| {
                        let db = Arc::clone(&db_for_verifier);
                        let grantor = grantor_verifier.clone();
                        Box::pin(async move {
                            db.is_node_trusted(&grantor, &trusted_id)
                                .await
                                .unwrap_or(false)
                        })
                    }));

                    // Spawn reputation decay worker
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(vox_config::timeouts::D_3600S); // Every hour
                        loop {
                            interval.tick().await;
                            // Threshold 10 severity sum within 24h
                            if let Ok(affected) = db_for_decay
                                .process_reputation_decay(&grantor_decay, 10)
                                .await
                            {
                                if affected > 0 {
                                    tracing::warn!(
                                        "Reputation decay: removed {} trust grants",
                                        affected
                                    );
                                }
                            }
                        }
                    });
                }

                // Spawn heartbeat
                let base_cl = base.clone();
                let timeout = vox_populi::http_lifecycle::populi_http_timeout_ms_from_env();
                let interval =
                    vox_populi::http_lifecycle::populi_heartbeat_interval_secs_from_env();

                if interval > 0 {
                    tokio::spawn(async move {
                        use sysinfo::System;
                        let mut sys = System::new_all();

                        let mut tick =
                            tokio::time::interval(std::time::Duration::from_secs(interval));
                        let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                            &base_cl,
                            std::time::Duration::from_millis(timeout),
                        )
                        .with_env_token();
                        let mut current_record = updated.clone();

                        loop {
                            tick.tick().await;
                            sys.refresh_cpu_all();
                            sys.refresh_memory();

                            current_record.cpu_usage_pct = Some(sys.global_cpu_usage());
                            current_record.memory_free_bytes = Some(sys.available_memory());

                            let _ = client.heartbeat(&current_record).await;
                        }
                    });
                }

                println!("Worker listening for dispatch on {}", addr);
                vox_populi::transport::serve(addr, state)
                    .await
                    .with_context(|| format!("populi worker serve on {addr}"))?;

                Ok(())
            }
            PopuliNodeCmd::Leave { node, control_url } => {
                let base = resolve_populi_control_base(control_url)?;
                let env = vox_populi::populi_env();
                let id = node.or(env.node_id).ok_or_else(|| {
                    anyhow::anyhow!("node id required (set --node or VOX_MESH_NODE_ID)")
                })?;

                let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
                let found = client
                    .leave(&id)
                    .await
                    .map_err(|e| anyhow::anyhow!("Mesh leave failed: {}", e))?;

                if found {
                    println!("Node {} left mesh {}", id, base);
                } else {
                    println!("Node {} was not found in mesh {}", id, base);
                }
                Ok(())
            }
            PopuliNodeCmd::List { control_url } => {
                let base = resolve_populi_control_base(control_url)?;
                let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
                let reg = client
                    .list_nodes()
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to list nodes: {}", e))?;

                if global_json {
                    println!("{}", serde_json::to_string_pretty(&reg)?);
                } else {
                    println!("Mesh nodes at {}:", base);
                    for n in reg.nodes {
                        println!(
                            "  - {} (version: {}, triple: {}, caps: {:?})",
                            n.id,
                            n.version,
                            n.host_triple.as_deref().unwrap_or("unknown"),
                            n.capabilities.labels
                        );
                    }
                }
                Ok(())
            }
        },
        PopuliCli::Dispatch {
            script,
            node,
            timeout,
            control_url,
            bundle,
            routing_labels,
            detach,
            priority,
            task_kind,
            model_id,
            min_vram,
        } => {
            let base = resolve_populi_control_base(control_url)?;
            let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();

            // Phase 3: Autonomous Target Resolution
            let mut target_triple = None;
            if let Some(node_id) = &node {
                // Query node metadata to find target triple
                if let Ok(reg) = client.list_nodes().await {
                    if let Some(n) = reg.nodes.iter().find(|n| n.id == *node_id) {
                        target_triple = n.host_triple.clone();
                    }
                }
            }

            let (b64_source, is_bundle, source_blake3_hex) =
                if bundle && script.extension().map_or(false, |ext| ext == "vox") {
                    // Auto-bundle source to a temp dir
                    let tmp_bundle_dir = std::env::temp_dir().join("vox-bundle-dispatch");
                    let _ = std::fs::remove_dir_all(&tmp_bundle_dir);

                    // Use resolved triple if found, otherwise let it be None (host native)
                    // Use vox binary via subprocess to break library dependency cycle
                    let mut cmd = tokio::process::Command::new("vox");
                    cmd.arg("bundle")
                        .arg(&script)
                        .arg("--out-dir")
                        .arg(&tmp_bundle_dir)
                        .arg("--mode")
                        .arg("script")
                        .arg("--release");
                    if let Some(t) = target_triple.as_deref() {
                        cmd.arg("--target").arg(t);
                    }
                    let status = cmd.status().await?;
                    if !status.success() {
                        anyhow::bail!("vox bundle via subprocess failed");
                    }

                    // Find the binary in the temp dir
                    let mut entries = std::fs::read_dir(&tmp_bundle_dir)?;
                    let mut bin_path = None;
                    while let Some(Ok(entry)) = entries.next() {
                        if entry.path().is_file() {
                            bin_path = Some(entry.path());
                            break;
                        }
                    }

                    let bin_path = bin_path
                        .ok_or_else(|| anyhow::anyhow!("Bundling failed to produce a binary"))?;
                    let bin_bytes = std::fs::read(&bin_path)?;
                    let hash_hex = blake3::hash(&bin_bytes).to_hex().to_string();
                    use base64::Engine as _;
                    (
                        base64::engine::general_purpose::STANDARD.encode(bin_bytes),
                        true,
                        Some(hash_hex),
                    )
                } else if bundle {
                    // Input is already a bundle binary
                    let bin_bytes = std::fs::read(&script)?;
                    let hash_hex = blake3::hash(&bin_bytes).to_hex().to_string();
                    use base64::Engine as _;
                    (
                        base64::engine::general_purpose::STANDARD.encode(bin_bytes),
                        true,
                        Some(hash_hex),
                    )
                } else {
                    // Send as source
                    let source = std::fs::read_to_string(&script)
                        .with_context(|| format!("failed to read script {}", script.display()))?;
                    let hash_hex = blake3::hash(source.as_bytes()).to_hex().to_string();
                    use base64::Engine as _;
                    (
                        base64::engine::general_purpose::STANDARD.encode(&source),
                        false,
                        Some(hash_hex),
                    )
                };

            let req = vox_populi::transport::DispatchRequest {
                source: b64_source,
                node_id: node,
                timeout_secs: timeout,
                is_bundle,
                source_blake3_hex,
                required_labels: if routing_labels.is_empty() {
                    None
                } else {
                    Some(routing_labels)
                },
                is_detached: detach,
                priority,
                task_kind,
                model_id,
                min_vram_mb: min_vram,
            };

            let resp = client
                .dispatch(&req)
                .await
                .map_err(|e| anyhow::anyhow!("Dispatch failed: {e}"))?;

            if global_json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                if resp.success {
                    println!("✓ Dispatch Success");
                    println!("  Node:     {}", resp.node_id);
                    println!("  Duration: {:.2}s", resp.duration_ms as f64 / 1000.0);
                    if let Some(code) = resp.exit_code {
                        println!("  ExitCode: {}", code);
                    }
                    if resp.is_truncated {
                        println!("  Warning:  Output was truncated due to length limits (10MB).");
                    }
                    println!("\nOutput:\n{}", resp.output);
                } else {
                    eprintln!("✗ Dispatch Failure");
                    eprintln!("  Node:     {}", resp.node_id);
                    eprintln!("  Duration: {:.2}s", resp.duration_ms as f64 / 1000.0);
                    if let Some(code) = resp.exit_code {
                        eprintln!("  ExitCode: {}", code);
                    }
                    if let Some(err) = resp.error {
                        eprintln!("  Error:    {}", err);
                    }
                    if resp.is_truncated {
                        eprintln!("  Warning:  Output was truncated due to length limits (10MB).");
                    }
                    eprintln!("\nOutput:\n{}", resp.output);
                    anyhow::bail!("Remote execution failed");
                }
            }
            Ok(())
        }
        PopuliCli::Result {
            dispatch_id,
            control_url,
        } => {
            let base = resolve_populi_control_base(control_url)?;
            let client = vox_populi::http_client::PopuliHttpClient::new(&base).with_env_token();
            let resp = client
                .dispatch_result_poll(&dispatch_id)
                .await
                .map_err(|e| anyhow::anyhow!("Result poll failed: {e}"))?;

            if global_json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                if resp.success {
                    println!("✓ Result Fetched for Dispatch {}", dispatch_id);
                    println!("  Node:     {}", resp.node_id);
                    println!("  Duration: {:.2}s", resp.duration_ms as f64 / 1000.0);
                    if let Some(code) = resp.exit_code {
                        println!("  ExitCode: {}", code);
                    }
                    println!("\nOutput:\n{}", resp.output);
                } else {
                    eprintln!("✗ Result Error for Dispatch {}", dispatch_id);
                    if let Some(err) = resp.error {
                        eprintln!("\nError:\n{}", err);
                    }
                }
            }
            Ok(())
        }
        PopuliCli::Corpus { cmd } => match cmd {
            PopuliCorpusCmd::Extract {
                input,
                output,
                limit,
            } => {
                println!("Extracting Vox corpus from {} ...", input.display());
                let config = vox_corpus::corpus::extract_vox::ExtractVoxConfig {
                    root: input.clone(),
                    limit: limit.unwrap_or(0),
                    ..Default::default()
                };
                let pairs = vox_corpus::corpus::extract_vox::walk_and_extract_vox(&config)?;
                let count = vox_corpus::corpus::extract_vox::write_vox_to_jsonl(&pairs, &output)?;
                println!("✓ Extracted {} pairs to {}", count, output.display());
                Ok(())
            }
            PopuliCorpusCmd::Dpo {
                input,
                output,
                limit,
            } => {
                println!("Generating DPO pairs from {} ...", input.display());
                let config = vox_corpus::corpus::dpo::DpoConfig {
                    input,
                    output: output.clone(),
                    limit: limit.unwrap_or(0),
                };
                let count = vox_corpus::corpus::dpo::generate_dpo_from_extract(&config)?;
                println!("✓ Generated {} DPO pairs to {}", count, output.display());
                Ok(())
            }
            PopuliCorpusCmd::HealToDpo { input, output } => {
                // Delegate to existing corpus command logic (needs features: mens-base)
                #[cfg(feature = "mens-base")]
                {
                    crate::commands::corpus::generate::run_heal_to_dpo(input, &output).await?;
                    Ok(())
                }
                #[cfg(not(feature = "mens-base"))]
                {
                    anyhow::bail!("HealToDpo requires `mens-base` feature")
                }
            }
            PopuliCorpusCmd::ResearchGen { count, output } => {
                println!(
                    "Generating {} research chains to {} ...",
                    count,
                    output.display()
                );
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&output)?;
                let actual = vox_corpus::research_gen::generate_research_chains(&mut f, count)?;
                println!("✓ Generated {} chains", actual);
                Ok(())
            }
            PopuliCorpusCmd::RustToVox { count, output } => {
                println!(
                    "Generating {} Rust-to-Vox translation pairs to {} ...",
                    count,
                    output.display()
                );
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&output)?;
                let actual = vox_corpus::rust_to_vox::generate_rust_to_vox_pairs(&mut f, count)?;
                println!("✓ Generated {} pairs", actual);
                Ok(())
            }
            PopuliCorpusCmd::BenchmarkGen {
                input,
                output,
                count,
            } => {
                println!(
                    "Producing frozen benchmark from {} to {} ...",
                    input.display(),
                    output.display()
                );
                let actual = vox_corpus::corpus::produce_benchmark(&input, &output, count)?;
                println!("✓ Produced {} benchmark samples", actual);
                Ok(())
            }
            PopuliCorpusCmd::FlywheelCheck { domain, corpus } => {
                let resolved_corpus = if let Some(c) = corpus {
                    c
                } else if let Some(d) = &domain {
                    PathBuf::from(format!(
                        "mens/data/train_mixed_{}.jsonl",
                        d.replace("-", "_")
                    ))
                } else {
                    PathBuf::from("mens/data/train_mixed_vox_lang.jsonl")
                };

                println!(
                    "Checking flywheel readiness for {} (Domain: {}) ...",
                    resolved_corpus.display(),
                    domain.as_deref().unwrap_or("vox-lang")
                );
                let result =
                    vox_corpus::flywheel::evaluate_readiness(&resolved_corpus, domain.as_deref())?;
                match result {
                    vox_corpus::flywheel::FlywheelSignal::Ready { ast_diversity } => {
                        println!("🚀 FLYWHEEL READY (Diversity: {:.2})", ast_diversity);

                        #[cfg(feature = "extras-ludus")]
                        {
                            vox_cli_core::gamify_shim::record_cli_event_fire_and_forget(
                                "mens_flywheel_triggered",
                                true,
                                Some("mens-corpus"),
                                Some("populi corpus flywheel-check"),
                            );
                        }
                    }
                    vox_corpus::flywheel::FlywheelSignal::Pending { new_samples } => {
                        println!("⏳ PENDING (Samples: {})", new_samples);
                    }
                    vox_corpus::flywheel::FlywheelSignal::Triggered => {
                        println!("⚡ FLYWHEEL TRIGGERED");
                    }
                    vox_corpus::flywheel::FlywheelSignal::Idle => {
                        println!("⚪ IDLE");
                    }
                }
                Ok(())
            }
            PopuliCorpusCmd::Transplant {
                input,
                output,
                count,
            } => {
                println!(
                    "Generating {} transplant pairs from {} to {} ...",
                    count,
                    input.display(),
                    output.display()
                );
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&output)?;
                let actual =
                    vox_corpus::synthetic_gen::transplant_pairs::generate_transplant_pairs(
                        &input, &mut f, count,
                    )?;
                println!("✓ Generated {} transplant pairs", actual);
                Ok(())
            }
            PopuliCorpusCmd::Mutate {
                input,
                output,
                factor,
            } => {
                println!(
                    "Mutating {} with factor {} to {} ...",
                    input.display(),
                    factor,
                    output.display()
                );
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&output)?;
                let actual = vox_corpus::ast_mutator::mutate_corpus(&input, &mut f, factor)?;
                println!("✓ Generated {} mutated pairs", actual);
                Ok(())
            }
            PopuliCorpusCmd::IngestLogs { log, output } => {
                println!(
                    "Ingesting logs from {} to {} ...",
                    log.display(),
                    output.display()
                );
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&output)?;
                let actual = vox_corpus::corpus::ingest_training_logs(&log, &mut f)?;
                println!("✓ Ingested {} failure patterns", actual);
                Ok(())
            }
            PopuliCorpusCmd::Snapshot { src, dest } => {
                println!(
                    "Creating dataset snapshot from {} to {} ...",
                    src.display(),
                    dest.display()
                );
                let version = vox_corpus::dataset_snapshot::create_snapshot(&src, &dest)?;
                println!("✓ Snapshot created: {}", version);
                Ok(())
            }
            PopuliCorpusCmd::IngestWorkflows {
                repository: _repository,
                output: _output,
            } => {
                #[cfg(feature = "mens-dei")]
                {
                    use std::io::BufWriter;
                    println!(
                        "Ingesting workflow traces from repository '{}' to {} ...",
                        _repository,
                        _output.display()
                    );
                    if let Some(parent) = _output.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let db = vox_db::VoxDb::connect(
                        vox_db::resolve_canonical_config().map_err(|e| anyhow::anyhow!(e))?,
                    )
                    .await?;
                    let mut f = BufWriter::new(std::fs::File::create(&_output)?);
                    let count = vox_orchestrator::services::topology_ingest::ingest_workflow_traces_to_jsonl(
                        &db,
                        &_repository,
                        &mut f
                    ).await?;
                    println!("✓ Ingested {} workflow traces", count);
                    Ok(())
                }
                #[cfg(not(feature = "mens-dei"))]
                {
                    anyhow::bail!("IngestWorkflows requires `dei` feature (vox-orchestrator)")
                }
            }
        },
        PopuliCli::Attest { cmd } => crate::commands::populi_attest::run(cmd).await,
        PopuliCli::Join(args) => crate::commands::populi_join::run(args).await,
    }
}
