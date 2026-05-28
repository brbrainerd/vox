//! Fetch PR review comments and normalize CodeRabbit-shaped markdown.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::debug;
use vox_corpus::external_review_replay::{
    extract_external_review_rows, validate_external_review_rows,
};
use vox_db::VoxDb;
use vox_db::store::types::{
    ExternalReviewFindingParams, ExternalReviewFindingStateParams, ExternalReviewRunParams,
    ExternalReviewThreadParams,
};
use vox_git::GitBridge;

use super::github::{forge_token, parse_github_owner_repo};

/// Normalized review item schema (ingestion output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedReviewItem {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub source_pr: u64,
    pub comment_id: u64,
    #[serde(default)]
    pub finding_identity: String,
    #[serde(default)]
    pub placement_kind: String,
    #[serde(default)]
    pub thread_identity: String,
    #[serde(default)]
    pub line_anchor_state: String,
    pub file_path: String,
    pub line: usize,
    pub line_end: Option<usize>,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub details: String,
    pub llm_prompt: Option<String>,
    pub suggested_fix: Option<String>,
    pub dedup_hash: String,
    #[serde(default)]
    pub source_payload_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity_reason: Option<String>,
}

const INGEST_SCHEMA_VERSION: u32 = 2;

fn default_schema_version() -> u32 {
    INGEST_SCHEMA_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    PullReviewComment,
    IssueComment,
    PullReviewSummary,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            SourceKind::PullReviewComment => "pull_review_comment",
            SourceKind::IssueComment => "issue_comment",
            SourceKind::PullReviewSummary => "review_summary",
        }
    }
}

#[derive(Debug, Deserialize)]
struct GhUser {
    login: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhReviewComment {
    id: u64,
    path: Option<String>,
    line: Option<u64>,
    #[serde(rename = "original_line")]
    original_line: Option<u64>,
    body: Option<String>,
    in_reply_to_id: Option<u64>,
    user: Option<GhUser>,
    #[serde(skip)]
    source_kind: Option<SourceKind>,
}

const GITHUB_PER_PAGE_DEFAULT: u32 = 100;

fn github_per_page() -> u32 {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCoderabbitGithubPerPage)
        .expose()
        .and_then(|s| s.parse().ok())
        .unwrap_or(GITHUB_PER_PAGE_DEFAULT)
        .clamp(1, 100)
}

async fn fetch_github_paginated(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    path: &str,
    source_kind: SourceKind,
) -> Result<Vec<GhReviewComment>> {
    let per_page = github_per_page();
    let mut all = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/{path}?per_page={per_page}&page={page}"
        );
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            // required User-Agent for GitHub APIs
            .header("User-Agent", "vox-cli")
            .send()
            .await
            .context(format!("GET {path}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("GitHub API {status}: {text}");
        }

        let mut comments: Vec<GhReviewComment> = resp.json().await.context("Parse JSON")?;
        for c in &mut comments {
            c.source_kind = Some(source_kind);
        }
        let n = comments.len();
        if n == 0 {
            break;
        }
        all.extend(comments);
        if n < per_page as usize {
            break;
        }
        page += 1;
    }

    Ok(all)
}

async fn fetch_all_pr_comments(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<Vec<GhReviewComment>> {
    let client = reqwest::Client::new();

    // 1. Inline review comments
    let mut all = fetch_github_paginated(
        &client,
        token,
        owner,
        repo,
        &format!("pulls/{pr_number}/comments"),
        SourceKind::PullReviewComment,
    )
    .await?;

    // 2. Issue comments (Main PR discussions)
    let issues = fetch_github_paginated(
        &client,
        token,
        owner,
        repo,
        &format!("issues/{pr_number}/comments"),
        SourceKind::IssueComment,
    )
    .await?;
    all.extend(issues);

    // 3. Top-level submitted reviews (Body content)
    let reviews = fetch_github_paginated(
        &client,
        token,
        owner,
        repo,
        &format!("pulls/{pr_number}/reviews"),
        SourceKind::PullReviewSummary,
    )
    .await?;
    all.extend(reviews);

    Ok(all)
}

fn parse_coderabbit_body(body: &str) -> (String, String, Option<String>, String) {
    let mut category = "style".to_string();
    let mut title = String::new();
    let mut details = String::new();
    let mut suggested_fix: Option<String> = None;

    let body = body.trim();
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("**") {
            let rest = line.trim_start_matches('*').trim_end_matches('*').trim();
            let lower = rest.to_lowercase();
            if lower == "suggested fix" || lower == "suggestion" || lower == "suggested change" {
                i += 1;
                let mut fix_lines = Vec::new();
                while i < lines.len() && !lines[i].trim().starts_with("**") {
                    fix_lines.push(lines[i]);
                    i += 1;
                }
                let fix = fix_lines.join("\n").trim().to_string();
                if !fix.is_empty() {
                    suggested_fix = Some(fix);
                }
                continue;
            }
            if matches!(
                lower.as_str(),
                "security"
                    | "performance"
                    | "logic"
                    | "error handling"
                    | "style"
                    | "dead code"
                    | "dependencies"
            ) {
                category = rest.to_lowercase().replace(' ', "_");
                i += 1;
                if i < lines.len() && !lines[i].trim().starts_with("**") {
                    let t = lines[i].trim();
                    if !t.is_empty() {
                        title = t.to_string();
                    }
                    i += 1;
                }
                let mut detail_lines = Vec::new();
                while i < lines.len() && !lines[i].trim().starts_with("**") {
                    detail_lines.push(lines[i]);
                    i += 1;
                }
                let d = detail_lines.join("\n").trim().to_string();
                if !d.is_empty() {
                    details = d;
                }
                continue;
            }
            if rest.is_empty() {
                i += 1;
                continue;
            }
        }
        if title.is_empty() && !line.trim().is_empty() && !line.trim().starts_with('#') {
            title = line.trim().to_string();
        } else if !line.trim().is_empty() {
            if !details.is_empty() {
                details.push('\n');
            }
            details.push_str(line);
        }
        i += 1;
    }

    if title.is_empty() && !details.is_empty() {
        title = details.lines().next().unwrap_or("").trim().to_string();
        if title.len() > 80 {
            title = format!("{}...", &title[..77]);
        }
    }
    if title.is_empty() {
        title = "Code review finding".to_string();
    }

    (category, title, suggested_fix, details)
}

fn infer_severity(category: &str, details: &str) -> (String, f64, String) {
    let cat = category.to_lowercase();
    let (base_severity, mut confidence, reason) = match cat.as_str() {
        "security" => ("error", 0.9, "category: security".to_string()),
        "logic" | "error_handling" => ("warning", 0.85, format!("category: {cat}")),
        "performance" => ("warning", 0.8, "category: performance".to_string()),
        "dead_code" => ("info", 0.75, "category: dead_code".to_string()),
        "dependencies" => ("warning", 0.7, "category: dependencies".to_string()),
        "style" => ("info", 0.6, "category: style".to_string()),
        _ => ("info", 0.4, format!("category: {cat} (unknown)")),
    };
    if details.contains("```") || details.contains("suggested") {
        confidence = (confidence + 0.1_f64).min(1.0);
    }
    let (severity, final_reason) = if confidence < 0.5 {
        (
            "needs_review".to_string(),
            "low confidence, manual review".to_string(),
        )
    } else {
        (base_severity.to_string(), reason)
    };
    (severity, confidence, final_reason)
}

fn dedup_hash(path: &str, line: usize, title: &str, details: &str) -> String {
    let mut data = Vec::new();
    data.extend_from_slice(path.as_bytes());
    data.extend_from_slice(&line.to_le_bytes());
    data.extend_from_slice(title.as_bytes());
    data.extend_from_slice(details.as_bytes());
    let h = blake3::hash(&data);
    h.to_hex().to_string()
}

fn extract_nitpicks(body: &str) -> Vec<(String, usize, String, String)> {
    let mut nitpicks = Vec::new();
    let mut in_nitpicks = false;
    let mut table_start = false;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## Nitpicks") {
            in_nitpicks = true;
            continue;
        }
        if in_nitpicks && trimmed.starts_with("## ") {
            break; // Next section
        }
        if in_nitpicks && trimmed.starts_with('|') {
            if trimmed.contains("---") || trimmed.contains("File") {
                table_start = true;
                continue;
            }
            if table_start {
                let columns: Vec<&str> = trimmed.split('|').collect();
                if columns.len() >= 4 {
                    let file = columns[1].trim().trim_matches('`').to_string();
                    let line_num = columns[2].trim().parse::<usize>().unwrap_or(0);
                    let suggestion = columns[3].trim().to_string();
                    if !file.is_empty() {
                        nitpicks.push((
                            file,
                            line_num,
                            "CodeRabbit Nitpick".to_string(),
                            suggestion,
                        ));
                    }
                }
            }
        }
    }
    nitpicks
}

fn payload_hash(
    source_kind: &str,
    comment_id: u64,
    path: &str,
    line: usize,
    body: &str,
    in_reply_to_id: Option<u64>,
) -> String {
    let mut data = Vec::new();
    data.extend_from_slice(source_kind.as_bytes());
    data.extend_from_slice(&comment_id.to_le_bytes());
    data.extend_from_slice(path.as_bytes());
    data.extend_from_slice(&line.to_le_bytes());
    data.extend_from_slice(body.as_bytes());
    data.extend_from_slice(&in_reply_to_id.unwrap_or_default().to_le_bytes());
    blake3::hash(&data).to_hex().to_string()
}

fn placement_kind(source_kind: SourceKind, in_reply_to_id: Option<u64>) -> &'static str {
    match source_kind {
        SourceKind::PullReviewSummary => "review_summary",
        SourceKind::IssueComment => "issue_comment",
        SourceKind::PullReviewComment => {
            if in_reply_to_id.is_some() {
                "reply"
            } else {
                "inline"
            }
        }
    }
}

fn map_category_to_anti_pattern_id(category: &str, severity: &str) -> &'static str {
    match (category, severity) {
        ("security", _) => "review/security-risk",
        ("performance", "error" | "warning") => "review/performance-regression",
        ("logic", "error" | "warning") => "review/logic-bug",
        ("error_handling", "error" | "warning") => "review/error-handling-gap",
        ("dependencies", _) => "review/dependency-hygiene",
        ("dead_code", _) => "review/dead-code",
        ("nitpick", _) => "review/style-nitpick",
        _ => "review/general-quality",
    }
}

fn line_anchor_state(line: Option<u64>, original_line: Option<u64>) -> &'static str {
    if line.is_some() {
        "current"
    } else if original_line.is_some() {
        "outdated"
    } else {
        "missing"
    }
}

fn thread_identity(
    source_kind: SourceKind,
    comment_id: u64,
    in_reply_to_id: Option<u64>,
    file_path: &str,
) -> String {
    let base = in_reply_to_id.unwrap_or(comment_id);
    format!(
        "{}:{}:{}:{}",
        source_kind.as_str(),
        base,
        comment_id,
        file_path.replace('\\', "/")
    )
}

fn finding_identity(
    pr_number: u64,
    source_kind: SourceKind,
    comment_id: u64,
    dedup: &str,
) -> String {
    let suffix = dedup.get(..12).unwrap_or(dedup);
    format!(
        "cr:{pr_number}:{}:{comment_id}:{suffix}",
        source_kind.as_str()
    )
}

fn should_ingest_comment(c: &GhReviewComment, body: &str) -> bool {
    let login = c
        .user
        .as_ref()
        .and_then(|u| u.login.as_ref())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    login.contains("coderabbit")
        || body.to_lowercase().contains("coderabbit")
        || body.contains("@coderabbitai")
}

/// Ingest PR review comments and produce normalized items.
pub async fn ingest_pr(pr_number: u64, path: &Path) -> Result<Vec<NormalizedReviewItem>> {
    let bridge = GitBridge::open(path).context("Open git repo")?;
    let remote_url = bridge.remote_url().context("Get remote URL")?;
    let (owner, repo) =
        parse_github_owner_repo(&remote_url).context("Parse owner/repo from remote URL")?;

    let token = forge_token()?;
    let comments = fetch_all_pr_comments(&token, &owner, &repo, pr_number).await?;

    let mut items = Vec::new();
    for c in comments {
        let body = c.body.as_deref().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        if !should_ingest_comment(&c, body) {
            continue;
        }
        let source_kind = c.source_kind.unwrap_or(SourceKind::IssueComment);
        debug!(
            "parse_coderabbit_body input (comment_id={}): {:?}",
            c.id, body
        );

        // 1. Extract any table-based nitpicks
        let nitpicks = extract_nitpicks(body);
        for (file, ln, title, details) in nitpicks {
            let hash = dedup_hash(&file, ln, &title, &details);
            let payload = payload_hash(
                source_kind.as_str(),
                c.id,
                &file,
                ln,
                body,
                c.in_reply_to_id,
            );
            let thread = thread_identity(source_kind, c.id, c.in_reply_to_id, &file);
            items.push(NormalizedReviewItem {
                schema_version: INGEST_SCHEMA_VERSION,
                source_pr: pr_number,
                comment_id: c.id,
                finding_identity: finding_identity(pr_number, source_kind, c.id, &hash),
                placement_kind: placement_kind(source_kind, c.in_reply_to_id).to_string(),
                thread_identity: thread,
                line_anchor_state: "current".to_string(),
                file_path: file,
                line: ln,
                line_end: None,
                severity: "info".to_string(),
                category: "nitpick".to_string(),
                title: title.clone(),
                details: details.clone(),
                llm_prompt: None,
                suggested_fix: None,
                dedup_hash: hash,
                source_payload_hash: payload,
                raw_body: None,
                confidence: Some(0.9),
                severity_reason: Some("category: nitpick".to_string()),
            });
        }

        let file_path = c.path.unwrap_or_else(|| "global".to_string());
        let line = c.line.or(c.original_line).unwrap_or(0) as usize;
        let anchor_state = line_anchor_state(c.line, c.original_line).to_string();
        let place = placement_kind(source_kind, c.in_reply_to_id).to_string();
        let payload = payload_hash(
            source_kind.as_str(),
            c.id,
            &file_path,
            line,
            body,
            c.in_reply_to_id,
        );
        let thread = thread_identity(source_kind, c.id, c.in_reply_to_id, &file_path);

        // 2. Global walkthroughs / PR Summaries
        if file_path == "global" && body.contains("## Walkthrough") {
            let hash = dedup_hash(
                &file_path,
                line,
                "Walkthrough",
                "Walkthrough summary included",
            );
            items.push(NormalizedReviewItem {
                schema_version: INGEST_SCHEMA_VERSION,
                source_pr: pr_number,
                comment_id: c.id,
                finding_identity: finding_identity(pr_number, source_kind, c.id, &hash),
                placement_kind: place.clone(),
                thread_identity: thread.clone(),
                line_anchor_state: anchor_state.clone(),
                file_path: file_path.clone(),
                line,
                line_end: None,
                severity: "info".to_string(),
                category: "walkthrough".to_string(),
                title: "CodeRabbit Walkthrough".to_string(),
                details: body.to_string(),
                llm_prompt: None,
                suggested_fix: None,
                dedup_hash: hash,
                source_payload_hash: payload.clone(),
                raw_body: Some(body.to_string()),
                confidence: Some(1.0),
                severity_reason: Some("category: walkthrough".to_string()),
            });
            continue;
        }

        // 3. Standard parsing
        let (category, title, suggested_fix, details) = parse_coderabbit_body(body);
        let (severity, confidence, severity_reason) = infer_severity(&category, &details);

        let hash = dedup_hash(&file_path, line, &title, &details);

        items.push(NormalizedReviewItem {
            schema_version: INGEST_SCHEMA_VERSION,
            source_pr: pr_number,
            comment_id: c.id,
            finding_identity: finding_identity(pr_number, source_kind, c.id, &hash),
            placement_kind: place,
            thread_identity: thread,
            line_anchor_state: anchor_state,
            file_path,
            line,
            line_end: None,
            severity,
            category,
            title,
            details,
            llm_prompt: None,
            suggested_fix,
            dedup_hash: hash,
            source_payload_hash: payload,
            raw_body: None,
            confidence: Some(confidence),
            severity_reason: Some(severity_reason),
        });
    }

    Ok(items)
}

fn normalize_legacy_item(mut item: NormalizedReviewItem) -> NormalizedReviewItem {
    if item.schema_version == 0 {
        item.schema_version = INGEST_SCHEMA_VERSION;
    }
    if item.placement_kind.is_empty() {
        item.placement_kind = if item.file_path == "global" {
            "review_summary".to_string()
        } else {
            "inline".to_string()
        };
    }
    if item.thread_identity.is_empty() {
        item.thread_identity = format!("legacy:{}:{}", item.source_pr, item.comment_id);
    }
    if item.line_anchor_state.is_empty() {
        item.line_anchor_state = if item.line > 0 {
            "current".to_string()
        } else {
            "missing".to_string()
        };
    }
    if item.source_payload_hash.is_empty() {
        item.source_payload_hash = blake3::hash(item.details.as_bytes()).to_hex().to_string();
    }
    if item.finding_identity.is_empty() {
        item.finding_identity = format!(
            "legacy:{}:{}:{}",
            item.source_pr,
            item.comment_id,
            item.dedup_hash.get(..12).unwrap_or(&item.dedup_hash)
        );
    }
    item
}

async fn persist_items_to_db(
    owner: &str,
    repo: &str,
    pr_number: u64,
    commit_sha: Option<&str>,
    trigger_kind: &str,
    idempotency_key: Option<&str>,
    reingest_window: Option<&str>,
    items: &[NormalizedReviewItem],
) -> Result<i64> {
    let repository_id = format!("{owner}/{repo}");
    let db = VoxDb::connect_default()
        .await
        .context("connect VoxDb for external review ingest")?;
    let metadata_json = serde_json::json!({
        "reingest_window": reingest_window,
        "schema_version": INGEST_SCHEMA_VERSION,
    });
    let metadata_json_s = metadata_json.to_string();
    let run_id = db
        .insert_external_review_run(ExternalReviewRunParams {
            provider: "coderabbit",
            repository_id: &repository_id,
            owner,
            repo,
            pr_number: pr_number as i64,
            commit_sha,
            trigger_kind,
            idempotency_key,
            item_count: items.len() as i64,
            metadata_json: Some(metadata_json_s.as_str()),
        })
        .await
        .context("insert external review run")?;

    for item in items {
        let payload_json = serde_json::to_string(item).unwrap_or_else(|_| "{}".to_string());
        db.upsert_external_review_thread(ExternalReviewThreadParams {
            provider: "coderabbit",
            repository_id: &repository_id,
            pr_number: pr_number as i64,
            thread_identity: &item.thread_identity,
            placement_kind: &item.placement_kind,
            line_anchor_state: &item.line_anchor_state,
            file_path: Some(&item.file_path),
            line_start: Some(item.line as i64),
            line_end: item.line_end.map(|v| v as i64),
            source_comment_id: Some(item.comment_id as i64),
            parent_comment_id: None,
            source_payload_hash: &item.source_payload_hash,
            raw_payload_json: &payload_json,
        })
        .await
        .with_context(|| format!("upsert thread {}", item.thread_identity))?;

        let finding_id = db
            .upsert_external_review_finding(ExternalReviewFindingParams {
                run_id,
                provider: "coderabbit",
                repository_id: &repository_id,
                pr_number: pr_number as i64,
                finding_identity: &item.finding_identity,
                thread_identity: Some(&item.thread_identity),
                source_comment_id: Some(item.comment_id as i64),
                placement_kind: &item.placement_kind,
                line_anchor_state: &item.line_anchor_state,
                file_path: Some(&item.file_path),
                line_start: Some(item.line as i64),
                line_end: item.line_end.map(|v| v as i64),
                category: &item.category,
                anti_pattern_id: Some(map_category_to_anti_pattern_id(
                    &item.category,
                    &item.severity,
                )),
                severity: &item.severity,
                title: &item.title,
                details: &item.details,
                suggested_fix: item.suggested_fix.as_deref(),
                extraction_confidence: item.confidence,
                source_payload_hash: &item.source_payload_hash,
                fingerprint: &item.dedup_hash,
                status: "unverified",
            })
            .await
            .with_context(|| format!("upsert finding {}", item.finding_identity))?;

        let _ = db
            .append_external_review_finding_state(ExternalReviewFindingStateParams {
                finding_id,
                previous_state: Some("open"),
                new_state: "unverified",
                reason: item.severity_reason.as_deref(),
                confidence: item.confidence,
                evidence_ref: Some("ingest"),
            })
            .await;
    }
    Ok(run_id)
}

fn choose_db_mode(persist: bool, db_only: bool, db_and_cache: bool) -> (bool, bool) {
    if db_and_cache {
        return (true, true);
    }
    if db_only {
        return (true, false);
    }
    (true, persist)
}

/// Run ingest command: fetch, normalize, output, persist to DB and optional cache.
pub async fn run_ingest(
    pr_number: u64,
    output: Option<&Path>,
    persist: bool,
    db_only: bool,
    db_and_cache: bool,
    reingest_window: Option<&str>,
    idempotency_key: Option<&str>,
    path: &Path,
) -> Result<()> {
    let bridge = GitBridge::open(path).context("Open git repo")?;
    let remote_url = bridge.remote_url().context("Get remote URL")?;
    let (owner, repo) =
        parse_github_owner_repo(&remote_url).context("Parse owner/repo from remote URL")?;

    let mut items = ingest_pr(pr_number, path).await?;
    items = items.into_iter().map(normalize_legacy_item).collect();

    let json = serde_json::to_string_pretty(&items).context("Serialize normalized items")?;
    let (persist_db, persist_cache) = choose_db_mode(persist, db_only, db_and_cache);

    match output {
        Some(p) => {
            std::fs::write(p, &json).with_context(|| format!("Write to {}", p.display()))?;
            eprintln!("Ingested {} items to {}", items.len(), p.display());
        }
        None => {
            println!("{json}");
        }
    }

    if persist_db {
        let commit_sha_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGithubSha);
        let commit_sha = commit_sha_resolved.expose();
        let run_id = persist_items_to_db(
            &owner,
            &repo,
            pr_number,
            commit_sha.as_deref(),
            if reingest_window.is_some() {
                "reingest"
            } else {
                "review"
            },
            idempotency_key,
            reingest_window,
            &items,
        )
        .await?;
        eprintln!(
            "Persisted {} findings to VoxDB run_id={run_id}",
            items.len()
        );
    }

    if persist_cache {
        let cr_dir = path.join(".coderabbit");
        std::fs::create_dir_all(&cr_dir).ok();
        let cache_path = cr_dir.join("ingested_findings.json");
        std::fs::write(&cache_path, &json)
            .with_context(|| format!("Write local cache {}", cache_path.display()))?;
        eprintln!(
            "Persisted {} items to {}",
            items.len(),
            cache_path.display()
        );
    }

    Ok(())
}

/// Backfill local cached findings into VoxDB.
pub async fn run_db_backfill(
    input: Option<&Path>,
    persist_local_cache: bool,
    idempotency_key: Option<&str>,
    path: &Path,
) -> Result<()> {
    let cache_path = input
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| path.join(".coderabbit").join("ingested_findings.json"));
    let raw = std::fs::read_to_string(&cache_path)
        .with_context(|| format!("read {}", cache_path.display()))?;
    let mut items: Vec<NormalizedReviewItem> =
        serde_json::from_str(&raw).context("parse cached ingested findings json")?;
    items = items.into_iter().map(normalize_legacy_item).collect();
    let pr = items.first().map(|i| i.source_pr).unwrap_or(0);
    if pr == 0 {
        anyhow::bail!("cannot backfill: cached file has no source_pr");
    }

    let bridge = GitBridge::open(path).context("Open git repo")?;
    let remote_url = bridge.remote_url().context("Get remote URL")?;
    let (owner, repo) =
        parse_github_owner_repo(&remote_url).context("Parse owner/repo from remote URL")?;

    let run_id = persist_items_to_db(
        &owner,
        &repo,
        pr,
        None,
        "backfill",
        idempotency_key,
        Some("historical_cache"),
        &items,
    )
    .await?;
    eprintln!(
        "Backfilled {} findings from {} to VoxDB run_id={run_id}",
        items.len(),
        cache_path.display()
    );

    if persist_local_cache {
        let normalized = serde_json::to_string_pretty(&items).context("serialize normalized")?;
        std::fs::write(&cache_path, normalized)
            .with_context(|| format!("rewrite normalized cache {}", cache_path.display()))?;
    }

    Ok(())
}

/// Print DB report for latest run and deadletters.
pub async fn run_db_report(pr_number: u64, path: &Path, limit: i64, json: bool) -> Result<()> {
    let bridge = GitBridge::open(path).context("Open git repo")?;
    let remote_url = bridge.remote_url().context("Get remote URL")?;
    let (owner, repo) =
        parse_github_owner_repo(&remote_url).context("Parse owner/repo from remote URL")?;
    let repository_id = format!("{owner}/{repo}");

    let db = VoxDb::connect_default()
        .await
        .context("connect VoxDb for db-report")?;
    let latest = db
        .latest_external_review_run(&repository_id, pr_number as i64)
        .await
        .context("query latest external review run")?;
    let deadletters = db
        .list_external_review_deadletters(&repository_id, pr_number as i64, limit)
        .await
        .context("query deadletters")?;
    let findings = db
        .list_external_review_findings_for_training_window(&repository_id, limit)
        .await
        .context("query findings window")?;

    let report = serde_json::json!({
        "repository_id": repository_id,
        "pr_number": pr_number,
        "latest_run": latest,
        "deadletter_count": deadletters.len(),
        "deadletters": deadletters,
        "finding_window_count": findings.len(),
        "findings": findings,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        eprintln!(
            "db-report pr={} latest_run={} findings={} deadletters={}",
            pr_number,
            report["latest_run"]["id"].as_i64().unwrap_or(0),
            report["finding_window_count"].as_u64().unwrap_or(0),
            report["deadletter_count"].as_u64().unwrap_or(0)
        );
    }
    Ok(())
}

/// Mark one deadletter row retried.
pub async fn run_deadletter_retry(id: i64, _path: &Path) -> Result<()> {
    let db = VoxDb::connect_default()
        .await
        .context("connect VoxDb for deadletter-retry")?;
    db.mark_external_review_deadletter_retried(id)
        .await
        .with_context(|| format!("mark deadletter retried id={id}"))?;
    eprintln!("Marked deadletter {id} as retried");
    Ok(())
}

/// Print repository-level DB status.
pub async fn run_db_status(path: &Path, json: bool) -> Result<()> {
    let bridge = GitBridge::open(path).context("Open git repo")?;
    let remote_url = bridge.remote_url().context("Get remote URL")?;
    let (owner, repo) =
        parse_github_owner_repo(&remote_url).context("Parse owner/repo from remote URL")?;
    let repository_id = format!("{owner}/{repo}");
    let db = VoxDb::connect_default()
        .await
        .context("connect VoxDb for db-status")?;

    let run_count = db
        .count_external_review_runs_for_repository(&repository_id)
        .await
        .context("count external_review_run")?;

    let finding_count = db
        .count_external_review_findings_for_repository(&repository_id)
        .await
        .context("count external_review_finding")?;

    let deadletter_pending = db
        .count_external_review_deadletters_pending_for_repository(&repository_id)
        .await
        .context("count external_review_deadletter pending")?;

    let kpi = db
        .list_external_review_kpi_snapshots(&repository_id, 10)
        .await
        .context("list kpi snapshots")?;

    let out = serde_json::json!({
        "repository_id": repository_id,
        "run_count": run_count,
        "finding_count": finding_count,
        "deadletter_pending": deadletter_pending,
        "kpi_snapshots": kpi,
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        eprintln!(
            "db-status repository={} runs={} findings={} deadletter_pending={} kpi_snapshots={}",
            out["repository_id"].as_str().unwrap_or("unknown"),
            run_count,
            finding_count,
            deadletter_pending,
            out["kpi_snapshots"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0)
        );
    }
    Ok(())
}

/// Build review dataset artifacts and validate them for training sync.
pub async fn run_learning_sync(path: &Path, repository_id: Option<&str>, limit: i64) -> Result<()> {
    let repo_id = if let Some(rid) = repository_id {
        rid.to_string()
    } else {
        let bridge = GitBridge::open(path).context("Open git repo")?;
        let remote_url = bridge.remote_url().context("Get remote URL")?;
        let (owner, repo) =
            parse_github_owner_repo(&remote_url).context("Parse owner/repo from remote URL")?;
        format!("{owner}/{repo}")
    };
    let output = std::path::PathBuf::from("mens/data/mix_sources/review_findings.jsonl");
    let db = VoxDb::connect_default().await.context("Connect to VoxDb")?;
    let rows = extract_external_review_rows(&db, &repo_id, limit)
        .await
        .context("Extract review rows")?;

    let parent = output.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent).context("Create output directory")?;

    let mut body = String::new();
    for row in &rows {
        body.push_str(&serde_json::to_string(row).context("Serialize row")?);
        body.push('\n');
    }
    std::fs::write(&output, body).context("Write output JSONL")?;

    validate_external_review_rows(&rows).context("Validate extracted review rows")?;
    eprintln!(
        "learning-sync complete repository={} output={}",
        repo_id,
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_coderabbit_body_suggested_fix() {
        let body = "**security**\n\nPotential XSS.\n\n**Suggested fix**\nUse `escape_html()`.";
        let (cat, title, fix, details) = parse_coderabbit_body(body);
        assert!(cat.contains("security") || cat == "security");
        assert!(title.contains("XSS") || details.contains("XSS"));
        assert!(fix.is_some());
        assert!(fix.unwrap().contains("escape_html"));
    }

    #[test]
    fn dedup_hash_deterministic() {
        let h1 = dedup_hash("a.rs", 10, "title", "details");
        let h2 = dedup_hash("a.rs", 10, "title", "details");
        assert_eq!(h1, h2);
    }

    #[test]
    fn payload_hash_is_stable() {
        let h1 = payload_hash("inline", 1, "a.rs", 10, "body", Some(2));
        let h2 = payload_hash("inline", 1, "a.rs", 10, "body", Some(2));
        assert_eq!(h1, h2);
    }

    #[test]
    fn normalize_legacy_item_fills_required_contract_fields() {
        let legacy = NormalizedReviewItem {
            schema_version: 0,
            source_pr: 12,
            comment_id: 34,
            finding_identity: String::new(),
            placement_kind: String::new(),
            thread_identity: String::new(),
            line_anchor_state: String::new(),
            file_path: "global".to_string(),
            line: 0,
            line_end: None,
            severity: "info".to_string(),
            category: "style".to_string(),
            title: "Title".to_string(),
            details: "Details".to_string(),
            llm_prompt: None,
            suggested_fix: None,
            dedup_hash: "abc".to_string(),
            source_payload_hash: String::new(),
            raw_body: None,
            confidence: None,
            severity_reason: None,
        };
        let normalized = normalize_legacy_item(legacy);
        assert_eq!(normalized.schema_version, INGEST_SCHEMA_VERSION);
        assert!(!normalized.finding_identity.is_empty());
        assert_eq!(normalized.placement_kind, "review_summary");
        assert_eq!(normalized.line_anchor_state, "missing");
        assert!(!normalized.source_payload_hash.is_empty());
    }

    #[test]
    fn taxonomy_mapping_is_non_empty() {
        assert_eq!(
            map_category_to_anti_pattern_id("security", "error"),
            "review/security-risk"
        );
        assert_eq!(
            map_category_to_anti_pattern_id("unknown", "info"),
            "review/general-quality"
        );
    }
}
