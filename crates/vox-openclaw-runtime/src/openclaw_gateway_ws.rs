//! OpenClaw Gateway WebSocket transport.
//!
//! This client implements the gateway `connect` handshake and generic method calls.

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use uuid::Uuid;

use crate::openclaw_protocol::{
    GatewayClientIdentity, GatewayConnectParams, GatewayRequest, InboundFrame,
};

/// Gateway WS configuration.
#[derive(Debug, Clone)]
pub struct OpenClawGatewayWsConfig {
    pub url: String,
    pub token: Option<String>,
    pub role: String,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub client_version: String,
    pub platform: String,
    pub mode: String,
    pub min_protocol: u32,
    pub max_protocol: u32,
}

impl Default for OpenClawGatewayWsConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:18789".to_string(),
            token: None,
            role: "operator".to_string(),
            scopes: vec!["operator.read".to_string(), "operator.write".to_string()],
            client_id: "vox-cli".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            mode: "operator".to_string(),
            min_protocol: 3,
            max_protocol: 3,
        }
    }
}

#[derive(Debug, Error)]
pub enum OpenClawGatewayWsError {
    #[error("OpenClaw WS connect failed: {0}")]
    Connect(String),
    #[error("OpenClaw WS protocol error: {0}")]
    Protocol(String),
    #[error("OpenClaw WS send failed: {0}")]
    Send(String),
    #[error("OpenClaw WS receive failed: {0}")]
    Receive(String),
    #[error("OpenClaw WS method call failed: {0}")]
    Method(String),
}

/// Connected Gateway WS session.
pub struct OpenClawGatewayWsClient {
    cfg: OpenClawGatewayWsConfig,
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl OpenClawGatewayWsClient {
    /// Connect and run the `connect` handshake.
    pub async fn connect(cfg: OpenClawGatewayWsConfig) -> Result<Self, OpenClawGatewayWsError> {
        let req = cfg
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| OpenClawGatewayWsError::Connect(e.to_string()))?;
        let (mut ws, _resp) = connect_async(req)
            .await
            .map_err(|e| OpenClawGatewayWsError::Connect(e.to_string()))?;

        // Best-effort consume initial challenge event if present.
        let maybe_first = ws.next().await;
        if let Some(Ok(Message::Text(text))) = maybe_first
            && let Ok(InboundFrame::Event { event, .. }) =
                serde_json::from_str::<InboundFrame>(&text)
            && event != "connect.challenge"
        {
            // Not a challenge event; keep processing path simple by ignoring it.
        }

        let connect_id = Uuid::new_v4().to_string();
        let params = GatewayConnectParams {
            min_protocol: cfg.min_protocol,
            max_protocol: cfg.max_protocol,
            client: GatewayClientIdentity {
                id: cfg.client_id.clone(),
                version: cfg.client_version.clone(),
                platform: cfg.platform.clone(),
                mode: cfg.mode.clone(),
            },
            role: cfg.role.clone(),
            scopes: cfg.scopes.clone(),
            caps: Vec::new(),
            commands: Vec::new(),
            permissions: json!({}),
            auth: cfg
                .token
                .as_ref()
                .map(|t| json!({ "token": t }))
                .unwrap_or_else(|| json!({})),
            user_agent: Some(format!("vox-openclaw/{}", env!("CARGO_PKG_VERSION"))),
            locale: Some("en-US".to_string()),
        };
        let frame = GatewayRequest::req(
            connect_id.clone(),
            "connect",
            serde_json::to_value(params).unwrap_or_else(|_| json!({})),
        );
        ws.send(Message::Text(
            (serde_json::to_string(&frame)
                .map_err(|e| OpenClawGatewayWsError::Send(e.to_string()))?)
            .into(),
        ))
        .await
        .map_err(|e| OpenClawGatewayWsError::Send(e.to_string()))?;

        loop {
            let msg = ws
                .next()
                .await
                .ok_or_else(|| OpenClawGatewayWsError::Receive("socket closed".to_string()))?
                .map_err(|e| OpenClawGatewayWsError::Receive(e.to_string()))?;
            if let Message::Text(text) = msg {
                let parsed = serde_json::from_str::<InboundFrame>(&text)
                    .map_err(|e| OpenClawGatewayWsError::Protocol(e.to_string()))?;
                if let InboundFrame::Response {
                    id,
                    ok,
                    payload,
                    error,
                    ..
                } = parsed
                    && id == connect_id
                {
                    if ok {
                        let _ = payload;
                        break;
                    }
                    return Err(OpenClawGatewayWsError::Protocol(format!(
                        "connect rejected: {}",
                        error
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    )));
                }
            }
        }

        Ok(Self { cfg, ws })
    }

    /// Reconnect using existing configuration.
    pub async fn reconnect(&mut self) -> Result<(), OpenClawGatewayWsError> {
        let fresh = Self::connect(self.cfg.clone()).await?;
        self.ws = fresh.ws;
        Ok(())
    }

    /// Call an arbitrary gateway method.
    pub async fn call_method(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, OpenClawGatewayWsError> {
        match self.call_method_inner(method, params.clone()).await {
            Ok(v) => Ok(v),
            Err(OpenClawGatewayWsError::Receive(_) | OpenClawGatewayWsError::Send(_)) => {
                self.reconnect().await?;
                self.call_method_inner(method, params).await
            }
            Err(err) => Err(err),
        }
    }

    async fn call_method_inner(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, OpenClawGatewayWsError> {
        let id = Uuid::new_v4().to_string();
        let frame = GatewayRequest::req(id.clone(), method.to_string(), params);
        self.ws
            .send(Message::Text(
                (serde_json::to_string(&frame)
                    .map_err(|e| OpenClawGatewayWsError::Send(e.to_string()))?)
                .into(),
            ))
            .await
            .map_err(|e| OpenClawGatewayWsError::Send(e.to_string()))?;

        loop {
            let msg = self
                .ws
                .next()
                .await
                .ok_or_else(|| OpenClawGatewayWsError::Receive("socket closed".to_string()))?
                .map_err(|e| OpenClawGatewayWsError::Receive(e.to_string()))?;
            if let Message::Text(text) = msg {
                let frame = serde_json::from_str::<InboundFrame>(&text)
                    .map_err(|e| OpenClawGatewayWsError::Protocol(e.to_string()))?;
                if let InboundFrame::Response {
                    id: rid,
                    ok,
                    payload,
                    error,
                } = frame
                {
                    if rid != id {
                        continue;
                    }
                    if ok {
                        return Ok(payload);
                    }
                    return Err(OpenClawGatewayWsError::Method(
                        error
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| format!("{method} failed")),
                    ));
                }
            }
        }
    }
}
