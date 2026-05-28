//! HTTP client for OpenClaw / ClawHub-compatible skill gateways.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::ArsSkill;
use vox_plugin_host::skill_parser::parse_skill_md;
use vox_plugin_host::skill_registry::{InstallResult, SkillRegistry};

/// Remote gateway configuration.
#[derive(Debug, Clone)]
pub struct OpenClawRemoteConfig {
    /// Base URL (no trailing slash required).
    pub gateway_url: String,
    /// Optional bearer token.
    pub auth_token: Option<String>,
    /// Whether to validate TLS certificates (passed to reqwest builder).
    pub verify_tls: bool,
}

/// Skill summary returned by [`OpenClawClient::list_skills`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawSkillSpec {
    /// Registry name / slug.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Wire-level errors.
#[derive(Debug, thiserror::Error)]
pub enum OpenClawError {
    /// HTTP / transport failure.
    #[error("OpenClaw HTTP: {0}")]
    Http(String),
    /// Unexpected JSON shape.
    #[error("OpenClaw payload: {0}")]
    Payload(String),
    /// Skill registry install failure.
    #[error("OpenClaw install: {0}")]
    Install(String),
    /// Gateway rejected a publish request.
    #[error("OpenClaw publish failed with status {status}: {body}")]
    PublishFailed { status: u16, body: String },
}

/// Result returned by [`OpenClawClient::publish_skill`].
#[derive(Debug, Clone, Deserialize)]
pub struct PublishResult {
    /// Gateway-assigned slug for the published skill.
    pub slug: String,
    /// Canonical URL of the published skill on the gateway.
    pub url: String,
    /// Optional revision / version string assigned by the gateway.
    #[serde(default)]
    pub revision: Option<String>,
}

/// Client for OpenClaw-compatible HTTP APIs.
pub struct OpenClawClient {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl OpenClawClient {
    /// Build a client from configuration.
    pub fn new(cfg: OpenClawRemoteConfig) -> Result<Self, OpenClawError> {
        let mut b =
            vox_http_client::client_builder().timeout(std::time::Duration::from_secs(60));
        if !cfg.verify_tls {
            b = b.danger_accept_invalid_certs(true);
        }
        let http = b.build().map_err(|e| OpenClawError::Http(e.to_string()))?;
        let base = cfg.gateway_url.trim_end_matches('/').to_string();
        Ok(Self {
            http,
            base,
            token: cfg.auth_token,
        })
    }

    fn bearer(&self) -> Option<&str> {
        self.token.as_deref()
    }

    fn get(&self, path: &str, token: Option<&str>) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base, path);
        let mut r = self
            .http
            .get(&url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            r = r.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        r
    }

    /// List published skills (`GET /v1/skills`).
    pub async fn list_skills(&self) -> Result<Vec<OpenClawSkillSpec>, OpenClawError> {
        let resp = self
            .get("/v1/skills", self.bearer())
            .send()
            .await
            .map_err(|e| OpenClawError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(OpenClawError::Http(format!(
                "list_skills HTTP {}",
                resp.status()
            )));
        }
        let v: Value = resp
            .json()
            .await
            .map_err(|e| OpenClawError::Payload(e.to_string()))?;
        parse_skill_list(&v)
    }

    /// Fetch a skill as [`ArsSkill`] metadata (`GET /v1/skills/{slug}`).
    pub async fn import_skill(&self, slug: &str) -> Result<ArsSkill, OpenClawError> {
        let path = format!("/v1/skills/{}", urlencoding_encode_path_segment(slug));
        let resp = self
            .get(&path, self.bearer())
            .send()
            .await
            .map_err(|e| OpenClawError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(OpenClawError::Http(format!(
                "import_skill HTTP {}",
                resp.status()
            )));
        }
        let v: Value = resp
            .json()
            .await
            .map_err(|e| OpenClawError::Payload(e.to_string()))?;
        ars_skill_from_gateway_value(&v, slug)
    }

    /// Download SKILL.md text and install into `registry`.
    pub async fn import_and_install(
        &self,
        slug: &str,
        registry: &SkillRegistry,
    ) -> Result<InstallResult, OpenClawError> {
        let md = self.fetch_skill_md(slug).await?;
        let bundle = parse_skill_md(&md).map_err(|e| OpenClawError::Install(e.to_string()))?;
        registry
            .install_bundle(&bundle)
            .await
            .map_err(|e| OpenClawError::Install(e.to_string()))
    }

    /// Publish a Vox skill to an OpenClaw gateway.
    ///
    /// `skill_md` should be the AgentSkills-compliant SKILL.md content (top-level
    /// `name` + `description`, Vox extensions under `metadata.vox-*`).
    ///
    /// Returns the gateway-assigned slug and canonical URL on success.
    pub async fn publish_skill(
        &self,
        skill_md: &str,
        spec: &OpenClawSkillSpec,
    ) -> Result<PublishResult, OpenClawError> {
        let url = format!("{}/v1/skills", self.base);
        let body = serde_json::json!({
            "name": spec.name,
            "version": spec.version,
            "description": spec.description,
            "skill_md": skill_md,
        });
        let mut req = self
            .http
            .post(&url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .json(&body);
        if let Some(t) = self.bearer() {
            req = req.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| OpenClawError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(OpenClawError::PublishFailed { status, body });
        }
        resp.json::<PublishResult>()
            .await
            .map_err(|e| OpenClawError::Payload(e.to_string()))
    }

    async fn fetch_skill_md(&self, slug: &str) -> Result<String, OpenClawError> {
        let path = format!(
            "/v1/skills/{}/skill.md",
            urlencoding_encode_path_segment(slug)
        );
        let resp = self
            .get(&path, self.bearer())
            .send()
            .await
            .map_err(|e| OpenClawError::Http(e.to_string()))?;
        if resp.status().is_success() {
            let ct = resp
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            if ct.contains("json") {
                let v: Value = resp
                    .json()
                    .await
                    .map_err(|e| OpenClawError::Payload(e.to_string()))?;
                return v
                    .get("skill_md")
                    .or_else(|| v.get("body"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| OpenClawError::Payload("missing skill_md in JSON".into()));
            }
            return resp
                .text()
                .await
                .map_err(|e| OpenClawError::Http(e.to_string()));
        }
        // Fallback: single JSON document with embedded skill_md.
        let path2 = format!("/v1/skills/{}", urlencoding_encode_path_segment(slug));
        let resp2 = self
            .get(&path2, self.bearer())
            .send()
            .await
            .map_err(|e| OpenClawError::Http(e.to_string()))?;
        if !resp2.status().is_success() {
            return Err(OpenClawError::Http(format!(
                "fetch skill.md HTTP {}",
                resp2.status()
            )));
        }
        let v: Value = resp2
            .json()
            .await
            .map_err(|e| OpenClawError::Payload(e.to_string()))?;
        v.get("skill_md")
            .or_else(|| v.get("body"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| OpenClawError::Payload("missing skill_md".into()))
    }
}

fn urlencoding_encode_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(char::from(*b));
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

fn parse_skill_list(v: &Value) -> Result<Vec<OpenClawSkillSpec>, OpenClawError> {
    let arr = v
        .get("skills")
        .and_then(|x| x.as_array())
        .or_else(|| v.as_array())
        .ok_or_else(|| OpenClawError::Payload("expected skills array".into()))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let name = item
            .get("name")
            .or_else(|| item.get("slug"))
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_string();
        let version = item
            .get("version")
            .and_then(|x| x.as_str())
            .unwrap_or("0.0.0")
            .to_string();
        let description = item
            .get("description")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        out.push(OpenClawSkillSpec {
            name,
            version,
            description,
        });
    }
    Ok(out)
}

fn ars_skill_from_gateway_value(v: &Value, slug_fallback: &str) -> Result<ArsSkill, OpenClawError> {
    let o = v
        .as_object()
        .or_else(|| v.get("skill").and_then(|x| x.as_object()))
        .ok_or_else(|| OpenClawError::Payload("expected skill object".into()))?;
    let id = o
        .get("id")
        .and_then(|x| x.as_str())
        .unwrap_or(slug_fallback)
        .to_string();
    let name = o
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or(&id)
        .to_string();
    let version = o
        .get("version")
        .and_then(|x| x.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    Ok(ArsSkill {
        id,
        namespace: o
            .get("namespace")
            .and_then(|x| x.as_str())
            .unwrap_or("openclaw")
            .to_string(),
        name,
        version,
        content_hash: o
            .get("content_hash")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        description: o
            .get("description")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        author: o
            .get("author")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        metadata: o
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default())),
        kind: crate::manifest::SkillKind::Document,
        body: o
            .get("skill_md")
            .or_else(|| o.get("body"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        resource_limits: crate::manifest::ResourceLimits::default(),
        trust: crate::manifest::TrustLevel::Community,
    })
}
