//! HTTP client for the populi control plane ([`crate::transport`]).
//!
//! Base URL should include scheme and host, e.g. `http://127.0.0.1:9847` (no trailing slash).
//! When the control plane is protected with **`VOX_MESH_TOKEN`**, set the same value via
//! [`PopuliHttpClient::with_bearer`] (or [`PopuliHttpClient::with_env_token`]).

use std::time::Duration;

use crate::transport::{
    A2AAckRequest, A2ADeliverRequest, A2AInboxRequest, A2AInboxResponse, A2ALeaseRenewRequest,
    A2AStoredMessage, AdminExecLeaseRevokeRequest, AdminMaintenanceRequest, AdminQuarantineRequest,
    DispatchRequest, DispatchResponse, LeaveRequest, MeshQueueStats, RemoteExecLeaseGrantRequest,
    RemoteExecLeaseGrantResponse, RemoteExecLeaseListResponse, RemoteExecLeaseReleaseRequest,
    RemoteExecLeaseRenewRequest,
};
use crate::{NodeRecord, PopuliRegistryError, PopuliRegistryFile};
use reqwest_middleware::ClientWithMiddleware;
use serde::Serialize;

fn populi_retry_transient_from_env() -> bool {
    match std::env::var("VOX_POPULI_HTTP_RETRY_TRANSIENT") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"),
        Err(_) => false,
    }
}

/// Iterator-style paging cursor for non-claimer A2A inbox reads.
#[derive(Debug, Clone)]
pub struct A2AInboxPager {
    receiver_agent_id: String,
    page_size: usize,
    before_message_id: Option<u64>,
    finished: bool,
}

impl A2AInboxPager {
    /// Create a pager for a receiver id with a bounded page size.
    #[must_use]
    pub fn new(receiver_agent_id: impl Into<String>, page_size: usize) -> Self {
        Self {
            receiver_agent_id: receiver_agent_id.into(),
            page_size: page_size.clamp(1, 256),
            before_message_id: None,
            finished: false,
        }
    }

    /// Fetch the next page (newest-first). Empty page means completion.
    pub async fn next_page(
        &mut self,
        client: &PopuliHttpClient,
    ) -> Result<Vec<A2AStoredMessage>, PopuliRegistryError> {
        if self.finished {
            return Ok(Vec::new());
        }
        let page = client
            .relay_a2a_inbox_limited(
                &self.receiver_agent_id,
                Some(self.page_size),
                self.before_message_id,
            )
            .await?;
        if page.messages.is_empty() {
            self.finished = true;
            return Ok(Vec::new());
        }
        self.before_message_id = page.messages.last().map(|m| m.id);
        Ok(page.messages)
    }
}

/// Call the populi HTTP API (join / list / heartbeat / leave).
#[derive(Clone)]
pub struct PopuliHttpClient {
    client: ClientWithMiddleware,
    base: String,
    bearer: Option<String>,
}

impl std::fmt::Debug for PopuliHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PopuliHttpClient")
            .field("base", &self.base)
            .field("bearer_set", &self.bearer.is_some())
            .finish_non_exhaustive()
    }
}

impl PopuliHttpClient {
    /// Hosted / BaaS control plane entrypoint: same as [`Self::new`], but documents org-scoped HTTPS
    /// bases (see `docs/src/adr/009-populi-hosted-baas.md`). **Never** embed secrets in the URL.
    #[must_use]
    pub fn for_hosted_control_plane(base: impl Into<String>) -> Self {
        Self::new(base)
    }

    /// Get mesh queue stats.
    pub async fn queue_stats(&self) -> Result<MeshQueueStats, PopuliRegistryError> {
        let mut req = self
            .client
            .get(format!("{}/v1/populi/queue/stats", self.base));
        if let Some(ref token) = self.bearer {
            req = req.bearer_auth(token);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Io(std::io::Error::other(e.to_string())))?;

        if !resp.status().is_success() {
            return Err(PopuliRegistryError::Io(std::io::Error::other(format!(
                "HTTP {}",
                resp.status()
            ))));
        }

        resp.json()
            .await
            .map_err(|e| PopuliRegistryError::Json(e.to_string()))
    }

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
        let inner = vox_http_client::client_builder()
            .timeout(timeout)
            .build()
            .expect("reqwest TLS stack must be available (platform TLS missing or misconfigured)");
        let client = vox_http_client::populi_control_plane_client(
            inner,
            populi_retry_transient_from_env(),
        );
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
        if let Some(token) = crate::http_auth::mesh_worker_plane_bearer_string() {
            self.with_bearer(token)
        } else {
            self
        }
    }

    /// Bearer for **`POST /v1/populi/a2a/deliver`**: first non-empty among mesh, submitter, then admin tokens.
    #[must_use]
    pub fn with_env_deliver_token(self) -> Self {
        if let Some(t) = crate::http_auth::deliver_bearer_string() {
            self.with_bearer(t)
        } else {
            self
        }
    }

    fn auth(&self, rb: reqwest_middleware::RequestBuilder) -> reqwest_middleware::RequestBuilder {
        match &self.bearer {
            Some(t) => rb.bearer_auth(t),
            None => rb,
        }
    }

    fn post_json<T: Serialize>(
        &self,
        url: &str,
        payload: &T,
    ) -> Result<reqwest_middleware::RequestBuilder, PopuliRegistryError> {
        let body =
            serde_json::to_vec(payload).map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(self
            .client
            .post(url.to_string())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body))
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

    /// `GET /v1/populi/federation/directory`
    pub async fn federation_directory(
        &self,
    ) -> Result<crate::transport::FederationDirectoryResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/federation/directory", self.base);
        let resp = self
            .auth(self.client.get(url))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "federation_directory")
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
            .auth(self.post_json(&url, req)?)
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
            .auth(self.post_json(&url, node)?)
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

    /// `POST /v1/populi/heartbeat`
    pub async fn heartbeat(&self, node: &NodeRecord) -> Result<NodeRecord, PopuliRegistryError> {
        let url = format!("{}/v1/populi/heartbeat", self.base);
        let resp = self
            .auth(self.post_json(&url, node)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "heartbeat")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }

    /// `POST /v1/populi/bootstrap/exchange` — exchange a one-time bootstrap token for the mesh bearer token.
    ///
    /// The endpoint is unauthenticated (no `Authorization` header); the bootstrap token itself is the credential.
    /// Returns the long-lived mesh bearer token and optional scope id.
    pub async fn bootstrap_exchange(
        &self,
        bootstrap_token: &str,
    ) -> Result<crate::transport::BootstrapExchangeResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/bootstrap/exchange", self.base);
        let req = crate::transport::BootstrapExchangeRequest {
            bootstrap_token: bootstrap_token.to_string(),
        };
        let resp = self
            .post_json(&url, &req)?
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "bootstrap_exchange")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }

    /// `POST /v1/populi/leave` — returns `true` if the node was present and removed.
    pub async fn leave(&self, node_id: &str) -> Result<bool, PopuliRegistryError> {
        let url = format!("{}/v1/populi/leave", self.base);
        let leave_req = LeaveRequest {
            id: node_id.to_string(),
        };
        let resp = self
            .auth(self.post_json(&url, &leave_req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        match resp.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(true),
            reqwest::StatusCode::NOT_FOUND => Ok(false),
            _ => Err(PopuliRegistryError::HttpStatus {
                status: resp.status().as_u16(),
                context: "leave".to_string(),
                body_suffix: String::new(),
            }),
        }
    }

    /// `POST /v1/populi/a2a/deliver` — forward an A2A message to a remote node.
    pub async fn relay_a2a(&self, req: &A2ADeliverRequest) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/a2a/deliver", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "a2a_deliver").await?;
        Ok(())
    }

    /// `POST /v1/populi/a2a/inbox` — fetch undelivered messages for a receiver id.
    pub async fn relay_a2a_inbox(
        &self,
        receiver_agent_id: &str,
    ) -> Result<A2AInboxResponse, PopuliRegistryError> {
        self.relay_a2a_inbox_limited(receiver_agent_id, None, None)
            .await
    }

    /// `POST /v1/populi/a2a/inbox` with optional server-side max row cap.
    pub async fn relay_a2a_inbox_limited(
        &self,
        receiver_agent_id: &str,
        max_messages: Option<usize>,
        before_message_id: Option<u64>,
    ) -> Result<A2AInboxResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/a2a/inbox", self.base);
        let req = A2AInboxRequest {
            receiver_agent_id: receiver_agent_id.to_string(),
            claimer_node_id: None,
            max_messages,
            before_message_id,
        };
        let resp = self
            .auth(self.post_json(&url, &req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "a2a_inbox")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))
    }

    /// Page through non-claimer inbox rows until empty (newest-to-oldest by `id`).
    pub async fn relay_a2a_inbox_all_paged(
        &self,
        receiver_agent_id: &str,
        page_size: usize,
    ) -> Result<Vec<A2AStoredMessage>, PopuliRegistryError> {
        let mut out = Vec::new();
        let mut pager = A2AInboxPager::new(receiver_agent_id, page_size);
        loop {
            let page = pager.next_page(self).await?;
            if page.is_empty() {
                break;
            }
            out.extend(page);
        }
        Ok(out)
    }

    /// `POST /v1/populi/a2a/ack` — acknowledge one delivered message.
    pub async fn relay_a2a_ack(
        &self,
        receiver_agent_id: &str,
        message_id: u64,
    ) -> Result<bool, PopuliRegistryError> {
        let url = format!("{}/v1/populi/a2a/ack", self.base);
        let ack_req = A2AAckRequest {
            receiver_agent_id: receiver_agent_id.to_string(),
            message_id,
        };
        let resp = self
            .auth(self.post_json(&url, &ack_req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        match resp.status() {
            reqwest::StatusCode::NO_CONTENT => Ok(true),
            reqwest::StatusCode::NOT_FOUND => Ok(false),
            _ => Err(PopuliRegistryError::HttpStatus {
                status: resp.status().as_u16(),
                context: "a2a_ack".to_string(),
                body_suffix: String::new(),
            }),
        }
    }

    /// `POST /v1/populi/exec/lease/grant` — acquire or refresh a remote execution lease for `scope_key`.
    pub async fn exec_lease_grant(
        &self,
        req: &RemoteExecLeaseGrantRequest,
    ) -> Result<RemoteExecLeaseGrantResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/exec/lease/grant", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "exec_lease_grant")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))
    }

    /// `GET /v1/populi/exec/leases` — mesh/admin bearer; non-expired rows after server sweep.
    pub async fn list_exec_leases(
        &self,
    ) -> Result<RemoteExecLeaseListResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/exec/leases", self.base);
        let resp = self
            .auth(self.client.get(url))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "exec_lease_list")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))
    }

    /// `POST /v1/populi/exec/lease/renew`.
    pub async fn exec_lease_renew(
        &self,
        req: &RemoteExecLeaseRenewRequest,
    ) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/exec/lease/renew", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "exec_lease_renew").await?;
        Ok(())
    }

    /// `POST /v1/populi/exec/lease/release`.
    pub async fn exec_lease_release(
        &self,
        req: &RemoteExecLeaseReleaseRequest,
    ) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/exec/lease/release", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "exec_lease_release").await?;
        Ok(())
    }

    /// `POST /v1/populi/a2a/lease-renew`.
    pub async fn relay_a2a_lease_renew(
        &self,
        req: &A2ALeaseRenewRequest,
    ) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/a2a/lease-renew", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "a2a_lease_renew").await?;
        Ok(())
    }

    /// `POST /v1/populi/admin/quarantine` — requires mesh/admin bearer.
    pub async fn admin_quarantine(
        &self,
        req: &AdminQuarantineRequest,
    ) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/admin/quarantine", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "admin_quarantine").await?;
        Ok(())
    }

    /// `POST /v1/populi/admin/maintenance` — requires mesh/admin bearer.
    pub async fn admin_maintenance(
        &self,
        req: &AdminMaintenanceRequest,
    ) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/admin/maintenance", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "admin_maintenance").await?;
        Ok(())
    }

    /// `POST /v1/populi/admin/exec-lease/revoke` — drop a lease row by id (mesh/admin bearer; no holder check).
    pub async fn admin_exec_lease_revoke(
        &self,
        req: &AdminExecLeaseRevokeRequest,
    ) -> Result<(), PopuliRegistryError> {
        let url = format!("{}/v1/populi/admin/exec-lease/revoke", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Self::ensure_success_with_context(resp, "admin_exec_lease_revoke").await?;
        Ok(())
    }

    /// `POST /v1/populi/dispatch` — send a script to the control plane for remote execution.
    pub async fn dispatch(
        &self,
        req: &DispatchRequest,
    ) -> Result<DispatchResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/dispatch", self.base);
        let resp = self
            .auth(self.post_json(&url, req)?)
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
            .auth(self.post_json(&url, req)?)
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

    /// `GET /v1/populi/dispatch/result/{id}` — poll for results of a detached execution (Wave 5).
    pub async fn dispatch_result_poll(
        &self,
        id: &str,
    ) -> Result<DispatchResponse, PopuliRegistryError> {
        let url = format!("{}/v1/populi/dispatch/result/{}", self.base, id);
        let resp = self
            .auth(self.client.get(url))
            .send()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        let v = Self::ensure_success_with_context(resp, "dispatch_result_poll")
            .await?
            .json()
            .await
            .map_err(|e| PopuliRegistryError::Http(e.to_string()))?;
        Ok(v)
    }
}
