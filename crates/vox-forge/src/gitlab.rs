//! GitLab forge implementation for `vox-forge`.
//!
//! Uses GitLab REST API v4. Works with gitlab.com and self-hosted GitLab CE/EE.
//! Authentication: personal access token or project access token.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ForgeError;
use crate::provider::GitForgeProvider;
use crate::types::{
    ChangeRequest, ChangeRequestId, ChangeRequestState, ChangeRequestStatus,
    ForgeRepoInfo, ForgeUser, Label, Review, ReviewState, WebhookEvent,
};

/// GitLab API base URL (gitlab.com). Override for self-hosted instances.
pub const GITLAB_API_BASE: &str = "https://gitlab.com/api/v4";

/// A GitLab forge provider.
#[derive(Debug, Clone)]
pub struct GitLabProvider {
    token: String,
    api_base: String,
    client: reqwest::Client,
}

impl GitLabProvider {
    /// Create a provider for gitlab.com.
    pub fn new(token: impl Into<String>) -> Result<Self, ForgeError> {
        Self::with_base(token, GITLAB_API_BASE)
    }

    /// Create with a custom API base (for self-hosted GitLab).
    pub fn with_base(token: impl Into<String>, api_base: &str) -> Result<Self, ForgeError> {
        let client = reqwest::Client::builder()
            .user_agent("vox-forge/0.1 (https://github.com/vox-lang/vox)")
            .build()
            .map_err(|e| ForgeError::Network(e.to_string()))?;
        Ok(Self {
            token: token.into(),
            api_base: api_base.trim_end_matches('/').to_string(),
            client,
        })
    }

    async fn get_json(&self, url: &str) -> Result<Value, ForgeError> {
        let resp = self
            .client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))?;

        let status = resp.status().as_u16();
        if status == 404 { return Err(ForgeError::NotFound { resource: url.to_string() }); }
        if status == 401 { return Err(ForgeError::Unauthorized { reason: "HTTP 401".into() }); }
        if status == 429 {
            return Err(ForgeError::RateLimited { retry_after_secs: 60 });
        }
        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(ForgeError::Http { status, message: msg });
        }
        resp.json::<Value>().await.map_err(|e| ForgeError::Network(e.to_string()))
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, ForgeError> {
        let resp = self
            .client
            .post(url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(ForgeError::Http { status, message: msg });
        }
        resp.json::<Value>().await.map_err(|e| ForgeError::Network(e.to_string()))
    }

    /// URL-encode `owner/repo` as `owner%2Frepo` for GitLab path params.
    fn encode_project_id(owner: &str, repo: &str) -> String {
        format!("{owner}%2F{repo}")
    }
}

fn parse_mr(v: &Value) -> Option<ChangeRequest> {
    let state_str = v["state"].as_str()?;
    let state = match state_str {
        "opened" => ChangeRequestState::Open,
        "merged" => ChangeRequestState::Merged,
        "closed" => ChangeRequestState::Closed,
        _ => ChangeRequestState::Closed,
    };
    let is_draft = v["draft"].as_bool().unwrap_or(false)
        || v["title"].as_str().map(|t| t.starts_with("Draft:") || t.starts_with("WIP:")).unwrap_or(false);

    Some(ChangeRequest {
        id: ChangeRequestId(v["id"].as_u64().unwrap_or(0)),
        number: v["iid"].as_u64().unwrap_or(0), // GitLab uses iid for project-scoped MR number
        title: v["title"].as_str().unwrap_or("").to_string(),
        body: v["description"].as_str().unwrap_or("").to_string(),
        source_branch: v["source_branch"].as_str().unwrap_or("").to_string(),
        target_branch: v["target_branch"].as_str().unwrap_or("").to_string(),
        state,
        status: ChangeRequestStatus::Unknown,
        author: v["author"]["username"].as_str().unwrap_or("").to_string(),
        assignees: v["assignees"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|a| a["username"].as_str().map(String::from))
            .collect(),
        labels: v["labels"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|l| l.as_str().map(|s| Label {
                name: s.to_string(),
                color: String::new(),
                description: None,
            }))
            .collect(),
        web_url: v["web_url"].as_str().unwrap_or("").to_string(),
        created_at: v["created_at"].as_str().unwrap_or("").to_string(),
        updated_at: v["updated_at"].as_str().unwrap_or("").to_string(),
        is_draft,
        mergeable: v["merge_status"].as_str().map(|s| s == "can_be_merged"),
    })
}

#[async_trait]
impl GitForgeProvider for GitLabProvider {
    fn name(&self) -> &str { "GitLab" }
    fn api_base_url(&self) -> &str { &self.api_base }

    async fn repo_info(&self, owner: &str, repo: &str) -> Result<ForgeRepoInfo, ForgeError> {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}", self.api_base);
        let v = self.get_json(&url).await?;
        Ok(ForgeRepoInfo {
            owner: v["namespace"]["path"].as_str().unwrap_or(owner).to_string(),
            name: v["path"].as_str().unwrap_or(repo).to_string(),
            full_name: v["path_with_namespace"].as_str().unwrap_or("").to_string(),
            clone_url: v["http_url_to_repo"].as_str().unwrap_or("").to_string(),
            ssh_url: v["ssh_url_to_repo"].as_str().map(String::from),
            default_branch: v["default_branch"].as_str().unwrap_or("main").to_string(),
            is_private: v["visibility"].as_str() == Some("private"),
            stars: v["star_count"].as_u64().unwrap_or(0),
            forks: v["forks_count"].as_u64().unwrap_or(0),
            open_issues: v["open_issues_count"].as_u64().unwrap_or(0),
            description: v["description"].as_str().filter(|s| !s.is_empty()).map(String::from),
            web_url: v["web_url"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn list_change_requests(
        &self, owner: &str, repo: &str,
        state: Option<ChangeRequestState>, limit: u32,
    ) -> Result<Vec<ChangeRequest>, ForgeError> {
        let pid = Self::encode_project_id(owner, repo);
        let state_param = match state {
            Some(ChangeRequestState::Open) | None => "opened",
            Some(ChangeRequestState::Closed) => "closed",
            Some(ChangeRequestState::Merged) => "merged",
            Some(ChangeRequestState::Draft) => "opened",
        };
        let url = format!(
            "{}/projects/{pid}/merge_requests?state={state_param}&per_page={}&order_by=updated_at&sort=desc",
            self.api_base, limit.min(100)
        );
        let arr = self.get_json(&url).await?;
        Ok(arr.as_array().unwrap_or(&vec![]).iter().filter_map(parse_mr).collect())
    }

    async fn get_change_request(&self, owner: &str, repo: &str, number: u64)
        -> Result<ChangeRequest, ForgeError>
    {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}/merge_requests/{number}", self.api_base);
        let v = self.get_json(&url).await?;
        parse_mr(&v).ok_or_else(|| ForgeError::NotFound { resource: format!("MR !{number}") })
    }

    async fn create_change_request(
        &self, owner: &str, repo: &str,
        title: &str, body: &str,
        source_branch: &str, target_branch: &str, draft: bool,
    ) -> Result<ChangeRequest, ForgeError> {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}/merge_requests", self.api_base);
        let draft_title = if draft { format!("Draft: {title}") } else { title.to_string() };
        let payload = serde_json::json!({
            "source_branch": source_branch,
            "target_branch": target_branch,
            "title": draft_title,
            "description": body,
        });
        let v = self.post_json(&url, &payload).await?;
        parse_mr(&v).ok_or_else(|| ForgeError::Http { status: 422, message: "Failed to parse MR".into() })
    }

    async fn update_change_request(
        &self, owner: &str, repo: &str, number: u64,
        title: Option<&str>, body: Option<&str>, state: Option<ChangeRequestState>,
    ) -> Result<ChangeRequest, ForgeError> {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}/merge_requests/{number}", self.api_base);
        let mut payload = serde_json::json!({});
        if let Some(t) = title { payload["title"] = t.into(); }
        if let Some(b) = body { payload["description"] = b.into(); }
        if let Some(s) = state {
            payload["state_event"] = match s {
                ChangeRequestState::Closed => "close".into(),
                ChangeRequestState::Open => "reopen".into(),
                _ => serde_json::Value::Null,
            };
        }
        let resp = self.client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&payload)
            .send().await
            .map_err(|e| ForgeError::Network(e.to_string()))?;
        let v: Value = resp.json().await.map_err(|e| ForgeError::Network(e.to_string()))?;
        parse_mr(&v).ok_or_else(|| ForgeError::Http { status: 422, message: "Failed to parse updated MR".into() })
    }

    async fn merge_change_request(
        &self, owner: &str, repo: &str, number: u64, _merge_message: Option<&str>,
    ) -> Result<String, ForgeError> {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}/merge_requests/{number}/merge", self.api_base);
        let v = self.post_json(&url, &serde_json::json!({})).await?;
        Ok(v["sha"].as_str().unwrap_or("").to_string())
    }

    async fn list_reviews(&self, owner: &str, repo: &str, number: u64)
        -> Result<Vec<Review>, ForgeError>
    {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}/merge_requests/{number}/approvals", self.api_base);
        let v = self.get_json(&url).await?;
        let reviews = v["approved_by"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|a| Review {
                reviewer: a["user"]["username"].as_str().unwrap_or("").to_string(),
                state: ReviewState::Approved,
                body: None,
                submitted_at: None,
            })
            .collect();
        Ok(reviews)
    }

    async fn add_labels(&self, owner: &str, repo: &str, number: u64, labels: &[String])
        -> Result<Vec<Label>, ForgeError>
    {
        let pid = Self::encode_project_id(owner, repo);
        let url = format!("{}/projects/{pid}/merge_requests/{number}", self.api_base);
        let payload = serde_json::json!({ "add_labels": labels.join(",") });
        let resp = self.client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&payload)
            .send().await
            .map_err(|e| ForgeError::Network(e.to_string()))?;
        let v: Value = resp.json().await.map_err(|e| ForgeError::Network(e.to_string()))?;
        Ok(v["labels"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|l| l.as_str().map(|s| Label { name: s.to_string(), color: String::new(), description: None }))
            .collect())
    }

    async fn current_user(&self) -> Result<ForgeUser, ForgeError> {
        let url = format!("{}/user", self.api_base);
        let v = self.get_json(&url).await?;
        Ok(ForgeUser {
            login: v["username"].as_str().unwrap_or("").to_string(),
            display_name: v["name"].as_str().map(String::from),
            email: v["email"].as_str().filter(|s| !s.is_empty()).map(String::from),
            avatar_url: v["avatar_url"].as_str().map(String::from),
            web_url: v["web_url"].as_str().unwrap_or("").to_string(),
            is_bot: v["bot"].as_bool().unwrap_or(false),
        })
    }

    fn parse_webhook(&self, event_type: &str, payload: &[u8]) -> Result<WebhookEvent, ForgeError> {
        let v: Value = serde_json::from_slice(payload)?;
        let kind = v["object_kind"].as_str().unwrap_or(event_type);
        let event = match kind {
            "push" => WebhookEvent::Push {
                branch: v["ref"].as_str()
                    .unwrap_or("")
                    .strip_prefix("refs/heads/")
                    .unwrap_or("")
                    .to_string(),
                commits: v["commits"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|c| c["id"].as_str().map(String::from))
                    .collect(),
                pusher: v["user_username"].as_str().unwrap_or("").to_string(),
            },
            "merge_request" => {
                let action = v["object_attributes"]["action"].as_str().unwrap_or("");
                let number = v["object_attributes"]["iid"].as_u64().unwrap_or(0);
                match action {
                    "open" | "reopen" => WebhookEvent::ChangeRequestOpened {
                        cr_number: number,
                        author: v["user"]["username"].as_str().unwrap_or("").to_string(),
                    },
                    "merge" => WebhookEvent::ChangeRequestMerged {
                        cr_number: number,
                        merged_by: v["user"]["username"].as_str().unwrap_or("").to_string(),
                    },
                    "close" => WebhookEvent::ChangeRequestClosed { cr_number: number },
                    _ => WebhookEvent::Unknown { event_type: format!("merge_request.{action}") },
                }
            },
            _ => WebhookEvent::Unknown { event_type: kind.to_string() },
        };
        Ok(event)
    }

    async fn health_check(&self) -> Result<Option<u32>, ForgeError> {
        let url = format!("{}/projects?per_page=1", self.api_base);
        self.get_json(&url).await?;
        Ok(None)
    }
}
