//! Cloud GPU offer resolver — parallel query, budget gate, cost-ranked dispatch.
//!
//! Queries all configured cloud providers in parallel, applies filters, estimates
//! cost per job, gates on the budget ledger, and returns a ranked list of offers.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    BudgetLedger, CloudJobSpec, CloudProvider, CloudProviderConfig, CloudTarget, GpuOffer,
    JobHandle, JobKind, ProviderKind,
    estimator::{EstimateSource, TimeEstimator},
    runpod_provider::RunPodClient,
    vast::VastClient,
    watchdog::CloudWatchdog,
};

/// A ranked cloud GPU offer with cost estimation.
pub struct ResolvedOffer {
    /// The raw offer from the provider.
    pub offer: GpuOffer,
    /// Estimated total time in seconds (including overhead).
    pub estimated_secs: f64,
    /// Estimated total cost in USD.
    pub estimated_cost_usd: f64,
    /// Which estimation tier produced this result.
    pub estimate_source: EstimateSource,
    /// Canonical preset name for this GPU's VRAM tier.
    pub effective_preset: &'static str,
}

/// Request parameters passed to [`CloudResolver::resolve`].
pub struct ResolveRequest {
    /// Minimum VRAM needed for this job.
    pub min_vram_mb: u64,
    /// Training sequence length (for time estimation).
    pub seq_len: usize,
    /// Micro-batch size (for time estimation).
    pub batch_size: usize,
    /// Total number of training pairs.
    pub num_samples: usize,
    /// Number of training epochs.
    pub epochs: usize,
    /// Maximum acceptable estimated cost in USD.
    pub max_acceptable_cost: f64,
    /// Which providers to query.
    pub target: CloudTarget,
}

/// Overhead factor per provider (accounts for launch + teardown time in cost estimate).
///
/// Vast.ai: fire-and-forget termination → 10% overhead.
/// RunPod: watchdog-polled termination → 20% overhead.
const OVERHEAD_AUTO_TERMINATE: f64 = 1.10;
const OVERHEAD_POLL_TERMINATE: f64 = 1.20;

/// VRAM-tiered preset names aligned with the presets section in `gpu-specs.yaml`.
pub fn preset_for_vram(vram_mb: u64) -> &'static str {
    match vram_mb {
        0..=8191 => "tiny",
        8192..=10239 => "safe",
        10240..=16383 => "prosumer_16g",
        16384..=23039 => "prosumer_16g",
        23040..=39679 => "prosumer_24g",
        39680..=49151 => "a6000",
        49152..=81919 => "a100",
        _ => "h100",
    }
}

/// Queries cloud providers, estimates costs, and returns ranked offers.
///
/// Owns the provider clients so `dispatch_top` can reuse them without
/// creating new clients (which would discard config).
pub struct CloudResolver {
    vast: Option<Arc<VastClient>>,
    runpod: Option<Arc<RunPodClient>>,
    local: Option<Arc<super::local_provider::LocalProvider>>,
    estimator: TimeEstimator,
    pub budget: Arc<BudgetLedger>,
    pub config: Arc<CloudProviderConfig>,
}

impl CloudResolver {
    /// Construct a resolver.
    ///
    /// - `gpu_specs_path`: path to `mens/config/gpu-specs.yaml`
    /// - `profiles`: output of `VoxDb::cloud_load_throughput_profiles()`
    /// - `budget`: Arca-backed spend ledger
    /// - `config`: shared provider config (reused by watchdog and dispatch)
    pub fn new(
        gpu_specs_path: PathBuf,
        profiles: Vec<(String, usize, usize, f64)>,
        budget: Arc<BudgetLedger>,
        config: Arc<CloudProviderConfig>,
        local: Option<Arc<super::local_provider::LocalProvider>>,
    ) -> anyhow::Result<Self> {
        let estimator = TimeEstimator::new(&gpu_specs_path, profiles)?;
        let vast = VastClient::from_env(Arc::clone(&config)).ok().map(Arc::new);
        let runpod = RunPodClient::from_env(Arc::clone(&config))
            .ok()
            .map(Arc::new);
        if vast.is_none() && runpod.is_none() && local.is_none() {
            anyhow::bail!(
                "No GPU providers available. Either:\n\
                 - Set VOX_VAST_API_KEY (Vast.ai) or VOX_RUNPOD_API_KEY (RunPod)\n\
                 - Ensure a GPU is detected locally (vox mens probe)"
            );
        }
        Ok(Self {
            vast,
            runpod,
            local,
            estimator,
            budget,
            config,
        })
    }

    /// Convenience: build a resolver from environment and any attached Arca store.
    pub async fn new_from_env() -> anyhow::Result<Self> {
        let root = vox_corpus::training::contract::find_workspace_root().ok_or_else(|| {
            anyhow::anyhow!("Could not find workspace root (required for gpu-specs.yaml)")
        })?;
        let specs_path = root.join("mens/config/gpu-specs.yaml");
        let config = Arc::new(CloudProviderConfig::default());

        let db = vox_db::connect_canonical_optional(
            vox_db::DbConnectSurface::PopuliCloudResolver,
            false,
        )
        .await
        .map(Arc::new);
        let budget = Arc::new(BudgetLedger::new(db.clone(), &config));

        let profiles = if let Some(ref voxdb) = db {
            voxdb.cloud_load_throughput_profiles().await?
        } else {
            vec![]
        };

        // Initialize local provider if hardware supports it
        let hw = crate::mens::hardware::HardwareRegistry::probe().await;
        let local = if hw.vram_mb > 0 {
            Some(Arc::new(super::local_provider::LocalProvider::new(
                hw,
                config.clone(),
            )))
        } else {
            None
        };

        Self::new(specs_path, profiles, budget, config, local)
    }

    /// Query all configured providers and return offers ranked by estimated cost.
    ///
    /// Providers are queried in parallel; individual failures are logged and skipped.
    pub async fn resolve(&self, req: &ResolveRequest) -> anyhow::Result<Vec<ResolvedOffer>> {
        self.budget.check_capacity(req.max_acceptable_cost).await?;

        let use_vast =
            matches!(req.target, CloudTarget::Auto | CloudTarget::Vast) && self.vast.is_some();
        let use_runpod =
            matches!(req.target, CloudTarget::Auto | CloudTarget::RunPod) && self.runpod.is_some();
        let use_local =
            matches!(req.target, CloudTarget::Auto | CloudTarget::Local) && self.local.is_some();

        let (vast_r, runpod_r, local_r) = tokio::join!(
            async {
                if use_vast {
                    self.vast
                        .as_ref()
                        .unwrap()
                        .list_offers(req.min_vram_mb)
                        .await
                } else {
                    Ok(vec![])
                }
            },
            async {
                if use_runpod {
                    self.runpod
                        .as_ref()
                        .unwrap()
                        .list_offers(req.min_vram_mb)
                        .await
                } else {
                    Ok(vec![])
                }
            },
            async {
                if use_local {
                    self.local
                        .as_ref()
                        .unwrap()
                        .list_offers(req.min_vram_mb)
                        .await
                } else {
                    Ok(vec![])
                }
            },
        );

        let mut all: Vec<GpuOffer> = vec![];
        if let Ok(v) = local_r {
            all.extend(v);
        }
        match vast_r {
            Ok(v) => all.extend(v),
            Err(e) => tracing::warn!("Vast.ai query failed (skipping): {e}"),
        }
        match runpod_r {
            Ok(v) => all.extend(v),
            Err(e) => tracing::warn!("RunPod query failed (skipping): {e}"),
        }

        if all.is_empty() {
            anyhow::bail!(
                "No GPU offers found (min_vram={}MB). Check API keys and filters.",
                req.min_vram_mb
            );
        }

        let remaining = self.budget.remaining_usd().await;

        let mut ranked: Vec<ResolvedOffer> = all
            .into_iter()
            .filter_map(|offer| {
                let overhead = if offer.auto_terminate {
                    OVERHEAD_AUTO_TERMINATE
                } else {
                    OVERHEAD_POLL_TERMINATE
                };
                let (est_secs, source) = self.estimator.estimate(
                    &offer.gpu_name,
                    req.seq_len,
                    req.batch_size,
                    req.num_samples,
                    req.epochs,
                );
                let total_secs = est_secs * overhead;
                let cost = (total_secs / 3600.0) * offer.price_per_hour_usd;

                if cost > remaining || cost > req.max_acceptable_cost {
                    return None;
                }

                Some(ResolvedOffer {
                    effective_preset: preset_for_vram(offer.vram_mb),
                    estimated_secs: total_secs,
                    estimated_cost_usd: cost,
                    estimate_source: source,
                    offer,
                })
            })
            .collect();

        // Sort: cheapest → prefer auto_terminate → higher reliability
        ranked.sort_by(|a, b| {
            a.estimated_cost_usd
                .partial_cmp(&b.estimated_cost_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.offer.auto_terminate.cmp(&a.offer.auto_terminate))
                .then(
                    b.offer
                        .reliability_pct
                        .partial_cmp(&a.offer.reliability_pct)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });

        Ok(ranked)
    }

    /// Dispatch a job to the top-ranked offer, start watchdog, return handle.
    ///
    /// Reuses the resolver's owned provider clients — no config discarding.
    /// Resolve and dispatch the top offer in one call.
    pub async fn dispatch(&self, spec: CloudJobSpec, target_str: &str) -> anyhow::Result<()> {
        use std::str::FromStr;
        let target = CloudTarget::from_str(target_str)?;
        let req = ResolveRequest {
            target,
            min_vram_mb: 24000, // 24GB default (preset "auto" handles specifics)
            max_acceptable_cost: spec.max_budget_usd.unwrap_or(self.config.max_budget_usd),
            seq_len: spec.seq_len,
            batch_size: spec.batch_size,
            num_samples: spec.num_samples,
            epochs: spec.epochs,
        };
        let ranked = self.resolve(&req).await?;
        let (_handle, join, _provider) = self.dispatch_top(&ranked, &spec).await?;

        // Wait for the watchdog if requested or just return handle
        // For the CLI, we usually want to wait until completion or detach.
        // run_train in mod.rs seems to expect a Result<()> and might background it itself.

        join.await
            .map_err(|e| anyhow::anyhow!("Watchdog task failed: {e}"))
    }

    /// Resolve and dispatch the top offer in one call.
    pub async fn dispatch_top(
        &self,
        ranked: &[ResolvedOffer],
        spec: &CloudJobSpec,
    ) -> anyhow::Result<(
        JobHandle,
        tokio::task::JoinHandle<()>,
        Arc<dyn CloudProvider>,
    )> {
        let top = ranked.first().ok_or_else(|| {
            anyhow::anyhow!("No offers to dispatch — resolve returned empty list")
        })?;

        // Validate runtime for serve/agent jobs
        if spec.job_kind.requires_explicit_runtime() && spec.max_runtime_secs.is_none() {
            anyhow::bail!(
                "JobKind::{:?} requires --max-runtime to prevent unbounded billing.",
                spec.job_kind
            );
        }

        // Reuse the owned client — does NOT create new clients or discard config
        let provider: Arc<dyn CloudProvider> = match top.offer.provider {
            ProviderKind::Vast => self
                .vast
                .as_ref()
                .map(|c| Arc::clone(c) as Arc<dyn CloudProvider>)
                .ok_or_else(|| anyhow::anyhow!("Vast.ai client not available"))?,
            ProviderKind::RunPod => self
                .runpod
                .as_ref()
                .map(|c| Arc::clone(c) as Arc<dyn CloudProvider>)
                .ok_or_else(|| anyhow::anyhow!("RunPod client not available"))?,
            ProviderKind::Local => self
                .local
                .as_ref()
                .map(|c| Arc::clone(c) as Arc<dyn CloudProvider>)
                .ok_or_else(|| anyhow::anyhow!("Local provider not available"))?,
        };

        let mut handle = provider.dispatch(&top.offer, spec).await?;
        handle.estimated_seconds = top.estimated_secs;

        // Record in Arca
        self.budget
            .open_job(
                &handle,
                &top.offer.offer_id,
                &top.offer.gpu_name,
                top.offer.vram_mb,
                top.estimated_cost_usd,
                spec.job_kind.as_str(),
            )
            .await?;

        // Spawn watchdog with THE SAME config (not a new default)
        let watchdog = CloudWatchdog {
            provider: Arc::clone(&provider),
            handle: handle.clone(),
            budget: Arc::clone(&self.budget),
            config: Arc::clone(&self.config),
        };
        let wh = watchdog.spawn();

        Ok((handle, wh, provider))
    }

    /// Print a ranked offer table to stdout (CLI display).
    pub fn print_offer_table(ranked: &[ResolvedOffer]) {
        if ranked.is_empty() {
            println!("No offers available within budget.");
            return;
        }
        println!(
            "\n{:<8} {:<20} {:<7} {:<9} {:<10} {:<10} {:<5} {:<20}",
            "Provider", "GPU", "VRAM", "$/hr", "Est Time", "Est Cost", "Auto", "Est. Source"
        );
        println!("{}", "-".repeat(95));
        for r in ranked {
            let mins = r.estimated_secs as u64 / 60;
            let time_str = if mins >= 60 {
                format!("{}h{:02}m", mins / 60, mins % 60)
            } else {
                let secs = r.estimated_secs as u64 % 60;
                format!("{mins}m{secs:02}s")
            };
            // Safe UTF-8 truncation — no panic on multi-byte chars
            let gpu_display: String = r.offer.gpu_name.chars().take(18).collect();
            println!(
                "{:<8} {:<20} {:<7} ${:<8.3} {:<10} ${:<9.2} {:<5} {}",
                r.offer.provider.display_name(),
                gpu_display,
                format!(
                    "{} GB",
                    r.offer.vram_mb / crate::mens::hardware::types::MB_PER_GB
                ),
                r.offer.price_per_hour_usd,
                time_str,
                r.estimated_cost_usd,
                if r.offer.auto_terminate { "✓" } else { "" },
                r.estimate_source,
            );
        }
        println!();
    }
}

/// Convenience constructor for a standard training job spec.
pub fn build_train_spec(
    config: &CloudProviderConfig,
    model_id: Option<String>,
    train_data_hf: Option<String>,
    adapter_upload_hf: Option<String>,
    extra_env: Vec<(String, String)>,
) -> CloudJobSpec {
    CloudJobSpec {
        model_id: model_id.unwrap_or_else(crate::mens::default_model_id),
        preset: "auto".to_string(),
        train_data_hf,
        adapter_upload_hf,
        image_tag: config.image_tag.clone(),
        extra_env,
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

/// Convenience constructor for a serve/inference job spec.
pub fn build_serve_spec(
    config: &CloudProviderConfig,
    model_id: String,
    max_runtime_secs: u64,
    serve_port: u16,
) -> CloudJobSpec {
    CloudJobSpec {
        model_id,
        preset: "auto".to_string(),
        train_data_hf: None,
        adapter_upload_hf: None,
        image_tag: config.image_tag.clone(),
        extra_env: vec![],
        job_kind: JobKind::Infer,
        checkpoint_volume: None,
        max_runtime_secs: Some(max_runtime_secs),
        max_budget_usd: None,
        seq_len: 256,
        num_samples: 0,
        epochs: 0,
        batch_size: 1,
        serve_port,
        persistent: false,
    }
}
