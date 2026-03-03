//! GitHub forge implementation for `vox-forge`.
//!
//! Uses GitHub REST API v3 + reqwest (rustls-tls, no C).
//! Authentication: personal access token or GitHub App JWT.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ForgeError;
use crate::provider::GitForgeProvider;
use crate::types::{
    ChangeRequest, ChangeRequestId, ChangeRequestState, ChangeRequestStatus,
    ForgeRepoInfo, ForgeUser, Label, Review, ReviewState, WebhookEvent,
};

/// GitHub API base URL (public cloud). Override for GitHub Enterprise.
pub const GITHUB_API_BASE: &str = "https://api.github.com";

/// A GitHub forge provider.
#[derive(Debug, Clone)]
pub struct GitHubProvider {
    /// Personal access token or GitHub App installation token.
    token: String,
    /// API base URL (allows pointing at GitHub Enterprise endpoints).
    api_base: String,
    /// HTTP client (reqwest with rustls).
    client: reqwest::Client,
}

impl GitHubProvider {
    /// Create a new GitHub provider with the given PAT or App token.
    pub fn new(token: impl Into<String>) -> Result<Self, ForgeError> {
        Self::with_base(token, GITHUB_API_BASE)
    }

    /// Create with a custom API base (for GitHub Enterprise).
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
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))?;

        let status = resp.status().as_u16();
        if status == 404 {
            return Err(ForgeError::NotFound { resource: url.to_string() });
        }
        if status == 401 || status == 403 {
            return Err(ForgeError::Unauthorized { reason: format!("HTTP {status}") });
        }
        if status == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(ForgeError::RateLimited { retry_after_secs: retry });
        }
        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(ForgeError::Http { status, message: msg });
        }
        resp.json::<Value>()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value, ForgeError> {
        let resp = self
            .client
            .post(url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(ForgeError::Http { status, message: msg });
        }
        resp.json::<Value>()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))
    }
}

// Helpers to parse GitHub JSON into Vox types.
fn parse_cr(v: &Value) -> Option<ChangeRequest> {
    let state_str = v["state"].as_str()?;
    let is_draft = v["draft"].as_bool().unwrap_or(false);
    let merged = v["merged"].as_bool().unwrap_or(false);
    let state = if merged {
        ChangeRequestState::Merged
    } else if is_draft && state_str == "open" {
        ChangeRequestState::Draft
    } else if state_str == "open" {
        ChangeRequestState::Open
    } else {
        ChangeRequestState::Closed
    };

    Some(ChangeRequest {
        id: ChangeRequestId(v["id"].as_u64().unwrap_or(0)),
        number: v["number"].as_u64().unwrap_or(0),
        title: v["title"].as_str().unwrap_or("").to_string(),
        body: v["body"].as_str().unwrap_or("").to_string(),
        source_branch: v["head"]["ref"].as_str().unwrap_or("").to_string(),
        target_branch: v["base"]["ref"].as_str().unwrap_or("").to_string(),
        state,
        status: ChangeRequestStatus::Unknown,
        author: v["user"]["login"].as_str().unwrap_or("").to_string(),
        assignees: v["assignees"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|a| a["login"].as_str().map(String::from))
            .collect(),
        labels: v["labels"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|l| parse_label(l))
            .collect(),
        web_url: v["html_url"].as_str().unwrap_or("").to_string(),
        created_at: v["created_at"].as_str().unwrap_or("").to_string(),
        updated_at: v["updated_at"].as_str().unwrap_or("").to_string(),
        is_draft,
        mergeable: v["mergeable"].as_bool(),
    })
}

fn parse_label(v: &Value) -> Option<Label> {
    Some(Label {
        name: v["name"].as_str()?.to_string(),
        color: v["color"].as_str().unwrap_or("").to_string(),
        description: v["description"].as_str().map(String::from),
    })
}

fn parse_review(v: &Value) -> Option<Review> {
    let state = match v["state"].as_str()? {
        "APPROVED" => ReviewState::Approved,
        "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
        "COMMENTED" => ReviewState::Commented,
        "DISMISSED" => ReviewState::Dismissed,
        _ => ReviewState::Pending,
    };
    Some(Review {
        reviewer: v["user"]["login"].as_str().unwrap_or("").to_string(),
        state,
        body: v["body"].as_str().filter(|s| !s.is_empty()).map(String::from),
        submitted_at: v["submitted_at"].as_str().map(String::from),
    })
}

#[async_trait]
impl GitForgeProvider for GitHubProvider {
    fn name(&self) -> &str { "GitHub" }
    fn api_base_url(&self) -> &str { &self.api_base }

    async fn repo_info(&self, owner: &str, repo: &str) -> Result<ForgeRepoInfo, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}", self.api_base);
        let v = self.get_json(&url).await?;
        Ok(ForgeRepoInfo {
            owner: v["owner"]["login"].as_str().unwrap_or(owner).to_string(),
            name: v["name"].as_str().unwrap_or(repo).to_string(),
            full_name: v["full_name"].as_str().unwrap_or("").to_string(),
            clone_url: v["clone_url"].as_str().unwrap_or("").to_string(),
            ssh_url: v["ssh_url"].as_str().map(String::from),
            default_branch: v["default_branch"].as_str().unwrap_or("main").to_string(),
            is_private: v["private"].as_bool().unwrap_or(false),
            stars: v["stargazers_count"].as_u64().unwrap_or(0),
            forks: v["forks_count"].as_u64().unwrap_or(0),
            open_issues: v["open_issues_count"].as_u64().unwrap_or(0),
            description: v["description"].as_str().filter(|s| !s.is_empty()).map(String::from),
            web_url: v["html_url"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn list_change_requests(
        &self, owner: &str, repo: &str,
        state: Option<ChangeRequestState>, limit: u32,
    ) -> Result<Vec<ChangeRequest>, ForgeError> {
        let state_param = match state {
            Some(ChangeRequestState::Open) | None => "open",
            Some(ChangeRequestState::Closed) => "closed",
            Some(ChangeRequestState::Merged) => "closed", // GitHub uses "closed" for merged
            Some(ChangeRequestState::Draft) => "open",
        };
        let url = format!(
            "{}/repos/{owner}/{repo}/pulls?state={state_param}&per_page={}&sort=updated&direction=desc",
            self.api_base, limit.min(100)
        );
        let arr = self.get_json(&url).await?;
        Ok(arr.as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(parse_cr)
            .collect())
    }

    async fn get_change_request(
        &self, owner: &str, repo: &str, number: u64,
    ) -> Result<ChangeRequest, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}/pulls/{number}", self.api_base);
        let v = self.get_json(&url).await?;
        parse_cr(&v).ok_or_else(|| ForgeError::Parse(
            serde_json::from_str::<serde_json::Value>("{}").unwrap_err()
        ))
    }

    async fn create_change_request(
        &self, owner: &str, repo: &str,
        title: &str, body: &str,
        source_branch: &str, target_branch: &str, draft: bool,
    ) -> Result<ChangeRequest, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}/pulls", self.api_base);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "head": source_branch,
            "base": target_branch,
            "draft": draft,
        });
        let v = self.post_json(&url, &payload).await?;
        parse_cr(&v).ok_or_else(|| ForgeError::Http {
            status: 422,
            message: "Failed to parse created PR".into(),
        })
    }

    async fn update_change_request(
        &self, owner: &str, repo: &str, number: u64,
        title: Option<&str>, body: Option<&str>,
        state: Option<ChangeRequestState>,
    ) -> Result<ChangeRequest, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}/pulls/{number}", self.api_base);
        let mut payload = serde_json::json!({});
        if let Some(t) = title { payload["title"] = t.into(); }
        if let Some(b) = body { payload["body"] = b.into(); }
        if let Some(s) = state {
            payload["state"] = match s {
                ChangeRequestState::Closed => "closed".into(),
                _ => "open".into(),
            };
        }
        let resp = self.client
            .patch(&url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ForgeError::Network(e.to_string()))?;
        let v: Value = resp.json().await.map_err(|e| ForgeError::Network(e.to_string()))?;
        parse_cr(&v).ok_or_else(|| ForgeError::Http {
            status: 422, message: "Failed to parse updated PR".into(),
        })
    }

    async fn merge_change_request(
        &self, owner: &str, repo: &str, number: u64,
        merge_message: Option<&str>,
    ) -> Result<String, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}/pulls/{number}/merge", self.api_base);
        let mut payload = serde_json::json!({ "merge_method": "squash" });
        if let Some(msg) = merge_message {
            payload["commit_message"] = msg.into();
        }
        let v = self.post_json(&url, &payload).await?;
        Ok(v["sha"].as_str().unwrap_or("").to_string())
    }

    async fn list_reviews(
        &self, owner: &str, repo: &str, number: u64,
    ) -> Result<Vec<Review>, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}/pulls/{number}/reviews", self.api_base);
        let arr = self.get_json(&url).await?;
        Ok(arr.as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(parse_review)
            .collect())
    }

    async fn add_labels(
        &self, owner: &str, repo: &str, number: u64, labels: &[String],
    ) -> Result<Vec<Label>, ForgeError> {
        let url = format!("{}/repos/{owner}/{repo}/issues/{number}/labels", self.api_base);
        let payload = serde_json::json!({ "labels": labels });
        let arr = self.post_json(&url, &payload).await?;
        Ok(arr.as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(parse_label)
            .collect())
    }

    async fn current_user(&self) -> Result<ForgeUser, ForgeError> {
        let url = format!("{}/user", self.api_base);
        let v = self.get_json(&url).await?;
        Ok(ForgeUser {
            login: v["login"].as_str().unwrap_or("").to_string(),
            display_name: v["name"].as_str().map(String::from),
            email: v["email"].as_str().filter(|s| !s.is_empty()).map(String::from),
            avatar_url: v["avatar_url"].as_str().map(String::from),
            web_url: v["html_url"].as_str().unwrap_or("").to_string(),
            is_bot: v["type"].as_str() == Some("Bot"),
        })
    }

    fn parse_webhook(
        &self, event_type: &str, payload: &[u8],
    ) -> Result<WebhookEvent, ForgeError> {
        let v: Value = serde_json::from_slice(payload)?;
        let event = match event_type {
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
                pusher: v["pusher"]["name"].as_str().unwrap_or("").to_string(),
            },
            "pull_request" => {
                let action = v["action"].as_str().unwrap_or("");
                let number = v["number"].as_u64().unwrap_or(0);
                let author = v["pull_request"]["user"]["login"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                match action {
                    "opened" | "reopened" => WebhookEvent::ChangeRequestOpened {
                        cr_number: number, author,
                    },
                    "closed" if v["pull_request"]["merged"].as_bool().unwrap_or(false) =>
                        WebhookEvent::ChangeRequestMerged {
                            cr_number: number,
                            merged_by: v["pull_request"]["merged_by"]["login"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                        },
                    "closed" => WebhookEvent::ChangeRequestClosed { cr_number: number },
                    _ => WebhookEvent::Unknown { event_type: format!("pull_request.{action}") },
                }
            },
            "pull_request_review" => {
                let number = v["pull_request"]["number"].as_u64().unwrap_or(0);
                let reviewer = v["review"]["user"]["login"].as_str().unwrap_or("").to_string();
                let state = match v["review"]["state"].as_str().unwrap_or("") {
                    "approved" => ReviewState::Approved,
                    "changes_requested" => ReviewState::ChangesRequested,
                    "dismissed" => ReviewState::Dismissed,
                    _ => ReviewState::Commented,
                };
                WebhookEvent::ReviewSubmitted { cr_number: number, reviewer, state }
            },
            "check_run" => {
                let conclusion = v["check_run"]["conclusion"].as_str().unwrap_or("unknown");
                let status = match conclusion {
                    "success" => ChangeRequestStatus::Success,
                    "failure" | "timed_out" => ChangeRequestStatus::Failure,
                    _ => ChangeRequestStatus::Unknown,
                };
                WebhookEvent::CheckCompleted {
                    cr_number: None,
                    name: v["check_run"]["name"].as_str().unwrap_or("").to_string(),
                    status,
                }
            },
            _ => WebhookEvent::Unknown { event_type: event_type.to_string() },
        };
        Ok(event)
    }

    async fn health_check(&self) -> Result<Option<u32>, ForgeError> {
        let url = format!("{}/rate_limit", self.api_base);
        let v = self.get_json(&url).await?;
        let remaining = v["rate"]["remaining"].as_u64().map(|n| n as u32);
        Ok(remaining)
    }
}
