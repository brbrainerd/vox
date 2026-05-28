//! Cloud STT offloading backend.
//!
//! When VRAM is exhausted locally, this backend dispatches an ephemeral
//! GPU instance via `CloudResolver`, sends the PCM audio payload for transcription,
//! and retrieves the result.

use anyhow::{Context, Result};
use reqwest::Client;
use std::sync::Arc;

use vox_populi::mens::cloud::{
    CloudJobSpec, CloudProviderConfig, CloudResolver, CloudTarget, JobKind, ResolveRequest,
};

use super::asr_backend::{AsrBackend, AsrOutput};

pub struct CloudOffloadBackend {
    http: Client,
}

impl CloudOffloadBackend {
    pub fn new() -> Self {
        Self {
            http: vox_http_client::client_builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("reqwest builder"),
        }
    }
}

#[async_trait::async_trait]
impl AsrBackend for CloudOffloadBackend {
    fn name(&self) -> &'static str {
        "cloud-offload"
    }

    fn transcribe_pcm(
        &self,
        _pcm: &[f32],
        _sample_rate: u32,
        _language: Option<&str>,
    ) -> Result<AsrOutput> {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            // 1. Resolve an offer
            let resolver = CloudResolver::new_from_env().await?;
            let req = ResolveRequest {
                min_vram_mb: 8000,
                seq_len: 256,
                batch_size: 1,
                num_samples: 1,
                epochs: 1,
                max_acceptable_cost: 1.0,
                target: CloudTarget::Auto,
            };

            let ranked = resolver.resolve(&req).await?;

            // 2. Dispatch job
            let config = Arc::new(CloudProviderConfig::default());
            let mut spec = CloudJobSpec::new_serve(&config, 300);
            spec.job_kind = JobKind::Agent; // Treat STT offload as Agent task

            let (_handle, join, provider) = resolver.dispatch_top(&ranked, &spec).await?;

            let is_local = matches!(
                _handle.provider,
                vox_populi::mens::cloud::ProviderKind::Local
            );
            let (poll_interval_secs, max_polls) = if is_local { (2, 10) } else { (5, 60) };

            // a) Wait for the pod to be "Running" and get its URL
            let mut serve_url = None;
            for _ in 0..max_polls {
                // poll for up to ~5 minutes
                match provider.get_serve_url(&_handle, spec.serve_port).await {
                    Ok(Some(url)) => {
                        serve_url = Some(url);
                        break;
                    }
                    _ => {
                        tokio::time::sleep(std::time::Duration::from_secs(poll_interval_secs)).await
                    }
                }
            }

            let url = serve_url
                .ok_or_else(|| anyhow::anyhow!("Cloud instance did not become ready in time"))?;

            // b) Send the PCM payload via HTTP
            let pcm_bytes: Vec<u8> = _pcm.iter().flat_map(|f| f.to_le_bytes()).collect();
            let part = reqwest::multipart::Part::bytes(pcm_bytes)
                .file_name("audio.raw")
                .mime_str("application/octet-stream")?;

            let mut form = reqwest::multipart::Form::new()
                .part("file", part)
                .text("sample_rate", _sample_rate.to_string());

            if let Some(lang) = _language {
                form = form.text("language", lang.to_string());
            }

            let (app_poll_interval, app_max_polls) = if is_local { (1, 30) } else { (5, 12) };

            // d) Receive the JSON AsrOutput (retry to allow container app to boot)
            let mut result = None;
            for _ in 0..app_max_polls {
                // wait ~1 min for container app to start up
                // We need to clone the form parts manually or just recreate them if it fails,
                // but since we only recreate it if the request fails, let's just recreate it inside the loop
                let pcm_bytes_clone: Vec<u8> = _pcm.iter().flat_map(|f| f.to_le_bytes()).collect();
                let part_clone = reqwest::multipart::Part::bytes(pcm_bytes_clone)
                    .file_name("audio.raw")
                    .mime_str("application/octet-stream")?;
                let mut form_clone = reqwest::multipart::Form::new()
                    .part("file", part_clone)
                    .text("sample_rate", _sample_rate.to_string());
                if let Some(lang) = _language {
                    form_clone = form_clone.text("language", lang.to_string());
                }

                let req = self
                    .http
                    .post(format!("{}/transcribe", url))
                    .multipart(form_clone)
                    .send()
                    .await;

                match req {
                    Ok(res) if res.status().is_success() => {
                        result = Some(res.json::<AsrOutput>().await?);
                        break;
                    }
                    _ => {
                        tokio::time::sleep(std::time::Duration::from_secs(app_poll_interval)).await
                    }
                }
            }

            // Terminate job upon completion or failure
            let _ = provider.terminate(&_handle).await;

            result.ok_or_else(|| anyhow::anyhow!("Failed to get transcription from cloud worker"))
        })
    }
}
