//! Chat, inline edit, and planning tools for the Vox MCP server.
//!
//! These back the VS Code extension thin-client layer. All context gathering,
//! @mention resolution, LLM routing, and history persistence happen here in Rust.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::params::ToolResult;
use crate::server::ServerState;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
    pub timestamp: u64,
    pub context_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessageParams {
    pub prompt: String,
    #[serde(default)]
    pub context_files: Vec<String>,
    /// Open file paths provided by the editor for implicit context injection
    #[serde(default)]
    pub open_files: Vec<String>,
    /// Active editor file path (workspace-relative)
    #[serde(default)]
    pub active_file: Option<String>,
    /// Active editor cursor line (1-indexed)
    #[serde(default)]
    pub active_line: Option<u32>,
    /// Selected text in the active editor
    #[serde(default)]
    pub selected_text: Option<String>,
    /// Active LSP diagnostics to inject as context
    #[serde(default)]
    pub diagnostics: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub struct InlineEditParams {
    /// The edit instruction / prompt from the user
    pub prompt: String,
    /// Workspace-relative file path
    pub file: String,
    /// Start line of target range (1-indexed)
    pub start_line: u32,
    /// End line of target range (1-indexed, inclusive)
    pub end_line: u32,
    /// The current text in the range (sent by editor, avoids FS read latency)
    pub current_text: String,
    /// Language ID of the file
    #[serde(default)]
    pub language: Option<String>,
    /// Surrounding context lines before and after the range (0-40 lines typically)
    #[serde(default)]
    pub context_before: Option<String>,
    #[serde(default)]
    pub context_after: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InlineEditResult {
    /// Replacement text for the range [start_line, end_line]
    pub replacement: String,
    /// Human-readable explanation of what was changed
    pub explanation: String,
    /// Estimated token usage
    pub tokens: u64,
    /// Model that produced this edit
    pub model_used: String,
}

#[derive(Debug, Deserialize)]
pub struct PlanParams {
    /// The request / goal to plan for
    pub goal: String,
    /// Optional files to scope the plan to
    #[serde(default)]
    pub scope_files: Vec<String>,
    /// Whether to write the plan to PLAN.md in the workspace root
    #[serde(default)]
    pub write_to_disk: bool,
    /// Maximum number of tasks to generate (default: 30)
    #[serde(default)]
    pub max_tasks: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct PlanTask {
    pub id: usize,
    pub description: String,
    pub files: Vec<String>,
    pub estimated_complexity: u8, // 1-10
    pub depends_on: Vec<usize>,
}

#[derive(Debug, Serialize)]
pub struct PlanResult {
    pub goal: String,
    pub tasks: Vec<PlanTask>,
    pub summary: String,
    pub plan_md: String,
    pub written_to_disk: bool,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Resolve @filename mentions in a prompt by searching the workspace for the file
/// and injecting its content. Truncates to 8000 chars to avoid context blowout.
fn resolve_mentions(prompt: &str, workspace_root: &std::path::Path) -> (String, Vec<String>) {
    let mut expanded = prompt.to_string();
    let mut resolved_files = Vec::new();

    let mention_re = regex::Regex::new(r"@([A-Za-z0-9_.:/\\-]+)").unwrap();
    for cap in mention_re.captures_iter(prompt) {
        let filename = &cap[1];
        // Walk workspace to find the file (cap at found=1)
        let mut found: Option<std::path::PathBuf> = None;
        for entry in walkdir::WalkDir::new(workspace_root)
            .follow_links(false)
            .max_depth(10)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let entry_path = entry.path();
                let entry_name = entry_path.file_name().unwrap_or_default().to_string_lossy();
                let entry_rel = entry_path
                    .strip_prefix(workspace_root)
                    .unwrap_or(entry_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if entry_name == filename || entry_rel == filename || entry_rel.ends_with(filename) {
                    found = Some(entry_path.to_path_buf());
                    break;
                }
            }
        }
        if let Some(path) = found {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel = path
                    .strip_prefix(workspace_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let truncated = if content.len() > 8000 {
                    format!("{}\n...[truncated]...", &content[..8000])
                } else {
                    content.clone()
                };
                let replacement = format!(
                    "\n\n--- @{filename} ({rel}) ---\n{truncated}\n---\n"
                );
                expanded = expanded.replace(&cap[0], &replacement);
                resolved_files.push(rel);
            }
        }
    }
    (expanded, resolved_files)
}

/// Build the full system prompt for the Vox chat assistant.
fn build_system_prompt(state: &ServerState) -> String {
    let ws_root = state
        .workspace_root
        .as_deref()
        .unwrap_or(std::path::Path::new("."))
        .display()
        .to_string();

    format!(
        r#"You are Vox, an elite AI coding assistant embedded inside VS Code.

Workspace: {ws_root}
You have access to the full Vox MCP toolbelt. You can read and modify files, run tests, inspect VCS history, manage agents, and query the knowledge graph.

Rules:
- Be concise and precise. Prefer code over prose.
- Always cite which files you modified or plan to modify.
- When generating code, produce valid, complete implementations — no stubs or placeholders.
- Use Markdown code blocks with language tags.
- For multi-file changes, use a structured diff or list each file separately.
- When asked to plan, produce a numbered task list in Markdown.
"#
    )
}

/// Route a prompt through the best available LLM using the orchestrator model registry.
async fn call_llm(
    state: &ServerState,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<(String, String, u64), String> {
    let orch = state.orchestrator.lock().await;
    let preference = orch.config().cost_preference;

    // Pick best model for chat (codegen task, midpoint complexity)
    let model = orch
        .models()
        .best_for(
            vox_orchestrator::types::TaskCategory::CodeGen,
            5,
            preference,
        )
        .or_else(|| orch.models().cheapest_free())
        .ok_or_else(|| "No models available in registry".to_string())?;

    drop(orch); // release lock before await

    // Build the actual HTTP call to the appropriate provider
    let (response_text, tokens) = dispatch_to_provider(state, &model, system_prompt, user_prompt).await?;

    Ok((response_text, model.id.clone(), tokens))
}

/// Dispatch the request to the correct API endpoint based on provider_type.
async fn dispatch_to_provider(
    _state: &ServerState,
    model: &vox_orchestrator::models::ModelSpec,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<(String, u64), String> {
    use vox_orchestrator::models::ProviderType;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    match &model.provider_type {
        ProviderType::GoogleDirect => {
            let api_key = std::env::var("GEMINI_API_KEY")
                .or_else(|_| std::env::var("GOOGLE_AI_API_KEY"))
                .map_err(|_| "GEMINI_API_KEY not set".to_string())?;

            let model_id = &model.id;
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model_id}:generateContent?key={api_key}"
            );

            let body = serde_json::json!({
                "system_instruction": { "parts": [{ "text": system_prompt }] },
                "contents": [{ "parts": [{ "text": user_prompt }], "role": "user" }],
                "generationConfig": { "maxOutputTokens": 8192 }
            });

            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(format!("Google API error {status}: {text}"));
            }

            let json: Value = resp.json().await.map_err(|e| e.to_string())?;
            let text = json["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let tokens = json["usageMetadata"]["totalTokenCount"]
                .as_u64()
                .unwrap_or(0);
            Ok((text, tokens))
        }

        ProviderType::OpenRouter => {
            let api_key = std::env::var("OPENROUTER_API_KEY")
                .map_err(|_| "OPENROUTER_API_KEY not set".to_string())?;

            let body = serde_json::json!({
                "model": model.id,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "max_tokens": 8192
            });

            let resp = client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {api_key}"))
                .header("HTTP-Referer", "https://vox-lang.dev")
                .header("X-Title", "Vox IDE")
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(format!("OpenRouter error {status}: {text}"));
            }

            let json: Value = resp.json().await.map_err(|e| e.to_string())?;
            let text = json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
            Ok((text, tokens))
        }

        ProviderType::Ollama => {
            let body = serde_json::json!({
                "model": model.id,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "stream": false
            });

            let resp = client
                .post("http://localhost:11434/api/chat")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Ollama not running (localhost:11434): {e}"))?;

            let json: Value = resp.json().await.map_err(|e| e.to_string())?;
            let text = json["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let tokens = json["eval_count"].as_u64().unwrap_or(0)
                + json["prompt_eval_count"].as_u64().unwrap_or(0);
            Ok((text, tokens))
        }
    }
}

// ─── Tool Handlers ────────────────────────────────────────────────────────────

/// Handle a user chat message. Resolves @mentions, injects context from the editor,
/// calls the best available LLM, persists to session history, and returns the updated history.
pub async fn chat_message(state: &ServerState, params: ChatMessageParams) -> String {
    // 1. Resolve @mentions in the prompt
    let workspace_root = state
        .workspace_root
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let (expanded_prompt, mention_files) = resolve_mentions(&params.prompt, &workspace_root);

    // 2. Build context preamble from editor state
    let mut context_parts = Vec::new();

    if let Some(active_file) = &params.active_file {
        let line_info = params.active_line
            .map(|l| format!(" (line {l})"))
            .unwrap_or_default();
        context_parts.push(format!("[ACTIVE FILE]: {active_file}{line_info}"));
    }

    if let Some(selected) = &params.selected_text {
        if !selected.is_empty() {
            context_parts.push(format!("[SELECTED TEXT]:\n{selected}"));
        }
    }

    if !params.diagnostics.is_empty() {
        let diag_str: Vec<String> = params.diagnostics.iter()
            .filter_map(|d| {
                let msg = d["message"].as_str()?;
                let line = d["line"].as_u64().unwrap_or(0);
                let sev = d["severity"].as_str().unwrap_or("error");
                Some(format!("  Line {line} [{sev}]: {msg}"))
            })
            .collect();
        if !diag_str.is_empty() {
            context_parts.push(format!("[ACTIVE ERRORS/WARNINGS]:\n{}", diag_str.join("\n")));
        }
    }

    if !params.open_files.is_empty() {
        context_parts.push(format!("[OPEN FILES]: {}", params.open_files.join(", ")));
    }

    let all_context_files: Vec<String> = {
        let mut v = params.context_files.clone();
        v.extend(mention_files);
        v.dedup();
        v
    };

    let user_prompt = if context_parts.is_empty() {
        expanded_prompt.clone()
    } else {
        format!("{}\n\n{}", context_parts.join("\n"), expanded_prompt)
    };

    // 3. Call LLM
    let system_prompt = build_system_prompt(state);
    let (response_text, model_used, tokens) = match call_llm(state, &system_prompt, &user_prompt).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err(format!("LLM error: {e}")).to_json();
        }
    };

    // 4. Persist to session history via memory store
    let user_msg = ChatMessage {
        id: format!("usr-{}", now_ts()),
        role: "user".to_string(),
        content: params.prompt.clone(), // store the original, not expanded
        timestamp: now_ts(),
        context_files: all_context_files,
        model_used: None,
        tokens: None,
    };
    let asst_msg = ChatMessage {
        id: format!("asst-{}", now_ts() + 1),
        role: "assistant".to_string(),
        content: response_text.clone(),
        timestamp: now_ts() + 1,
        context_files: vec![],
        model_used: Some(model_used.clone()),
        tokens: Some(tokens),
    };

    // Load existing history from context store, append, re-save
    let history_key = "chat_history:default";
    let orch = state.orchestrator.lock().await;
    let existing_history: Vec<ChatMessage> = orch.context()
        .get(history_key)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    drop(orch);

    let mut history = existing_history;
    history.push(user_msg);
    history.push(asst_msg.clone());
    // Keep last 100 messages only
    if history.len() > 100 {
        history = history[history.len() - 100..].to_vec();
    }

    if let Ok(history_json) = serde_json::to_string(&history) {
        let orch = state.orchestrator.lock().await;
        use vox_orchestrator::AgentId;
        orch.context().set(AgentId(0), history_key, &history_json, 0);
    }

    // 5. Return updated history + the new assistant message
    let result = serde_json::json!({
        "message": asst_msg,
        "history": history,
        "model_used": model_used,
        "tokens": tokens,
    });

    ToolResult::ok(result).to_json()
}

/// Return the full chat history for the default session.
pub async fn chat_history(state: &ServerState) -> String {
    let history_key = "chat_history:default";
    let orch = state.orchestrator.lock().await;
    let history: Vec<ChatMessage> = orch.context()
        .get(history_key)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    ToolResult::ok(history).to_json()
}

/// Perform an inline edit on a range in a file.
/// The editor sends the current text; Rust queries the LLM and returns the replacement.
pub async fn inline_edit(state: &ServerState, params: InlineEditParams) -> String {
    let language = params.language.as_deref().unwrap_or("text");
    let context_before = params.context_before.as_deref().unwrap_or("");
    let context_after = params.context_after.as_deref().unwrap_or("");

    let user_prompt = format!(
        r#"You are an expert {language} programmer. Edit the following code snippet as instructed.

INSTRUCTION: {prompt}

CONTEXT BEFORE (do not modify):
```{language}
{context_before}
```

CODE TO EDIT (lines {start_line}-{end_line} of file `{file}`):
```{language}
{current_text}
```

CONTEXT AFTER (do not modify):
```{language}
{context_after}
```

OUTPUT RULES:
- Output ONLY the replacement code for lines {start_line}-{end_line}.
- Do NOT include context_before or context_after.
- Do NOT wrap output in markdown fences — output raw code only.
- Preserve indentation consistent with context_before.
- Do NOT add placeholder comments or TODOs."#,
        prompt = params.prompt,
        file = params.file,
        start_line = params.start_line,
        end_line = params.end_line,
        current_text = params.current_text,
    );

    let system_prompt = format!(
        "You are an expert inline code editor. You output ONLY replacement code, no markdown fences, no explanation."
    );

    let orch = state.orchestrator.lock().await;
    let model = orch
        .models()
        .best_free_for(vox_orchestrator::types::TaskCategory::CodeGen)
        .or_else(|| orch.models().cheapest())
        .ok_or_else(|| "No models available".to_string());
    drop(orch);

    let model = match model {
        Ok(m) => m,
        Err(e) => return ToolResult::<String>::err(e).to_json(),
    };

    let (replacement, tokens) = match dispatch_to_provider(state, &model, &system_prompt, &user_prompt).await {
        Ok(r) => r,
        Err(e) => return ToolResult::<String>::err(format!("LLM error: {e}")).to_json(),
    };

    let result = InlineEditResult {
        replacement: replacement.trim().to_string(),
        explanation: format!("{}", params.prompt),
        tokens,
        model_used: model.id.clone(),
    };

    ToolResult::ok(result).to_json()
}

/// Generate a structured plan for a goal. Optionally writes PLAN.md to the workspace root.
/// This backs the Cursor-style "Planning Mode" in the extension and in Vox agents.
pub async fn plan_goal(state: &ServerState, params: PlanParams) -> String {
    let max_tasks = params.max_tasks.unwrap_or(30);
    let scope_note = if params.scope_files.is_empty() {
        String::new()
    } else {
        format!("\n\nScope this plan to these files:\n{}", params.scope_files.join("\n"))
    };

    let user_prompt = format!(
        r#"You are an expert software architect and planner.

GOAL: {goal}{scope_note}

Generate a comprehensive, ordered task list to achieve this goal. Follow this exact output format:

## Plan: [GOAL SUMMARY]

**Overall Summary**: [2-3 sentence summary of the approach]

### Tasks

1. **[Task Title]** — [files: path/to/file.rs, path/to/other.ts] [complexity: N/10]
   [One-sentence description of exactly what to implement. No placeholders.]

2. **[Task Title]** — [files: ...] [complexity: N/10] [depends: 1]
   [Description]

... (up to {max_tasks} tasks)

Rules:
- Every task must be atomic and independently verifiable.
- Mark complexity 1 (trivial edit) to 10 (full subsystem build).
- Use `depends: N,M` when a task requires prior tasks to be done first.
- If files are unknown, use `[files: TBD]`.
- Include test tasks explicitly.
- Do NOT include filler tasks like 'Review and refactor'."#,
        goal = params.goal,
    );

    let system_prompt = build_system_prompt(state);
    let (plan_md, model_used, _tokens) = match call_llm(state, &system_prompt, &user_prompt).await {
        Ok(r) => r,
        Err(e) => return ToolResult::<String>::err(format!("LLM error: {e}")).to_json(),
    };

    // Parse tasks from markdown (best-effort)
    let tasks = parse_plan_tasks(&plan_md);

    // Optionally write PLAN.md
    let written_to_disk = if params.write_to_disk {
        let plan_path = state
            .workspace_root
            .as_deref()
            .unwrap_or(std::path::Path::new("."))
            .join("PLAN.md");
        let header = format!(
            "# Vox Plan\n\n**Goal**: {}\n**Generated**: {}\n**Model**: {}\n\n",
            params.goal,
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            model_used,
        );
        let full = header + &plan_md;
        std::fs::write(&plan_path, &full).is_ok()
    } else {
        false
    };

    let result = PlanResult {
        goal: params.goal,
        tasks,
        summary: extract_summary(&plan_md),
        plan_md: plan_md.clone(),
        written_to_disk,
    };

    ToolResult::ok(result).to_json()
}

/// Parse a best-effort list of PlanTask structs from the LLM markdown output.
fn parse_plan_tasks(plan_md: &str) -> Vec<PlanTask> {
    let mut tasks = Vec::new();
    let task_re = regex::Regex::new(
        r"(?m)^(\d+)\.\s+\*\*(.+?)\*\*.*?\[files?:\s*([^\]]*)\].*?\[complexity:\s*(\d+)/10\](?:.*?\[depends?:\s*([^\]]*)\])?"
    ).unwrap();

    for cap in task_re.captures_iter(plan_md) {
        let id: usize = cap[1].parse().unwrap_or(tasks.len() + 1);
        let title = cap[2].trim().to_string();
        let files: Vec<String> = cap[3]
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty() && f != "TBD")
            .collect();
        let complexity: u8 = cap[4].parse().unwrap_or(5).min(10);
        let depends_on: Vec<usize> = cap
            .get(5)
            .map(|m| m.as_str())
            .unwrap_or("")
            .split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .collect();

        tasks.push(PlanTask {
            id,
            description: title,
            files,
            estimated_complexity: complexity,
            depends_on,
        });
    }

    tasks
}

/// Extract the summary from the plan markdown.
fn extract_summary(plan_md: &str) -> String {
    let summary_re = regex::Regex::new(r"\*\*Overall Summary\*\*:\s*(.+)").unwrap();
    summary_re
        .captures(plan_md)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "See plan for details.".to_string())
}
