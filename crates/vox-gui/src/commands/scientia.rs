//! Typed Scientia-domain read commands (research sessions + publication manifests).
//! Reads go directly to the canonical DB, mirroring the CLI handlers — no CLI
//! stdout parsing and no dependency on the (disabled) HTTP gateway.

#[derive(Debug, serde::Serialize)]
pub struct ResearchSessionDto {
    pub id: i64,
    pub status: String,
    pub query_text: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ResearchDetailDto {
    pub session: ResearchSessionDto,
    pub report_markdown: Option<String>,
    pub artifact_json: Option<String>,
}

async fn connect_canonical_db() -> Result<vox_db::VoxDb, String> {
    vox_db::VoxDb::connect_canonical()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_research_sessions(limit: Option<u32>) -> Result<Vec<ResearchSessionDto>, String> {
    let db = connect_canonical_db().await?;
    let rows = db
        .list_recent_research_sessions(limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| ResearchSessionDto {
            id: r.id,
            status: r.status.clone(),
            query_text: r.query_text.clone(),
            started_at_ms: r.started_at_ms,
            finished_at_ms: r.finished_at_ms,
        })
        .collect())
}

#[tauri::command]
pub async fn get_research_session_detail(session_id: i64) -> Result<ResearchDetailDto, String> {
    let db = connect_canonical_db().await?;
    let s = db
        .get_research_session(session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("research session {session_id} not found"))?;
    let artifact = db
        .get_research_artifact(session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ResearchDetailDto {
        session: ResearchSessionDto {
            id: s.id,
            status: s.status.clone(),
            query_text: s.query_text.clone(),
            started_at_ms: s.started_at_ms,
            finished_at_ms: s.finished_at_ms,
        },
        report_markdown: artifact.as_ref().map(|a| a.report_markdown.clone()),
        artifact_json: artifact.as_ref().map(|a| a.artifact_json.clone()),
    })
}

#[derive(Debug, serde::Serialize)]
pub struct PublicationManifestDto {
    pub publication_id: String,
    pub content_type: String,
    pub state: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[tauri::command]
pub async fn list_publication_manifests(
    limit: Option<u32>,
) -> Result<Vec<PublicationManifestDto>, String> {
    let db = vox_db::VoxDb::connect_default()
        .await
        .map_err(|e| e.to_string())?;
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, limit.unwrap_or(200) as i64)
        .await
        .map_err(|e| e.to_string())?;
    Ok(manifests
        .iter()
        .map(|m| PublicationManifestDto {
            publication_id: m.publication_id.clone(),
            content_type: m.content_type.clone(),
            state: m.state.clone(),
            created_at_ms: m.created_at_ms,
            updated_at_ms: m.updated_at_ms,
        })
        .collect())
}
