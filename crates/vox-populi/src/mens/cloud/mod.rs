//! Cloud GPU dispatch — domain model and provider trait.
//!
//! Single source of truth for all cloud provider types.
//! Every provider implementation ([`vast`], [`runpod_provider`]) imports from here.
//!
//! # Architecture
//!
//! ```text
//! resolver::CloudResolver
//!   → VastClient (vast.rs, reqwest → cloud.vast.ai)
//!   → RunPodClient (runpod_provider.rs, reqwest → rest.runpod.io/v1)
//!   → TimeEstimator (estimator.rs, gpu-specs.yaml + Arca profiles)
//!   → BudgetLedger (budget.rs, Arca-backed spend tracking)
//!   → CloudWatchdog (watchdog.rs, idle + time + cost kill daemon)
//! ```
//!
//! # Environment variables
//!
//! | Variable | Default | Purpose |
//! |---|---|---|
//! | `VOX_VAST_API_KEY` | — | Vast.ai API key |
//! | `VOX_RUNPOD_API_KEY` | — | RunPod API key |
//! | `VOX_CLOUD_MAX_BUDGET` | `10.00` | Global spend cap USD |
//! | `VOX_CLOUD_PRICE_TTL` | `30` | Offer cache TTL seconds |
//! | `VOX_CLOUD_IMAGE` | `ghcr.io/vox-foundation/vox-ml-cli-cuda:latest` | Container image |
//! | `VOX_CLOUD_MAX_RUNTIME` | `3600` | Absolute hard cap seconds (any job kind) |

pub mod budget;
pub mod estimator;
pub mod resolver;
pub mod runpod_provider;
pub mod vast;
pub mod watchdog;

pub use budget::BudgetLedger;
pub use estimator::TimeEstimator;
pub use resolver::{CloudResolver, ResolveRequest};

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

// ── Provider kind ─────────────────────────────────────────────────────────────

/// Identifies which cloud provider an offer or job comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Vast.ai marketplace — primary provider (cheapest spot prices).
    Vast,
    /// RunPod — secondary provider (reliable fallback, official REST API).
    RunPod,
    /// Local GPU — zero cost, displayed in offer table for comparison.
    Local,
}

impl ProviderKind {
    /// Lowercase display name for tables and DB storage.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Vast => "vast",
            Self::RunPod => "runpod",
            Self::Local => "local",
        }
    }
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

// ── Job kind ──────────────────────────────────────────────────────────────────

/// Distinguishes what the cloud GPU is being used for.
///
/// Affects billing model: `Train` terminates on completion;
/// `Infer`/`Agent` are billed by uptime and require `--max-runtime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// `vox mens train` — terminates when training completes.
    Train,
    /// `vox mens serve` — persistent inference server.
    Infer,
    /// Mens agent execution — same billing model as `Infer`.
    Agent,
}

impl JobKind {
    /// Lowercase string for Arca `cloud_dispatch_log.job_kind` column.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Infer => "infer",
            Self::Agent => "agent",
        }
    }

    /// Whether this job kind requires an explicit `max_runtime_secs`.
    pub fn requires_explicit_runtime(&self) -> bool {
        matches!(self, Self::Infer | Self::Agent)
    }
}

// ── Provider configuration ────────────────────────────────────────────────────

/// Default Docker image for cloud GPU jobs.
pub const DEFAULT_CLOUD_IMAGE: &str = "ghcr.io/vox-foundation/vox-ml-cli-cuda:latest";

/// Conservative fallback ms/step when no measured profile exists.
/// At 200 ms/step a 5k-sample × 3-epoch run takes ~600s.
/// Keeps cost estimates safely non-trivial on unknown hardware.
pub const CONSERVATIVE_MS_PER_STEP: f64 = 200.0;

/// Runtime configuration for cloud dispatch, loaded from env + Vox.toml.
///
/// All fields have reasonable defaults; override any via env vars.
#[derive(Debug, Clone)]
pub struct CloudProviderConfig {
    /// Global maximum spend across all running jobs. Default $10.00.
    pub max_budget_usd: f64,
    /// Minimum Vast.ai `reliability2` score [0.0, 1.0]. Default 0.90.
    pub min_reliability: f32,
    /// Minimum CUDA version on host (from `cuda_max_good`). Default 12.0.
    pub min_cuda_version: f32,
    /// Price cache TTL in seconds. Default 30.
    pub price_cache_ttl_secs: u64,
    /// Kill at this multiple of estimated time. Default 1.5.
    pub watchdog_time_factor: f64,
    /// Kill when GPU util stays below this pct for `watchdog_idle_grace_secs`. Default 5%.
    pub watchdog_idle_pct: f32,
    /// Grace period before idle kill in seconds. Default 300.
    pub watchdog_idle_grace_secs: u64,
    /// Watchdog poll interval in seconds. Default 60.
    pub watchdog_poll_secs: u64,
    /// Startup grace — don't count idle until this many seconds have elapsed. Default 120.
    pub watchdog_startup_grace_secs: u64,
    /// Maximum consecutive poll failures before marking job orphaned. Default 5.
    pub watchdog_max_poll_failures: u32,
    /// **Absolute hard cap** — job always terminated at this many seconds regardless
    /// of estimate. Set to 0 to disable (not recommended). Default 3600 (1 hour).
    pub absolute_max_runtime_secs: u64,
    /// RunPod cloud type: community (cheaper) or secure (SLA). Default: Community.
    pub runpod_cloud_type: RunPodCloudType,
    /// Default RunPod reliability when the API does not expose it. Default 92%.
    pub runpod_default_reliability_pct: f32,
    /// Docker image to use for cloud GPU jobs.
    pub image_tag: String,
    /// Disk space in GB allotted on the cloud instance. Default 80.
    pub disk_gb: u32,
    /// RunPod network volume size in GB (for checkpoint persistence). Default 50.
    pub volume_gb: u32,
    /// Vast.ai bid markup over median (e.g. 1.05 = 5% over median). Default 1.05.
    pub vast_bid_markup: f64,
    /// Maximum offers to fetch per provider per query. Default 100.
    pub max_offers: u32,
    /// Minimum deadline in seconds even if estimate is tiny. Default 300.
    pub min_deadline_secs: u64,
}

impl Default for CloudProviderConfig {
    fn default() -> Self {
        let max_budget = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCloudMaxBudget)
            .expose()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(10.0);
        let cache_ttl = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCloudPriceTtl)
            .expose()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30_u64);
        let image = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCloudImage)
            .expose()
            .unwrap_or(DEFAULT_CLOUD_IMAGE)
            .to_string();
        let abs_max = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCloudMaxRuntime)
            .expose()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3600_u64);
        Self {
            max_budget_usd: max_budget,
            min_reliability: 0.90,
            min_cuda_version: 12.0,
            price_cache_ttl_secs: cache_ttl,
            watchdog_time_factor: 1.5,
            watchdog_idle_pct: 5.0,
            watchdog_idle_grace_secs: 300,
            watchdog_poll_secs: 60,
            watchdog_startup_grace_secs: 120,
            watchdog_max_poll_failures: 5,
            absolute_max_runtime_secs: abs_max,
            runpod_cloud_type: RunPodCloudType::Community,
            runpod_default_reliability_pct: 92.0,
            image_tag: image,
            disk_gb: 80,
            volume_gb: 50,
            vast_bid_markup: 1.05,
            max_offers: 100,
            min_deadline_secs: 300,
        }
    }
}

/// RunPod cloud tier selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunPodCloudType {
    /// Community cloud — cheaper, less SLA.
    #[default]
    Community,
    /// Secure cloud — RunPod-managed datacenters, better SLA.
    Secure,
}

impl RunPodCloudType {
    /// API string value for the `cloudType` field.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Community => "COMMUNITY",
            Self::Secure => "SECURE",
        }
    }
}

// ── GPU offer ─────────────────────────────────────────────────────────────────

/// Normalized GPU availability record from any provider.
///
/// Both [`vast::VastClient`] and [`runpod_provider::RunPodClient`] transform
/// their raw API responses into this common shape so the resolver can rank them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuOffer {
    /// Which cloud provider this offer is from.
    pub provider: ProviderKind,
    /// Provider-specific handle (Vast instance ID, RunPod GPU type ID).
    pub offer_id: String,
    /// Normalized GPU name (lowercase, vendor prefix stripped).
    /// Examples: `"rtx 4090"`, `"a100-sxm4-80gb"`, `"h100 sxm"`.
    pub gpu_name: String,
    /// Number of GPUs in this offer.
    pub gpu_count: u32,
    /// Total VRAM in MB across all GPUs.
    pub vram_mb: u64,
    /// Live price per hour in USD — always from provider API, never hardcoded.
    pub price_per_hour_usd: f64,
    /// Whether the provider may preempt this instance.
    pub is_spot: bool,
    /// Provider reliability score as a percentage [0, 100].
    pub reliability_pct: f32,
    /// Whether the provider supports fire-and-forget termination.
    ///
    /// Vast.ai: `true` (via `onstart` shell hook).
    /// RunPod: `false` — watchdog polls and terminates programmatically.
    pub auto_terminate: bool,
    /// When this offer was fetched from the provider (monotonic clock).
    #[serde(skip)]
    pub fetched_at: Option<std::time::Instant>,
    /// Provider datacenter region hint, if available.
    pub datacenter_region: Option<String>,
    /// Max CUDA version tested on Vast.ai host (used for CUDA version gate).
    pub cuda_max: Option<f32>,
}

impl GpuOffer {
    /// Whether this offer's price data is older than `max_age`.
    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.fetched_at
            .map(|t| t.elapsed() > max_age)
            .unwrap_or(true)
    }
}

// ── Cloud job spec ────────────────────────────────────────────────────────────

/// Parameters for dispatching any cloud GPU job (train, serve, or agent).
///
/// Previously named `TrainCommand`; renamed to reflect that it covers all job kinds.
#[derive(Debug, Clone)]
pub struct CloudJobSpec {
    /// HuggingFace model repo. Example: `"Qwen/Qwen3.5-4B"` (see `DEFAULT_MODEL_ID`).
    pub model_id: String,
    /// Training preset name (`"auto"` lets the cloud instance auto-detect via VRAM).
    pub preset: String,
    /// HuggingFace dataset repo ID for `train.jsonl`. Only used by `Train` jobs.
    pub train_data_hf: Option<String>,
    /// HuggingFace model repo to upload the trained adapter to. Only used by `Train` jobs.
    pub adapter_upload_hf: Option<String>,
    /// Docker image tag. Defaults to `CloudProviderConfig.image_tag`.
    pub image_tag: String,
    /// Additional environment variables injected into the remote container.
    pub extra_env: Vec<(String, String)>,
    /// What this job is doing.
    pub job_kind: JobKind,
    /// Provider-specific volume ID for checkpoint resume on spot preemption.
    pub checkpoint_volume: Option<String>,
    /// Maximum runtime in seconds. **Required for `Infer`/`Agent`.**
    /// For `Train`, derived from the time estimate × `watchdog_time_factor` if not set.
    pub max_runtime_secs: Option<u64>,
    /// Optional per-job spend cap. Defaults to `CloudProviderConfig.max_budget_usd`.
    pub max_budget_usd: Option<f64>,
    /// Estimated sequence length (for time ranking). Default 256.
    pub seq_len: usize,
    /// Estimated total training samples (for time ranking). Default 5000.
    pub num_samples: usize,
    /// Estimated training epochs (for time ranking). Default 3.
    pub epochs: usize,
    /// Estimated micro-batch size (for time ranking). Default 4.
    pub batch_size: usize,
    /// Port to expose for `Infer`/`Agent` jobs. Default 8080.
    pub serve_port: u16,
    /// Keep the instance alive after task completion as a persistent mesh node.
    pub persistent: bool,
}

impl CloudJobSpec {
    /// Construct from config defaults. Use builder pattern for non-default fields.
    pub fn new_train(config: &CloudProviderConfig) -> Self {
        Self {
            model_id: crate::mens::default_model_id(),
            preset: "auto".to_string(),
            train_data_hf: None,
            adapter_upload_hf: None,
            image_tag: config.image_tag.clone(),
            extra_env: vec![],
            job_kind: JobKind::Train,
            checkpoint_volume: None,
            max_runtime_secs: None,
            max_budget_usd: None,
            seq_len: 256,
            num_samples: 5000,
            epochs: 3,
            batch_size: 4,
            serve_port: 8080,
            persistent: false,
        }
    }

    /// Construct a serve job spec.
    pub fn new_serve(config: &CloudProviderConfig, max_runtime_secs: u64) -> Self {
        Self {
            model_id: crate::mens::default_model_id(),
            preset: "auto".to_string(),
            train_data_hf: None,
            adapter_upload_hf: None,
            image_tag: config.image_tag.clone(),
            extra_env: vec![],
            job_kind: JobKind::Infer,
            checkpoint_volume: None,
            max_runtime_secs: Some(max_runtime_secs),
            max_budget_usd: None,
            seq_len: 256, // not used for serve
            num_samples: 0,
            epochs: 0,
            batch_size: 1,
            serve_port: 8080,
            persistent: false,
        }
    }

    /// The effective max runtime: explicit > absolute_max from config > 0 (no cap).
    pub fn effective_max_runtime(&self, config: &CloudProviderConfig) -> u64 {
        let explicit = self.max_runtime_secs.unwrap_or(0);
        let abs = config.absolute_max_runtime_secs;
        match (explicit, abs) {
            (0, 0) => u64::MAX, // no cap (not recommended)
            (0, a) => a,
            (e, 0) => e,
            (e, a) => e.min(a),
        }
    }
}
include!("part_jobs.rs");
include!("part_cli.rs");
pub mod local_provider;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_geforce() {
        assert_eq!(normalize_gpu_name("NVIDIA GeForce RTX 4090"), "rtx 4090");
    }

    #[test]
    fn normalize_strips_tesla() {
        let n = normalize_gpu_name("Tesla V100-SXM2-16GB");
        assert!(n.contains("v100"), "{n}");
    }

    #[test]
    fn normalize_a100_sxm() {
        let n = normalize_gpu_name("NVIDIA A100-SXM4-80GB");
        assert!(n.contains("a100"), "{n}");
    }

    #[test]
    fn normalize_no_double_ada() {
        // Must not have duplicate removal artifacts
        let n = normalize_gpu_name("NVIDIA RTX 6000 Ada Lovelace");
        assert!(!n.contains("ada"), "{n}");
    }

    #[test]
    fn job_handle_accrued_zero_at_start() {
        let h = JobHandle {
            provider: ProviderKind::Vast,
            job_id: "test".into(),
            started_at: SystemTime::now(),
            estimated_seconds: 3600.0,
            price_per_hour_usd: 1.0,
            is_persistent: false,
        };
        assert!(h.accrued_cost_usd() < 0.01);
    }

    #[test]
    fn provider_kind_display() {
        assert_eq!(ProviderKind::Vast.to_string(), "vast");
        assert_eq!(ProviderKind::RunPod.to_string(), "runpod");
        assert_eq!(ProviderKind::Local.to_string(), "local");
    }

    #[test]
    fn cloud_target_parse() {
        assert_eq!("auto".parse::<CloudTarget>().unwrap(), CloudTarget::Auto);
        assert_eq!("vast".parse::<CloudTarget>().unwrap(), CloudTarget::Vast);
        assert_eq!(
            "runpod".parse::<CloudTarget>().unwrap(),
            CloudTarget::RunPod
        );
        assert!("gcp".parse::<CloudTarget>().is_err());
    }

    #[test]
    fn job_spec_effective_runtime_caps() {
        let cfg = CloudProviderConfig {
            absolute_max_runtime_secs: 3600,
            ..Default::default()
        };
        let spec = CloudJobSpec {
            max_runtime_secs: Some(7200),
            ..CloudJobSpec::new_train(&cfg)
        };
        // explicit 7200 capped by abs 3600
        assert_eq!(spec.effective_max_runtime(&cfg), 3600);
    }
}
