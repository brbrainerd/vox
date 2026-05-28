//! HTTP client for the populi control plane ([`crate::transport`]).
//!
//! Base URL should include scheme and host, e.g. `http://127.0.0.1:9847` (no trailing slash).
//! When the control plane is protected with **`VOX_MESH_TOKEN`**, set the same value via
//! [`PopuliHttpClient::with_bearer`] (or [`PopuliHttpClient::with_env_token`]).

use std::time::Duration;

use crate::transport::{A2ADeliverRequest, DispatchRequest, DispatchResponse};
use crate::{NodeRecord, PopuliRegistryError, PopuliRegistryFile};

/// Call the populi HTTP API (join / list / heartbeat / leave).
#[derive(Debug, Clone)]
pub struct PopuliHttpClient {
    client: reqwest::Client,
    base: String,
    bearer: Option<String>,
}

impl PopuliHttpClient {
    /// New client; `base` is normalized (trailing `/` stripped). No `Authorization` header.
    #[must_use]
    pub fn new(base: impl Into<String>) -> Self {
        Self::new_with_timeout(base, Duration::from_secs(30))
    }

    /// Like [`Self::new`] with an explicit request timeout (federation / control plane).
    #[must_use]
    pub fn new_with_timeout(base: impl Into<String>, timeout: Duration) -> Self {
        let mut base = base.into();
        while base.ends_with('/') {
            base.pop();
        }
        let client = vox_http_client::client_builder()
            .timeout(timeout)
            .build()
            .expect("reqwest TLS stack must be available (platform TLS missing or misconfigured)");
        Self {
            client,
            base,
            bearer: None,
        }
    }

    /// Attach `Authorization: Bearer …` to every request (e.g. matches server **`VOX_MESH_TOKEN`**).
    #[must_use]
    pub fn with_bearer(mut self, token: impl Into<String>) -> Self {
        let t = token.into();
        self.bearer = if t.trim().is_empty() { None } else { Some(t) };
        self
    }

    /// If **`VOX_MESH_TOKEN`** is set and non-empty, same as [`Self::with_bearer`] with that value.
    #[must_use]
    pub fn with_env_token(self) -> Self {
        if let Some(token) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken)
            .expose()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            self.with_bearer(token.to_string())
        } else {
            self
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.bearer {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        }
    }

    async fn ensure_success_with_context(
        resp: reqwest::Response,
        context: &str,
    ) -> Result<reqwest::Response, PopuliRegistryError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body_suffix = match resp.text().await.ok().map(|s| s.trim().to_string()) {
            Some(body) if !body.is_empty() => {
                let mut clipped = body;
                if clipped.len() > 240 {
                    clipped.truncate(240);
                    clipped.push_str("...");
                }
                format!(": {clipped}")
            }
            _ => String::new(),
        };
        Err(PopuliRegistryError::HttpStatus {
            status: status.as_u16(),
            context: context.to_string(),
            body_suffix,
        })
    }

    /// `GET /v1/populi/nodes`
    pub async fn list_nodes(&self) -> Result<PopuliRegistryFile, PopuliRegistryError> {
        let url = format!("{}/v1/populi/nodes", self.base);
        let resp = self
            .auth(self.client.get(url))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "list_nodes")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }

    /// `POST /v1/populi/federation/announce`
    pub async fn federation_announce(
        &self,
        req: &crate::transport::FederationAnnounceRequest,
    ) -> Result<crate::transport::FederationDirectoryResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/federation/announce", self.base);
        let resp = self
            .auth(self.client.post(url).json(req))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "federation_announce")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }

    /// `POST /v1/populi/join`
    pub async fn join(&self, node: &NodeRecord) -> Result<NodeRecord, PopuliRegistryError> {
        let url = format!("{}/v1/populi/join", self.base);
        let resp = self
            .auth(self.client.post(url).json(node))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "join")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }

    /// `POST /v1/populi/a2a/deliver` — forward an A2A message to a remote node.
    pub async fn relay_a2a(&self, req: &A2ADeliverRequest) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/a2a/deliver", self.base);
        let resp = self
            .auth(self.client.post(url).json(req))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "a2a_deliver").await?;
        Ok(())
    }

    /// `POST /v1/populi/dispatch` — send a script to the control plane for remote execution.
    pub async fn dispatch(
        &self,
        req: &DispatchRequest,
    ) -> Result<DispatchResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/dispatch", self.base);
        let resp = self
            .auth(self.client.post(url).json(req))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "dispatch")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }

    /// `POST /v1/populi/worker/execute` — worker-side internal endpoint for executing dispatched scripts.
    pub async fn worker_execute(
        &self,
        req: &DispatchRequest,
    ) -> Result<DispatchResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/worker/execute", self.base);
        let resp = self
            .auth(self.client.post(url).json(req))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "worker_execute")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }
}
