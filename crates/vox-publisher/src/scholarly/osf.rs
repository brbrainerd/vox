//! OSF (Open Science Framework) v2 REST adapter.
//!
//! Creates a node (preprint project) via `POST /v2/nodes/` and returns the node id.
//! Uses Bearer token from `VoxOsfApiToken`. The actual file upload and metadata
//! annotation happen in Phase 9 operator flow.

use std::time::Duration;

use async_trait::async_trait;

use super::error::classify_scholarly_http;
use super::flags;
use super::{ScholarlyAdapter, ScholarlyError, ScholarlyRemoteStatus, ScholarlySubmissionReceipt};
use crate::publication::PublicationManifest;

const OSF_API_PRODUCTION: &str = "https://api.osf.io/v2";

#[derive(Debug, Clone)]
pub struct OsfAdapter {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl OsfAdapter {
    pub fn new(base: String, token: String) -> Self {
        let http = vox_http_client::client_builder()
            .user_agent("vox-publisher/osf")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("osf http client");
        Self { base, token, http }
    }
}

pub(super) fn osf_from_secrets() -> Result<OsfAdapter, ScholarlyError> {
    let token = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOsfApiToken)
        .expose()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ScholarlyError::Config {
            message: "VoxOsfApiToken is not set".into(),
        })?;
    let base = OSF_API_PRODUCTION.to_string();
    Ok(OsfAdapter::new(base, token))
}

pub(crate) fn osf_node_create_body(title: &str, category: &str) -> String {
    serde_json::json!({
        "data": {
            "type": "nodes",
            "attributes": {
                "title": title,
                "category": category,
                "public": false,
                "description": "Vox SCIENTIA publication node"
            }
        }
    })
    .to_string()
}

#[async_trait]
impl ScholarlyAdapter for OsfAdapter {
    fn adapter_name(&self) -> &'static str {
        "osf"
    }

    async fn submit(
        &self,
        manifest: &PublicationManifest,
    ) -> Result<ScholarlySubmissionReceipt, ScholarlyError> {
        if flags::scholarly_live_globally_disabled() {
            return Err(ScholarlyError::Disabled {
                reason: "VOX_SCHOLARLY_DISABLE_LIVE is set".into(),
            });
        }
        let body = osf_node_create_body(&manifest.title, "paper");
        let url = format!("{}/nodes/", self.base.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "application/vnd.api+json")
            .bearer_auth(&self.token)
            .body(body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
                code: "osf_json_parse".into(),
                message: format!("invalid JSON from OSF: {e}"),
            })?;
        let node_id = json
            .pointer("/data/id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let osf_node_url = format!("https://osf.io/{}", node_id);
        Ok(ScholarlySubmissionReceipt {
            adapter: self.adapter_name().to_string(),
            external_submission_id: node_id,
            status: "created".to_string(),
            response_fingerprint: Some(manifest.content_sha3_256()),
            metadata_json: Some(serde_json::json!({ "osf_node_url": osf_node_url }).to_string()),
        })
    }

    async fn fetch_status(
        &self,
        external_submission_id: &str,
    ) -> Result<ScholarlyRemoteStatus, ScholarlyError> {
        let url = format!(
            "{}/nodes/{}/",
            self.base.trim_end_matches('/'),
            external_submission_id
        );
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        Ok(ScholarlyRemoteStatus {
            status: "active".to_string(),
            detail_json: Some(text),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osf_node_create_body_is_valid_json() {
        let body = osf_node_create_body("Provider Atlas 2026", "paper");
        let v: serde_json::Value = serde_json::from_str(&body).expect("valid json");
        assert_eq!(v["data"]["type"], "nodes");
        assert_eq!(v["data"]["attributes"]["title"], "Provider Atlas 2026");
        assert_eq!(v["data"]["attributes"]["category"], "paper");
    }

    #[test]
    fn osf_adapter_name_is_osf() {
        let adapter = OsfAdapter::new("https://api.osf.io/v2".into(), "tok".into());
        assert_eq!(adapter.adapter_name(), "osf");
    }
}
