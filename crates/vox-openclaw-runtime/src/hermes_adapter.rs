use async_trait::async_trait;
use serde_json::Value;
use crate::openclaw_adapter::{AgentRuntimeAdapter, AgentRuntimeConfig, AgentProvider};
use crate::openclaw::OpenClawSkillSpec;
use crate::openclaw_adapter::OpenClawAdapterError;
use crate::ArsSkill;

pub struct DefaultHermesRuntimeAdapter {
    cfg: AgentRuntimeConfig,
    http: reqwest::Client,
}

impl DefaultHermesRuntimeAdapter {
    pub fn new(cfg: AgentRuntimeConfig) -> Self {
        Self {
            cfg,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AgentRuntimeAdapter for DefaultHermesRuntimeAdapter {
    async fn list_remote_skills(&mut self) -> Result<Vec<OpenClawSkillSpec>, OpenClawAdapterError> {
        let Some(ref path) = self.cfg.local_skills_path else {
            return Ok(Vec::new());
        };
        if !path.exists() {
            return Ok(Vec::new());
        }
        // Bare dir discovery (similar to external_skills)
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.path().join("SKILL.md").exists() {
                    out.push(OpenClawSkillSpec {
                        name: entry.file_name().to_string_lossy().to_string(),
                        version: "0.1.0".to_string(),
                        description: Some("Hermes local skill".to_string()),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn import_skill(&mut self, _slug: &str) -> Result<ArsSkill, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("Importing remote skills not supported on Hermes local".to_string()))
    }

    async fn list_subscriptions(&mut self) -> Result<Value, OpenClawAdapterError> {
        Ok(serde_json::json!({}))
    }

    async fn subscribe_domain(&mut self, _domain: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("WebSocket subscriptions not supported on Hermes".to_string()))
    }

    async fn unsubscribe_domain(&mut self, _domain: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("WebSocket subscriptions not supported on Hermes".to_string()))
    }

    async fn notify_domain(&mut self, _domain: &str, _message: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other("WebSocket notifications not supported on Hermes".to_string()))
    }

    async fn gateway_call(&mut self, method: &str, params: Value) -> Result<Value, OpenClawAdapterError> {
        if method == "generate" || method == "chat" {
            let res = self.http.post(&format!("{}/chat/completions", self.cfg.http_gateway_url.trim_end_matches('/')))
                .json(&params)
                .send()
                .await
                .map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            let json = res.json::<Value>().await.map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            Ok(json)
        } else {
            Err(OpenClawAdapterError::Other(format!("Method {} not supported by Hermes", method)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hermes_skills_empty_for_missing_dir() {
        let mut adapter = DefaultHermesRuntimeAdapter::new(AgentRuntimeConfig {
            provider: AgentProvider::Hermes,
            http_gateway_url: "http://localhost:8642/v1".to_string(),
            ws_gateway_url: None,
            auth_token: None,
            local_skills_path: Some(std::path::PathBuf::from("/nonexistent-dir-for-test-999")),
        });
        let skills = adapter.list_remote_skills().await.unwrap();
        assert!(skills.is_empty());
    }
}
