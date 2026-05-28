//! Vast.ai cloud GPU client.
//!
//! Uses the Vast.ai REST API v0 (`cloud.vast.ai/api/v0/`).
//! Auth: `Authorization: Bearer {VOX_VAST_API_KEY}` on all requests.
//!
//! Endpoints:
//! - `GET  /bundles/`          — list available GPU offers
//! - `PUT  /asks/{offer_id}/`  — create instance from offer
//! - `GET  /instances/{id}/`   — poll instance status
//! - `DELETE /instances/{id}/` — terminate instance
//!
//! Termination: the `onstart` script calls `/entrypoint.sh`, which self-terminates
//! via `DELETE /instances/{id}/` on completion. The watchdog is a mandatory fallback.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{
    CloudJobSpec, CloudProvider, CloudProviderConfig, GpuOffer, JobHandle, JobKind, JobStatus,
    ProviderKind, normalize_gpu_name,
};

const BASE_URL: &str = "https://cloud.vast.ai/api/v0";

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct VastOffer {
    id: u64,
    gpu_name: String,
    /// Total VRAM in MB.
    gpu_ram: u64,
    num_gpus: Option<u32>,
    /// Total $/hr (all-inclusive).
    dph_total: f64,
    /// Host reliability [0.0, 1.0].
    reliability2: Option<f64>,
    /// Highest CUDA version tested (e.g. `12.4`).
    cuda_max_good: Option<f64>,
    /// GPU utilization percentage [0, 100] — NOT [0, 1] (reserved for future offer ranking).
    #[allow(dead_code)]
    gpu_utilization: Option<f64>,
}

#[derive(Deserialize)]
struct VastBundleResponse {
    offers: Vec<VastOffer>,
}

// ── Cache ─────────────────────────────────────────────────────────────────────

struct VastCache {
    fetched_at: Option<Instant>,
    offers: Vec<GpuOffer>,
}

// ── Client ────────────────────────────────────────────────────────────────────

/// Vast.ai REST API client.
pub struct VastClient {
    http: reqwest::Client,
    api_key: String,
    config: Arc<CloudProviderConfig>,
    cache: RwLock<VastCache>,
}

impl VastClient {
    /// Construct using `VOX_VAST_API_KEY` environment variable.
    pub fn from_env(config: Arc<CloudProviderConfig>) -> anyhow::Result<Self> {
        let api_key = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxVastApiKey)
            .expose()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| {
                anyhow::anyhow!("VOX_VAST_API_KEY not set. Get it at: https://cloud.vast.ai/cli/")
            })?;
        Ok(Self::new(api_key, config))
    }

    /// Construct with explicit API key.
    pub fn new(api_key: String, config: Arc<CloudProviderConfig>) -> Self {
        let http = vox_http_client::client_builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest TLS stack must be available");
        Self {
            http,
            api_key,
            config,
            cache: RwLock::new(VastCache {
                fetched_at: None,
                offers: vec![],
            }),
        }
    }

    /// Generate the `onstart` script for the PUT body.
    ///
    /// Calls the Docker entrypoint (`/entrypoint.sh`) so the logic is not duplicated.
    /// The container image must have `/entrypoint.sh` as its CMD/ENTRYPOINT.
    fn build_onstart_script(&self, _spec: &CloudJobSpec) -> String {
        // The entrypoint handles all logic: download, train/serve/agent, upload, self-terminate.
        // We only need to set env vars here; the entrypoint reads them.
        "exec /entrypoint.sh".to_string()
    }

    /// Build env map for the PUT /asks/ body.
    fn build_env_map(&self, spec: &CloudJobSpec) -> serde_json::Map<String, serde_json::Value> {
        let mut env = serde_json::Map::new();
        // API key — needed by entrypoint for self-termination curl
        env.insert("VOX_VAST_API_KEY".into(), self.api_key.clone().into());
        env.insert("VOX_MODEL_ID".into(), spec.model_id.clone().into());
        env.insert("VOX_JOB_KIND".into(), spec.job_kind.as_str().into());
        if let Some(ref d) = spec.train_data_hf {
            env.insert("VOX_TRAIN_DATA_HF".into(), d.clone().into());
        }
        if let Some(ref a) = spec.adapter_upload_hf {
            env.insert("VOX_ADAPTER_UPLOAD_HF".into(), a.clone().into());
        }
        if spec.job_kind != JobKind::Train {
            env.insert("VOX_SERVE_PORT".into(), spec.serve_port.to_string().into());
        }
        if spec.persistent {
            env.insert("VOX_PERSISTENT_NODE".into(), "1".into());
        }
        for (k, v) in &spec.extra_env {
            env.insert(k.clone(), v.clone().into());
        }
        env
    }

    /// Compute bid price: median of top-N same-GPU offers × markup.
    fn compute_bid_price(&self, offer: &GpuOffer, all_offers: &[GpuOffer]) -> f64 {
        let mut prices: Vec<f64> = all_offers
            .iter()
            .filter(|o| o.gpu_name == offer.gpu_name)
            .take(10)
            .map(|o| o.price_per_hour_usd)
            .collect();
        if prices.is_empty() {
            return offer.price_per_hour_usd * self.config.vast_bid_markup;
        }
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        prices[prices.len() / 2] * self.config.vast_bid_markup
    }
}

#[async_trait::async_trait]
impl CloudProvider for VastClient {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Vast
    }

    async fn list_offers(&self, min_vram_mb: u64) -> anyhow::Result<Vec<GpuOffer>> {
        let ttl = Duration::from_secs(self.config.price_cache_ttl_secs);
        {
            let cache = self.cache.read().await;
            if cache.fetched_at.map_or(false, |t| t.elapsed() < ttl) {
                return Ok(cache
                    .offers
                    .iter()
                    .filter(|o| o.vram_mb >= min_vram_mb)
                    .cloned()
                    .collect());
            }
        }

        let raw = self
            .http
            .get(format!("{BASE_URL}/bundles/"))
            .query(&[
                ("gpu_ram__gte", min_vram_mb.to_string()),
                ("order", "dph_total".to_string()),
                ("type", "bid".to_string()),
                ("limit", self.config.max_offers.to_string()),
            ])
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("Vast.ai GET /bundles/ request")?;
        let resp = raw
            .error_for_status()
            .context("Vast.ai /bundles/ error status")?
            .json::<VastBundleResponse>()
            .await
            .context("Vast.ai /bundles/ parse")?;

        let now = Instant::now();
        let offers: Vec<GpuOffer> = resp
            .offers
            .into_iter()
            .filter(|o| {
                o.reliability2.unwrap_or(0.0) >= self.config.min_reliability as f64
                    && o.cuda_max_good.unwrap_or(0.0) >= self.config.min_cuda_version as f64
            })
            .map(|o| GpuOffer {
                provider: ProviderKind::Vast,
                offer_id: o.id.to_string(),
                gpu_name: normalize_gpu_name(&o.gpu_name),
                gpu_count: o.num_gpus.unwrap_or(1),
                vram_mb: o.gpu_ram,
                price_per_hour_usd: o.dph_total,
                is_spot: true,
                reliability_pct: (o.reliability2.unwrap_or(0.5) * 100.0) as f32,
                auto_terminate: true,
                fetched_at: Some(now),
                datacenter_region: None,
                cuda_max: o.cuda_max_good.map(|v| v as f32),
            })
            .collect();

        {
            let mut c = self.cache.write().await;
            c.fetched_at = Some(now);
            c.offers = offers.clone();
        }
        Ok(offers
            .into_iter()
            .filter(|o| o.vram_mb >= min_vram_mb)
            .collect())
    }

    async fn dispatch(&self, offer: &GpuOffer, spec: &CloudJobSpec) -> anyhow::Result<JobHandle> {
        let ttl = Duration::from_secs(self.config.price_cache_ttl_secs);
        if offer.is_stale(ttl) {
            anyhow::bail!("Vast.ai offer {} is stale — re-query first", offer.offer_id);
        }

        // Confirm offer still available
        let check = self
            .http
            .get(format!("{BASE_URL}/bundles/"))
            .query(&[("id", &offer.offer_id)])
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("Vast.ai offer availability check")?;
        if !check.status().is_success() {
            anyhow::bail!("Vast.ai offer {} no longer available", offer.offer_id);
        }

        let all_cached = self.cache.read().await.offers.clone();
        let bid = self.compute_bid_price(offer, &all_cached);

        let label = match spec.job_kind {
            JobKind::Train => format!(
                "vox-schola-{}",
                offer.gpu_name.chars().take(12).collect::<String>()
            ),
            JobKind::Infer => "vox-serve".to_string(),
            JobKind::Agent => "vox-agent".to_string(),
        };

        let body = serde_json::json!({
            "client_id": "vox-ml-cli",
            "image": spec.image_tag,
            "disk": self.config.disk_gb,
            "runtype": "ssh",
            "price": bid,
            "onstart": self.build_onstart_script(spec),
            "label": label,
            "env": self.build_env_map(spec),
        });

        let raw = self
            .http
            .put(format!("{BASE_URL}/asks/{}/", offer.offer_id))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("Vast.ai PUT /asks/")?;
        let resp: serde_json::Value = raw
            .error_for_status()
            .context("Vast.ai PUT /asks/ error")?
            .json()
            .await
            .context("Vast.ai PUT /asks/ parse")?;

        let instance_id = resp["new_contract"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Vast.ai: no 'new_contract' in response: {resp}"))?;

        tracing::info!(
            "Vast.ai instance {} created (bid ${bid:.3}/hr, {}, job={:?})",
            instance_id,
            offer.gpu_name,
            spec.job_kind
        );
        Ok(JobHandle {
            provider: ProviderKind::Vast,
            job_id: instance_id.to_string(),
            started_at: std::time::SystemTime::now(),
            estimated_seconds: 0.0,
            price_per_hour_usd: bid,
            is_persistent: spec.persistent,
        })
    }

    async fn poll_status(&self, handle: &JobHandle) -> anyhow::Result<JobStatus> {
        let raw = self
            .http
            .get(format!("{BASE_URL}/instances/{}/", handle.job_id))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("Vast.ai GET /instances/ poll")?;
        let resp: serde_json::Value = raw.error_for_status()?.json().await?;

        let status_str = resp
            .get("actual_status")
            .or_else(|| resp.pointer("/instances/0/actual_status"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Vast.ai returns gpu_utilization as a percentage [0, 100], not a fraction.
        // Do NOT multiply by 100 here.
        let gpu_util = resp
            .get("gpu_utilization")
            .or_else(|| resp.pointer("/instances/0/gpu_utilization"))
            .and_then(|v| v.as_f64())
            .map(|u| u as f32);

        Ok(match status_str {
            "running" => JobStatus::Running {
                progress_pct: None,
                gpu_util_pct: gpu_util,
            },
            "loading" | "scheduling" => JobStatus::Pending,
            "stopped" | "exited" | "destroyed" => JobStatus::Completed {
                adapter_uploaded: false,
            },
            "failed" => JobStatus::Failed(format!("Instance {} failed", handle.job_id)),
            _ => JobStatus::Running {
                progress_pct: None,
                gpu_util_pct: gpu_util,
            },
        })
    }

    async fn terminate(&self, handle: &JobHandle) -> anyhow::Result<()> {
        let raw = self
            .http
            .delete(format!("{BASE_URL}/instances/{}/", handle.job_id))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("Vast.ai DELETE /instances/")?;
        raw.error_for_status().context("Vast.ai terminate error")?;
        tracing::info!("Vast.ai instance {} terminated.", handle.job_id);
        Ok(())
    }

    async fn get_serve_url(
        &self,
        handle: &JobHandle,
        serve_port: u16,
    ) -> anyhow::Result<Option<String>> {
        let raw = self
            .http
            .get(format!("{BASE_URL}/instances/{}/", handle.job_id))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("Vast.ai GET /instances/ poll")?;
        let resp: serde_json::Value = raw.error_for_status()?.json().await?;

        // Format is often {"public_ipaddr": "x.x.x.x", "ports": {"8080/tcp": [{"HostIp": "...", "HostPort": "12345"}]}}
        let public_ip = match resp
            .get("public_ipaddr")
            .or_else(|| resp.pointer("/instances/0/public_ipaddr"))
            .and_then(|v| v.as_str())
        {
            Some(ip) => ip,
            None => return Ok(None),
        };

        let ports_map = resp
            .get("ports")
            .or_else(|| resp.pointer("/instances/0/ports"));
        let external_port = if let Some(map) = ports_map {
            map.pointer(&format!("/{}/tcp/0/HostPort", serve_port))
                .and_then(|v| v.as_str())
        } else {
            None
        };

        if let (Some(ip), Some(ext_port)) = (Some(public_ip), external_port) {
            Ok(Some(format!("http://{}:{}", ip, ext_port)))
        } else {
            Ok(None)
        }
    }
}
