//! RunPod REST API v1 client (`rest.runpod.io/v1`, launched March 2025).
//!
//! Auth: `Authorization: Bearer {VOX_RUNPOD_API_KEY}`.
//! Zero new dependencies — uses `reqwest` gated behind the `cloud` feature.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{
    CloudJobSpec, CloudProvider, CloudProviderConfig, GpuOffer, JobHandle, JobKind, JobStatus,
    ProviderKind, RunPodCloudType, normalize_gpu_name,
};

const BASE: &str = "https://rest.runpod.io/v1";

// ── Wire types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunPodGpuType {
    id: String,
    display_name: Option<String>,
    memory_in_gb: Option<f32>,
    community_price: Option<f64>,
    secure_price: Option<f64>,
}

#[derive(Deserialize)]
struct GpusResp {
    gpus: Option<Vec<RunPodGpuType>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PodResp {
    #[allow(dead_code)]
    id: Option<String>,
    desired_status: Option<String>,
    runtime: Option<PodRuntime>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PodRuntime {
    gpus: Option<Vec<GpuStat>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuStat {
    /// GPU utilization percentage [0, 100].
    gpu_util_percent: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateResp {
    id: Option<String>,
}

// ── Cache ──────────────────────────────────────────────────────────────────

struct Cache {
    fetched_at: Option<Instant>,
    offers: Vec<GpuOffer>,
}

// ── Client ─────────────────────────────────────────────────────────────────

/// RunPod REST API v1 client.
pub struct RunPodClient {
    http: reqwest::Client,
    api_key: String,
    config: Arc<CloudProviderConfig>,
    cache: RwLock<Cache>,
}

impl RunPodClient {
    /// Construct from `VOX_RUNPOD_API_KEY`.
    pub fn from_env(config: Arc<CloudProviderConfig>) -> anyhow::Result<Self> {
        let key = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRunpodApiKey)
            .expose()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| {
            anyhow::anyhow!(
                "VOX_RUNPOD_API_KEY not set. Get it at https://www.runpod.io/console/user/settings"
            )
        })?;
        Ok(Self::new(key, config))
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
            cache: RwLock::new(Cache {
                fetched_at: None,
                offers: vec![],
            }),
        }
    }

    fn map_gpu(&self, g: RunPodGpuType, now: Instant) -> Option<GpuOffer> {
        let name = g.display_name.as_deref().unwrap_or(&g.id).to_string();
        let vram_mb = (g.memory_in_gb? as u64) * 1024;
        let price = match self.config.runpod_cloud_type {
            RunPodCloudType::Community => g.community_price,
            RunPodCloudType::Secure => g.secure_price,
        }?;
        Some(GpuOffer {
            provider: ProviderKind::RunPod,
            offer_id: g.id,
            gpu_name: normalize_gpu_name(&name),
            gpu_count: 1,
            vram_mb,
            price_per_hour_usd: price,
            is_spot: true,
            // RunPod does not expose a per-GPU reliability score in /gpus — use config default
            reliability_pct: self.config.runpod_default_reliability_pct,
            auto_terminate: false,
            fetched_at: Some(now),
            datacenter_region: None,
            cuda_max: None,
        })
    }

    fn build_env(&self, spec: &CloudJobSpec) -> Vec<serde_json::Value> {
        let mut env = vec![
            serde_json::json!({"key": "VOX_MODEL_ID", "value": spec.model_id}),
            // Inject API key so the entrypoint can self-terminate via REST
            serde_json::json!({"key": "RUNPOD_API_KEY", "value": self.api_key}),
            serde_json::json!({"key": "VOX_RUNPOD_SELF_TERMINATE", "value": "1"}),
            serde_json::json!({"key": "VOX_JOB_KIND", "value": spec.job_kind.as_str()}),
        ];
        if let Some(ref d) = spec.train_data_hf {
            env.push(serde_json::json!({"key": "VOX_TRAIN_DATA_HF", "value": d}));
        }
        if let Some(ref a) = spec.adapter_upload_hf {
            env.push(serde_json::json!({"key": "VOX_ADAPTER_UPLOAD_HF", "value": a}));
        }
        if spec.job_kind != JobKind::Train {
            env.push(
                serde_json::json!({"key": "VOX_SERVE_PORT", "value": spec.serve_port.to_string()}),
            );
        }
        if spec.persistent {
            env.push(serde_json::json!({"key": "VOX_PERSISTENT_NODE", "value": "1"}));
        }
        for (k, v) in &spec.extra_env {
            env.push(serde_json::json!({"key": k, "value": v}));
        }
        env
    }
}

#[async_trait::async_trait]
impl CloudProvider for RunPodClient {
    fn kind(&self) -> ProviderKind {
        ProviderKind::RunPod
    }

    async fn list_offers(&self, min_vram_mb: u64) -> anyhow::Result<Vec<GpuOffer>> {
        let ttl = Duration::from_secs(self.config.price_cache_ttl_secs);
        {
            let c = self.cache.read().await;
            if c.fetched_at.map_or(false, |t| t.elapsed() < ttl) {
                return Ok(c
                    .offers
                    .iter()
                    .filter(|o| o.vram_mb >= min_vram_mb)
                    .cloned()
                    .collect());
            }
        }
        let raw = self
            .http
            .get(format!("{BASE}/gpus"))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("RunPod GET /gpus")?;
        let resp = raw
            .error_for_status()
            .context("RunPod /gpus error status")?
            .json::<GpusResp>()
            .await
            .context("RunPod /gpus parse")?;

        let now = Instant::now();
        let offers: Vec<GpuOffer> = resp
            .gpus
            .unwrap_or_default()
            .into_iter()
            .filter_map(|g| self.map_gpu(g, now))
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
            anyhow::bail!(
                "RunPod offer {} is stale — re-query before dispatch",
                offer.offer_id
            );
        }

        let name = format!(
            "vox-{}-{}",
            spec.job_kind.as_str(),
            chrono::Utc::now().timestamp(),
        );

        let body = serde_json::json!({
            "gpuTypeId": offer.offer_id,
            "name": name,
            "imageName": spec.image_tag,
            "gpuCount": 1,
            "volumeInGb": self.config.volume_gb,
            "containerDiskInGb": self.config.disk_gb,
            "isSpot": true,
            "cloudType": self.config.runpod_cloud_type.as_str(),
            "env": self.build_env(spec),
            "startSsh": false,
            "startJupyter": false,
        });

        let raw = self
            .http
            .post(format!("{BASE}/pods"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("RunPod POST /pods")?;
        let resp = raw
            .error_for_status()
            .context("RunPod POST /pods error")?
            .json::<CreateResp>()
            .await
            .context("RunPod create parse")?;
        let pod_id = resp
            .id
            .ok_or_else(|| anyhow::anyhow!("RunPod: no pod id in response"))?;

        tracing::info!(
            "RunPod pod {} created ({}, job={:?}, ${:.3}/hr)",
            pod_id,
            offer.gpu_name,
            spec.job_kind,
            offer.price_per_hour_usd
        );

        Ok(JobHandle {
            provider: ProviderKind::RunPod,
            job_id: pod_id,
            started_at: std::time::SystemTime::now(),
            estimated_seconds: 0.0,
            price_per_hour_usd: offer.price_per_hour_usd,
            is_persistent: spec.persistent,
        })
    }

    async fn poll_status(&self, handle: &JobHandle) -> anyhow::Result<JobStatus> {
        let raw = self
            .http
            .get(format!("{BASE}/pods/{}", handle.job_id))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("RunPod GET /pods/{id}")?;
        let pod = raw.error_for_status()?.json::<PodResp>().await?;

        let gpu_util = pod
            .runtime
            .as_ref()
            .and_then(|r| r.gpus.as_ref())
            .and_then(|g| g.first())
            .and_then(|g| g.gpu_util_percent);

        Ok(match pod.desired_status.as_deref() {
            Some("RUNNING") => JobStatus::Running {
                progress_pct: None,
                gpu_util_pct: gpu_util,
            },
            Some("EXITED") | Some("STOPPED") => JobStatus::Completed {
                adapter_uploaded: false,
            },
            Some("FAILED") | Some("TERMINATED") => JobStatus::Terminated,
            _ => JobStatus::Pending,
        })
    }

    async fn terminate(&self, handle: &JobHandle) -> anyhow::Result<()> {
        // Best-effort stop first (graceful), then hard delete
        let _ = self
            .http
            .post(format!("{BASE}/pods/{}/stop", handle.job_id))
            .bearer_auth(&self.api_key)
            .send()
            .await;
        let raw = self
            .http
            .delete(format!("{BASE}/pods/{}", handle.job_id))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("RunPod DELETE /pods/{id}")?;
        raw.error_for_status().context("RunPod terminate error")?;
        tracing::info!("RunPod pod {} terminated.", handle.job_id);
        Ok(())
    }

    async fn get_serve_url(
        &self,
        handle: &JobHandle,
        serve_port: u16,
    ) -> anyhow::Result<Option<String>> {
        // RunPod exposes ports via proxy. The URL is deterministic once the pod is running.
        let status = self.poll_status(handle).await?;
        if matches!(status, JobStatus::Running { .. }) {
            Ok(Some(format!(
                "https://{}-{}.proxy.runpod.net",
                handle.job_id, serve_port
            )))
        } else {
            Ok(None)
        }
    }
}
