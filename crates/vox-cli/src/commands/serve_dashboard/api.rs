use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Dashboard API — provides REST endpoints for the Vox Dashboard UI.
/// Each endpoint reads from the CodeStore so the dashboard shows real data
/// instead of hardcoded mocks.
///
/// Endpoints:
///   GET  /api/workflows       → list workflow definitions
///   GET  /api/skills          → list skill-type artifacts
///   GET  /api/agents          → list agent definitions
///   GET  /api/snippets        → list/search code snippets
///   GET  /api/marketplace     → list published shared artifacts
///   GET  /api/feedback        → list LLM interactions + feedback
///   POST /api/agents          → create/update agent definition
///   POST /api/snippets        → save a code snippet
///   POST /api/feedback        → submit feedback for an interaction
/// Shared state for the dashboard API.
pub struct DashboardState {
    pub store: Arc<vox_pm::CodeStore>,
}

// ── Response types ──────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct WorkflowListResponse {
    pub workflows: Vec<WorkflowItem>,
}

#[derive(Debug, Serialize)]
pub struct WorkflowItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct SkillListResponse {
    pub skills: Vec<SkillItem>,
}

#[derive(Debug, Serialize)]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub downloads: i64,
    pub tags: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentListResponse {
    pub agents: Vec<AgentItem>,
}

#[derive(Debug, Serialize)]
pub struct AgentItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub tools: Option<String>,
    pub version: String,
    pub is_public: bool,
}

#[derive(Debug, Serialize)]
pub struct SnippetListResponse {
    pub snippets: Vec<SnippetItem>,
}

#[derive(Debug, Serialize)]
pub struct SnippetItem {
    pub id: i64,
    pub language: String,
    pub title: String,
    pub code: String,
    pub description: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceResponse {
    pub artifacts: Vec<MarketplaceItem>,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceItem {
    pub id: String,
    pub name: String,
    pub artifact_type: String,
    pub description: Option<String>,
    pub version: String,
    pub downloads: i64,
    pub avg_rating: f64,
    pub author_id: String,
}

#[derive(Debug, Serialize)]
pub struct FeedbackListResponse {
    pub training_pairs: Vec<FeedbackItem>,
}

#[derive(Debug, Serialize)]
pub struct FeedbackItem {
    pub prompt: String,
    pub response: String,
    pub rating: Option<i64>,
    pub feedback_type: String,
}

// ── Request types ───────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub tools: Option<String>,
    pub model_config: Option<String>,
    pub version: String,
    pub is_public: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveSnippetRequest {
    pub language: String,
    pub title: String,
    pub code: String,
    pub description: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitFeedbackRequest {
    pub interaction_id: i64,
    pub rating: Option<i64>,
    pub feedback_type: String,
    pub correction_text: Option<String>,
    pub preferred_response: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

// ── Handler functions ───────────────────────────────────

/// GET /api/workflows
pub async fn list_workflows(store: &vox_pm::CodeStore) -> Result<WorkflowListResponse, String> {
    let defs = store
        .list_workflow_defs()
        .await
        .map_err(|e| format!("Failed to list workflows: {e}"))?;

    Ok(WorkflowListResponse {
        workflows: defs
            .into_iter()
            .map(|d| WorkflowItem {
                id: d.id,
                name: d.name,
                description: d.description,
                version: d.version,
                status: d.status,
            })
            .collect(),
    })
}

/// GET /api/skills
pub async fn list_skills(store: &vox_pm::CodeStore) -> Result<SkillListResponse, String> {
    let artifacts = store
        .list_artifacts("skill")
        .await
        .map_err(|e| format!("Failed to list skills: {e}"))?;

    Ok(SkillListResponse {
        skills: artifacts
            .into_iter()
            .map(|a| SkillItem {
                id: a.id,
                name: a.name,
                description: a.description,
                version: a.version,
                downloads: a.downloads,
                tags: a.tags,
            })
            .collect(),
    })
}

/// GET /api/agents
pub async fn list_agents(store: &vox_pm::CodeStore) -> Result<AgentListResponse, String> {
    let agents = store
        .list_agents()
        .await
        .map_err(|e| format!("Failed to list agents: {e}"))?;

    Ok(AgentListResponse {
        agents: agents
            .into_iter()
            .map(|a| AgentItem {
                id: a.id,
                name: a.name,
                description: a.description,
                system_prompt: a.system_prompt,
                tools: a.tools,
                version: a.version,
                is_public: a.is_public,
            })
            .collect(),
    })
}

/// GET /api/snippets?q=&lt;query&gt;
pub async fn list_snippets(
    store: &vox_pm::CodeStore,
    query: Option<&str>,
) -> Result<SnippetListResponse, String> {
    let snippets = if let Some(q) = query {
        store
            .search_snippets(q, None)
            .await
            .map_err(|e| format!("Failed to search snippets: {e}"))?
    } else {
        // List recent snippets (search with empty gives all)
        store
            .search_snippets("", None)
            .await
            .map_err(|e| format!("Failed to list snippets: {e}"))?
    };

    Ok(SnippetListResponse {
        snippets: snippets
            .into_iter()
            .map(|s| SnippetItem {
                id: s.id,
                language: s.language,
                title: s.title,
                code: s.code,
                description: s.description,
                tags: s.tags,
            })
            .collect(),
    })
}

/// GET /api/marketplace?q=&lt;query&gt;
pub async fn list_marketplace(
    store: &vox_pm::CodeStore,
    query: Option<&str>,
) -> Result<MarketplaceResponse, String> {
    let artifacts = if let Some(q) = query {
        store
            .search_artifacts(q)
            .await
            .map_err(|e| format!("Failed to search marketplace: {e}"))?
    } else {
        // Get all published artifacts
        store
            .search_artifacts("")
            .await
            .map_err(|e| format!("Failed to list marketplace: {e}"))?
    };

    Ok(MarketplaceResponse {
        artifacts: artifacts
            .into_iter()
            .map(|a| MarketplaceItem {
                id: a.id,
                name: a.name,
                artifact_type: a.artifact_type,
                description: a.description,
                version: a.version,
                downloads: a.downloads,
                avg_rating: a.avg_rating,
                author_id: a.author_id,
            })
            .collect(),
    })
}

/// GET /api/feedback
pub async fn list_feedback(
    store: &vox_pm::CodeStore,
    limit: i64,
) -> Result<FeedbackListResponse, String> {
    let pairs = store
        .get_training_data(limit)
        .await
        .map_err(|e| format!("Failed to get feedback: {e}"))?;

    Ok(FeedbackListResponse {
        training_pairs: pairs
            .into_iter()
            .map(|p| FeedbackItem {
                prompt: p.prompt,
                response: p.response,
                rating: p.rating,
                feedback_type: p.feedback_type,
            })
            .collect(),
    })
}

/// POST /api/agents
pub async fn create_agent(
    store: &vox_pm::CodeStore,
    req: CreateAgentRequest,
) -> Result<(), String> {
    store
        .register_agent(
            &req.id,
            &req.name,
            req.description.as_deref(),
            req.system_prompt.as_deref(),
            req.tools.as_deref(),
            req.model_config.as_deref(),
            None, // author_id from session
            &req.version,
            req.is_public,
        )
        .await
        .map_err(|e| format!("Failed to create agent: {e}"))
}

/// POST /api/snippets
pub async fn save_snippet(
    store: &vox_pm::CodeStore,
    req: SaveSnippetRequest,
) -> Result<i64, String> {
    store
        .save_snippet(
            &req.language,
            &req.title,
            &req.code,
            req.description.as_deref(),
            req.tags.as_deref(),
            None, // author_id from session
            None, // source_file
            None, // embedding
        )
        .await
        .map_err(|e| format!("Failed to save snippet: {e}"))
}

/// POST /api/feedback
pub async fn submit_feedback(
    store: &vox_pm::CodeStore,
    req: SubmitFeedbackRequest,
) -> Result<i64, String> {
    store
        .submit_feedback(
            req.interaction_id,
            None, // user_id from session
            req.rating,
            &req.feedback_type,
            req.correction_text.as_deref(),
            req.preferred_response.as_deref(),
        )
        .await
        .map_err(|e| format!("Failed to submit feedback: {e}"))
}
