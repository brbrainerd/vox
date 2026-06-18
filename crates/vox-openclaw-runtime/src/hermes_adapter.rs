use crate::ArsSkill;
use crate::openclaw::OpenClawSkillSpec;
use crate::openclaw_adapter::OpenClawAdapterError;
use crate::openclaw_adapter::{AgentRuntimeAdapter, AgentRuntimeConfig};
use async_trait::async_trait;
use serde_json::Value;

fn resolve_tilde(path: &std::path::Path) -> std::path::PathBuf {
    let path_str = path.to_string_lossy();
    if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            let mut new_path = home;
            if path_str.len() > 1 {
                let remainder = &path_str[1..];
                let remainder = remainder.trim_start_matches('/').trim_start_matches('\\');
                new_path.push(remainder);
            }
            return new_path;
        }
    }
    path.to_path_buf()
}

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
        let mut paths_to_scan = Vec::new();

        if let Some(ref path) = self.cfg.local_skills_path {
            paths_to_scan.push(resolve_tilde(path));
        } else {
            // Workspace roots
            if let Ok(cwd) = std::env::current_dir() {
                paths_to_scan.push(cwd.join(".vox/skills"));
                paths_to_scan.push(cwd.join(".agents/skills"));
                paths_to_scan.push(cwd.join(".claude/skills"));
            }
            // Home directory roots
            if let Some(home) = dirs::home_dir() {
                paths_to_scan.push(home.join(".vox/skills"));
                paths_to_scan.push(home.join(".agents/skills"));
                paths_to_scan.push(home.join(".claude/skills"));
            }
            // Vendored assets skills
            if let Ok(cwd) = std::env::current_dir() {
                paths_to_scan.push(cwd.join("assets/skills"));
            }
        }

        let handle = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            let mut registered_skills = std::collections::HashSet::new();

            for path in paths_to_scan {
                if !path.exists() {
                    continue;
                }
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            let skill_md_path = entry.path().join("SKILL.md");
                            if skill_md_path.exists() {
                                let folder_name = entry.file_name().to_string_lossy().to_string();
                                let dedup_key = folder_name.to_lowercase();
                                if registered_skills.contains(&dedup_key) {
                                    continue;
                                }
                                registered_skills.insert(dedup_key);

                                let mut name = folder_name.clone();
                                let mut version = "0.1.0".to_string();
                                let mut description = Some("Hermes local skill".to_string());

                                if let Ok(content) = std::fs::read_to_string(&skill_md_path) {
                                    match vox_plugin_host::skill_parser::parse_skill_md(&content) {
                                        Ok(bundle) => {
                                            if !bundle.manifest.name.is_empty() {
                                                name = bundle.manifest.name;
                                            }
                                            version = bundle.manifest.version;
                                            if !bundle.manifest.description.is_empty() {
                                                description = Some(bundle.manifest.description);
                                            }
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                "Failed to parse skill manifest at {}: {:?}",
                                                skill_md_path.display(),
                                                e
                                            );
                                        }
                                    }
                                }

                                out.push(OpenClawSkillSpec {
                                    name,
                                    version,
                                    description,
                                });
                            }
                        }
                    }
                }
            }
            out
        });

        let out = handle
            .await
            .map_err(|e| OpenClawAdapterError::Other(format!("blocking task join failed: {e}")))?;
        Ok(out)
    }

    async fn import_skill(&mut self, _slug: &str) -> Result<ArsSkill, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other(
            "Importing remote skills not supported on Hermes local".to_string(),
        ))
    }

    async fn list_subscriptions(&mut self) -> Result<Value, OpenClawAdapterError> {
        Ok(serde_json::json!({}))
    }

    async fn subscribe_domain(&mut self, _domain: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other(
            "WebSocket subscriptions not supported on Hermes".to_string(),
        ))
    }

    async fn unsubscribe_domain(&mut self, _domain: &str) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other(
            "WebSocket subscriptions not supported on Hermes".to_string(),
        ))
    }

    async fn notify_domain(
        &mut self,
        _domain: &str,
        _message: &str,
    ) -> Result<Value, OpenClawAdapterError> {
        Err(OpenClawAdapterError::Other(
            "WebSocket notifications not supported on Hermes".to_string(),
        ))
    }

    async fn gateway_call(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, OpenClawAdapterError> {
        if method == "generate" || method == "chat" {
            let mut req = self
                .http
                .post(&format!(
                    "{}/chat/completions",
                    self.cfg.http_gateway_url.trim_end_matches('/')
                ))
                .json(&params);
            if let Some(ref token) = self.cfg.auth_token {
                req = req.bearer_auth(token);
            }
            let res = req
                .send()
                .await
                .map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            let res = res
                .error_for_status()
                .map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            let json = res
                .json::<Value>()
                .await
                .map_err(|e| OpenClawAdapterError::Other(e.to_string()))?;
            Ok(json)
        } else {
            Err(OpenClawAdapterError::Other(format!(
                "Method {} not supported by Hermes",
                method
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentProvider;

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

    #[test]
    fn test_tilde_resolution_expands_home() {
        let path = std::path::Path::new("~/test/path");
        let resolved = resolve_tilde(path);
        if let Some(home) = dirs::home_dir() {
            assert_eq!(resolved, home.join("test/path"));
        } else {
            assert_eq!(resolved, path);
        }
    }

    #[tokio::test]
    async fn test_hermes_skills_discovery_and_parsing() {
        let temp = tempfile::tempdir().unwrap();
        // Use a folder name different from the skill name in SKILL.md
        let skill_dir = temp.path().join("different-folder-name");
        std::fs::create_dir_all(&skill_dir).unwrap();

        let skill_md_content = r#"---
name = "mock-discovered-skill"
description = "A parsed mock skill for test"
[metadata]
"vox-id" = "vox.mock.discovered"
"vox-version" = "2.3.4"
"vox-author" = "vox-team"
"vox-category" = "refactor"
---
"#;
        std::fs::write(skill_dir.join("SKILL.md"), skill_md_content).unwrap();

        let mut adapter = DefaultHermesRuntimeAdapter::new(AgentRuntimeConfig {
            provider: AgentProvider::Hermes,
            http_gateway_url: "http://localhost:8642/v1".to_string(),
            ws_gateway_url: None,
            auth_token: None,
            local_skills_path: Some(temp.path().to_path_buf()),
        });

        let skills = adapter.list_remote_skills().await.unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "mock-discovered-skill"); // Uses name from SKILL.md frontmatter
        assert_eq!(skills[0].version, "2.3.4");
        assert_eq!(
            skills[0].description,
            Some("A parsed mock skill for test".to_string())
        );
    }
}
