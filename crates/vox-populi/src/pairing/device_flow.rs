//! GitHub OAuth device-flow client. Read-only `gist` scope (P5-T2b).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct DeviceFlowConfig {
    pub client_id: String,
    pub github_login_base: String,
    pub github_api_base: String,
    pub scope: String,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceFlowInit {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceFlowToken {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceFlowError {
    #[error("http: {0}")]
    Http(String),
    #[error("github error: {0}")]
    GitHub(String),
    #[error("expired before authorization")]
    Expired,
}

#[derive(Debug, Clone)]
pub struct DeviceFlow {
    cfg: DeviceFlowConfig,
    client: reqwest::Client,
}

impl DeviceFlow {
    pub fn new(cfg: DeviceFlowConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("vox-populi-pairing/1")
            .build()
            .expect("reqwest client");
        Self { cfg, client }
    }

    pub async fn start(&self) -> Result<DeviceFlowInit, DeviceFlowError> {
        let url = format!("{}/login/device/code", self.cfg.github_login_base);
        let body = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.cfg.client_id.as_str()),
                ("scope", self.cfg.scope.as_str()),
            ])
            .send()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| DeviceFlowError::GitHub(e.to_string()))?
            .json::<DeviceFlowInit>()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
        Ok(body)
    }

    pub async fn poll_until_token(&self, init: &DeviceFlowInit) -> Result<String, DeviceFlowError> {
        let url = format!("{}/login/oauth/access_token", self.cfg.github_login_base);
        let started = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(init.expires_in);
        loop {
            if started.elapsed() > timeout {
                return Err(DeviceFlowError::Expired);
            }
            tokio::time::sleep(std::time::Duration::from_millis(self.cfg.poll_interval_ms)).await;
            let resp = self
                .client
                .post(&url)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", self.cfg.client_id.as_str()),
                    ("device_code", init.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
            if !resp.status().is_success() {
                continue;
            }
            let body = resp
                .text()
                .await
                .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
            if let Ok(token) = serde_json::from_str::<DeviceFlowToken>(&body) {
                return Ok(token.access_token);
            }
        }
    }

    pub async fn publish_gist(
        &self,
        access_token: &str,
        manifest_json: &str,
    ) -> Result<String, DeviceFlowError> {
        #[derive(Serialize)]
        struct GistFile<'a> {
            content: &'a str,
        }
        #[derive(Serialize)]
        struct GistBody<'a> {
            description: &'a str,
            public: bool,
            files: std::collections::HashMap<&'a str, GistFile<'a>>,
        }
        let mut files = std::collections::HashMap::new();
        files.insert(
            "vox-attestation.json",
            GistFile {
                content: manifest_json,
            },
        );
        let body = GistBody {
            description: "Vox mesh node attestation manifest (auto-generated)",
            public: true,
            files,
        };
        let url = format!("{}/gists", self.cfg.github_api_base);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?
            .error_for_status()
            .map_err(|e| DeviceFlowError::GitHub(e.to_string()))?;
        let v = resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| DeviceFlowError::Http(e.to_string()))?;
        let raw_url = v
            .pointer("/files/vox-attestation.json/raw_url")
            .and_then(|x| x.as_str())
            .ok_or_else(|| DeviceFlowError::GitHub("missing raw_url".into()))?
            .to_string();
        Ok(raw_url)
    }
}
