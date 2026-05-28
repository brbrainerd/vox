// ── Backward compat alias ─────────────────────────────────────────────────────

/// Alias for [`CloudJobSpec`] — retained for any existing call-sites.
///
/// New code should use [`CloudJobSpec`] directly.
pub type TrainCommand = CloudJobSpec;

// ── Job handle ────────────────────────────────────────────────────────────────

/// Running cloud job reference with billing state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHandle {
    /// Which provider is running this job.
    pub provider: ProviderKind,
    /// Provider-specific job/instance/pod identifier.
    pub job_id: String,
    /// Wall-clock time when the job was dispatched.
    pub started_at: SystemTime,
    /// Estimated total duration in seconds (from `TimeEstimator`).
    pub estimated_seconds: f64,
    /// Hourly rate at dispatch time (used for cost accrual).
    pub price_per_hour_usd: f64,
    /// Whether this job is a persistent mesh node (ignores idle/time limits).
    pub is_persistent: bool,
}

impl JobHandle {
    /// Current accrued cost estimate (elapsed × price_per_hr).
    pub fn accrued_cost_usd(&self) -> f64 {
        let elapsed_hrs = self.started_at.elapsed().unwrap_or_default().as_secs_f64() / 3600.0;
        elapsed_hrs * self.price_per_hour_usd
    }

    /// Elapsed seconds since dispatch.
    pub fn elapsed_secs(&self) -> f64 {
        self.started_at.elapsed().unwrap_or_default().as_secs_f64()
    }
}

// ── Job status ────────────────────────────────────────────────────────────────

/// Current observed state returned by [`CloudProvider::poll_status`].
#[derive(Debug, Clone)]
pub enum JobStatus {
    /// Instance being provisioned; not yet running.
    Pending,
    /// Instance is actively running.
    Running {
        /// Training progress as [0.0, 1.0] if available from telemetry.
        progress_pct: Option<f32>,
        /// GPU utilization as a percentage [0, 100] if the provider exposes it.
        gpu_util_pct: Option<f32>,
    },
    /// Job finished normally.
    Completed {
        /// Whether the adapter was uploaded to HuggingFace Hub.
        adapter_uploaded: bool,
    },
    /// Job was terminated by the watchdog or destroyed by the provider.
    Terminated,
    /// Job failed with an error message.
    Failed(String),
}

// ── Termination reason ────────────────────────────────────────────────────────

/// Canonical termination reason stored in `cloud_dispatch_log.termination_reason`.
pub enum TerminationReason {
    /// Job completed normally.
    Completed,
    /// Watchdog killed for exceeding time × factor.
    WatchdogTime,
    /// Watchdog killed for budget exhaustion.
    WatchdogBudget,
    /// Watchdog killed for GPU idle too long.
    WatchdogIdle,
    /// Watchdog killed for hitting absolute hard cap.
    WatchdogAbsoluteCap,
    /// Provider API unreachable for `max_poll_failures` polls.
    Orphaned,
    /// Explicitly terminated by the user.
    UserRequest,
    /// Job failed with some error.
    Failed,
}

impl TerminationReason {
    /// Canonical string stored in Arca.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::WatchdogTime => "watchdog_time",
            Self::WatchdogBudget => "watchdog_budget",
            Self::WatchdogIdle => "watchdog_idle",
            Self::WatchdogAbsoluteCap => "watchdog_abs_cap",
            Self::Orphaned => "orphaned",
            Self::UserRequest => "user",
            Self::Failed => "failed",
        }
    }
}

// ── CloudProvider trait ───────────────────────────────────────────────────────

/// Interface every cloud provider must implement.
///
/// Implementations: [`vast::VastClient`], [`runpod_provider::RunPodClient`].
#[async_trait::async_trait]
pub trait CloudProvider: Send + Sync {
    /// Identifies which provider this is.
    fn kind(&self) -> ProviderKind;

    /// Human-readable provider name.
    fn name(&self) -> &str {
        self.kind().display_name()
    }

    /// List available GPU offers filtered to minimum VRAM.
    ///
    /// Results should be cached for `price_cache_ttl_secs` to avoid hammering the API.
    async fn list_offers(&self, min_vram_mb: u64) -> anyhow::Result<Vec<GpuOffer>>;

    /// Dispatch a job to the given offer and return a handle.
    ///
    /// **Implementations must re-confirm the offer is still available** before
    /// creating the instance — spot prices change rapidly on both platforms.
    async fn dispatch(&self, offer: &GpuOffer, spec: &CloudJobSpec) -> anyhow::Result<JobHandle>;

    /// Poll the current status of a dispatched job.
    async fn poll_status(&self, handle: &JobHandle) -> anyhow::Result<JobStatus>;

    /// Terminate a job immediately and release the GPU.
    async fn terminate(&self, handle: &JobHandle) -> anyhow::Result<()>;

    /// Retrieve the public endpoint URL for a serving job, if it is available.
    /// Returns `None` if the pod is not yet fully provisioned with network info.
    async fn get_serve_url(
        &self,
        handle: &JobHandle,
        serve_port: u16,
    ) -> anyhow::Result<Option<String>>;
}
