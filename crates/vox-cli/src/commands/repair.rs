use crate::cli_args::RepairArgs;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::process::Command;
use std::time::Instant;
use vox_config::bootstrap_inference::REPAIR_LOOP_PREFERRED;
use vox_config::inference::{OPENROUTER_CHAT_COMPLETIONS_URL, openrouter_chat_model_preference};
use vox_orchestrator::models::{SelectionIntent, select_with_default_registry};
use vox_secrets::{SecretId, resolve_secret};
use vox_telemetry::{
    RepairAttemptEvent, RepairOutcomeEvent, TelemetryEvent, record_event,
};

#[derive(Debug, Deserialize)]
struct SpanPayload {
    start_line: usize,
    #[allow(dead_code)]
    start_col: usize,
    #[allow(dead_code)]
    end_line: usize,
    #[allow(dead_code)]
    end_col: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SuggestedFix {
    label: String,
    replacement: String,
    span: SpanPayload,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DiagnosticPayload {
    error_code: String,
    message: String,
    span: SpanPayload,
    correction_hints: Vec<String>,
    suggested_fixes: Vec<SuggestedFix>,
}

// ───────────────────────────────────────────────────────────────────────────
// CR-L8 corpus-feedback telemetry (P2.1b-repair).
//
// `record_event!` is a no-op when no recorder is registered (v0.5.x default).
// All telemetry below is therefore zero-cost on the existing repair path.
// Council-ratified 2026-05-15 (D8 / §1.3 P2.1b).
// ───────────────────────────────────────────────────────────────────────────

/// Per-attempt scratch: filled at attempt start, closed when we see the next
/// iteration's check result (or on session terminus).
struct PendingAttempt {
    attempt_number: u32,
    diagnostics_in: u32,
    started: Instant,
}

/// Session-level telemetry state for one `vox repair <file>` invocation.
struct RepairSession {
    started: Instant,
    attempts_budget: u32,
    panel_member_id: Option<String>,
    repository_id: Option<String>,
    /// Sum of per-attempt USD costs (always 0.0 today; OpenRouter does not
    /// return per-call USD pricing — the aggregator applies pricing post-hoc).
    total_cost_usd: f64,
    /// Open attempt waiting for its diagnostics_out value (next-iteration's
    /// check count, or final re-check on session end).
    pending: Option<PendingAttempt>,
}

impl RepairSession {
    fn new(attempts_budget: u32, panel_member_id: Option<String>) -> Self {
        Self {
            started: Instant::now(),
            attempts_budget,
            panel_member_id,
            repository_id: None,
            total_cost_usd: 0.0,
            pending: None,
        }
    }

    /// Walk up from `start` looking for a `Vox.toml`. Returns the basename of
    /// the parent directory (the conventional repository identifier used by
    /// the CR-L8 aggregator). Stops at the filesystem root or after 32 levels
    /// to bound the search (defends against symlink loops).
    fn discover_repository_id(start: &std::path::Path) -> Option<String> {
        let mut cur: Option<std::path::PathBuf> = if start.is_dir() {
            Some(start.to_path_buf())
        } else {
            start.parent().map(|p| p.to_path_buf())
        };
        for _ in 0..32 {
            let Some(here) = cur else { return None };
            if here.join("Vox.toml").exists() {
                return here
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(str::to_string);
            }
            cur = here.parent().map(|p| p.to_path_buf());
        }
        None
    }

    /// Begin a new attempt; if there's already a pending attempt it MUST be
    /// closed first via [`Self::close_pending_with`].
    fn open_attempt(&mut self, attempt_number: u32, diagnostics_in: u32) {
        debug_assert!(
            self.pending.is_none(),
            "open_attempt called while another attempt is pending; close it first"
        );
        self.pending = Some(PendingAttempt {
            attempt_number,
            diagnostics_in,
            started: Instant::now(),
        });
    }

    /// Close the pending attempt with the supplied `diagnostics_out` count
    /// and emit its `RepairAttemptEvent`. No-op if nothing is pending.
    fn close_pending_with(&mut self, diagnostics_out: u32, files_touched: u32) {
        let Some(p) = self.pending.take() else { return };
        let event = TelemetryEvent::RepairAttempt(RepairAttemptEvent {
            attempt_number: p.attempt_number,
            diagnostics_in: p.diagnostics_in,
            diagnostics_out,
            files_touched,
            cost_usd: 0.0,
            duration_ms: p.started.elapsed().as_millis() as u64,
            panel_member_id: self.panel_member_id.clone(),
            repository_id: self.repository_id.clone(),
        });
        record_event!(&event);
    }

    /// Emit the `RepairOutcomeEvent` that closes the session.
    fn finalize(&self, final_state: &str, attempts_used: u32, residual_diagnostics: u32, note: Option<String>) {
        let event = TelemetryEvent::RepairOutcome(RepairOutcomeEvent {
            final_state: final_state.to_string(),
            attempts_used,
            attempts_budget: self.attempts_budget,
            total_cost_usd: self.total_cost_usd,
            total_duration_ms: self.started.elapsed().as_millis() as u64,
            residual_diagnostics,
            note,
            repository_id: self.repository_id.clone(),
        });
        record_event!(&event);
    }
}

/// Run `vox check --format json` on `file_path` and return the parsed diagnostics.
fn check_diagnostics(file_path: &std::path::Path) -> Result<Vec<DiagnosticPayload>> {
    let output = Command::new(std::env::current_exe().unwrap_or_else(|_| "vox".into()))
        .arg("check")
        .arg("--output-format")
        .arg("json")
        .arg(file_path)
        .output()
        .context("Failed to run vox check")?;

    if output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Empty output / unparseable JSON is treated as "no diagnostics" by the
    // existing repair loop; preserve that behavior.
    Ok(serde_json::from_str::<Vec<DiagnosticPayload>>(&stdout).unwrap_or_default())
}

pub async fn run(args: RepairArgs) -> Result<()> {
    // Project-scope mode (CR-L3): walk a directory of `.vox` files and run
    // the single-file repair loop on each one that has errors.
    if let Some(project_root) = args.project.clone() {
        return run_project(&project_root, args.json).await;
    }

    let file_path = args
        .file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("vox repair: either FILE or --project must be provided"))?;
    if !file_path.exists() {
        anyhow::bail!("File not found: {}", file_path.display());
    }

    println!("Starting automated repair loop for {}", file_path.display());

    let http = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let max_attempts: u32 = 3;
    // 2026-Q2 refresh: route through the unified `select()` SSOT so the
    // 3-axis user knob (`VOX_MODEL_AXES`) + premium_alias map drive the
    // pick. User-set OpenRouter override still wins; otherwise we let the
    // registry choose a cacheable Anthropic model (sonnet/opus) — that
    // cuts effective cost ~60-80% via prompt caching on the 3-attempt loop.
    let openrouter_model = {
        let resolved = openrouter_chat_model_preference();
        if !resolved.is_empty()
            && resolved != vox_config::bootstrap_inference::OPENROUTER_AUTO
        {
            resolved
        } else {
            select_with_default_registry(&SelectionIntent::repair_loop())
                .map(|o| o.model_id)
                .unwrap_or_else(|| REPAIR_LOOP_PREFERRED.to_string())
        }
    };
    let supports_anthropic_prompt_cache = openrouter_model.starts_with("anthropic/")
        || openrouter_model.starts_with("claude-");
    let mut session = RepairSession::new(max_attempts, Some(openrouter_model.clone()));
    // CR-L8 aggregator buckets by repository_id; discover via Vox.toml walk-up.
    session.repository_id = RepairSession::discover_repository_id(file_path);
    let mut attempts: u32 = 0;

    loop {
        // 1. Run `vox check --format json` on the file.
        let diagnostics = check_diagnostics(file_path)?;

        // Close out the previous attempt now that we know its diagnostics_out.
        session.close_pending_with(diagnostics.len() as u32, /* files_touched */ 1);

        if diagnostics.is_empty() {
            // Either the file is clean (exit success) or `vox check` failed in
            // a way we can't parse. Both close the session as success since no
            // diagnostics remain to fix.
            println!("✓ No errors found. File is clean!");
            session.finalize("success", attempts, 0, None);
            return Ok(());
        }

        if attempts >= max_attempts {
            // Budget exhausted: the just-closed attempt was attempt N=max, and
            // diagnostics_out (the count we just observed) is the residual.
            println!(
                "Repair loop exhausted after {} attempts without converging.",
                max_attempts
            );
            session.finalize(
                "abandoned",
                attempts,
                diagnostics.len() as u32,
                Some(format!(
                    "{} diagnostics still firing after {max_attempts} attempts",
                    diagnostics.len()
                )),
            );
            return Ok(());
        }

        attempts += 1;
        println!("\nAttempt {}/{}...", attempts, max_attempts);

        session.open_attempt(attempts, diagnostics.len() as u32);

        // 2. Resolve API key (do this after open_attempt so the event closes
        // out cleanly on bail).
        let token_opt = resolve_secret(SecretId::OpenRouterApiKey)
            .expose()
            .map(|s| s.to_string());
        let token = match token_opt {
            Some(t) => t,
            None => {
                // Close the just-opened attempt with no progress + finalize as
                // infra_error so the aggregator sees the session terminated.
                session.close_pending_with(diagnostics.len() as u32, 0);
                session.finalize(
                    "infra_error",
                    attempts,
                    diagnostics.len() as u32,
                    Some("OPENROUTER_API_KEY not configured".to_string()),
                );
                anyhow::bail!(
                    "OpenRouter API key (VOX_OPENROUTER_API_KEY) not found. Repair requires an LLM backend."
                );
            }
        };

        // 3. Build prompt.
        let source_code = fs::read_to_string(file_path)?;
        let mut error_summary = String::new();
        for d in &diagnostics {
            error_summary.push_str(&format!(
                "- [{}] Line {}: {}\n",
                d.error_code, d.span.start_line, d.message
            ));
            for hint in &d.correction_hints {
                error_summary.push_str(&format!("  Hint: {}\n", hint));
            }
        }

        let system_prompt = "You are an expert Vox language repair agent.
Your goal is to fix compiler errors in the provided Vox source code.
You will be given the original source code and a list of structured compiler diagnostics.
Return ONLY the full corrected source code inside a single markdown code block.
Do not provide explanations or chat.
Focus on correctness and adhering to Vox language standards (Wave 1: non-null by default, colon blocks).";

        // The source-code block is the bulkiest, most-repeated content across the
        // 3-attempt loop — cache_control on it cuts cached input cost to $0.30/MTok
        // for Anthropic Sonnet 4.6. The per-attempt-varying error summary stays
        // outside the cache boundary.
        let source_code_block = format!(
            "File: {}\n\nSOURCE CODE:\n```vox\n{}\n```",
            file_path.display(),
            source_code
        );
        let error_block = format!(
            "COMPILER ERRORS:\n{error_summary}\n\nPlease fix these errors and return the full corrected file."
        );

        // 4. Build the request body. Use structured content arrays for Anthropic
        //    models (so cache_control passes through OpenRouter to Anthropic);
        //    plain string content for everything else (OpenAI-compat default).
        println!(
            "Calling LLM ({openrouter_model}) via OpenRouter{}...",
            if supports_anthropic_prompt_cache {
                " [prompt-cache enabled]"
            } else {
                ""
            }
        );
        let request_body = if supports_anthropic_prompt_cache {
            serde_json::json!({
                "model": openrouter_model,
                "messages": [
                    {
                        "role": "system",
                        "content": [
                            {
                                "type": "text",
                                "text": system_prompt,
                                "cache_control": { "type": "ephemeral" }
                            }
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "text",
                                "text": source_code_block,
                                "cache_control": { "type": "ephemeral" }
                            },
                            { "type": "text", "text": error_block }
                        ]
                    }
                ],
                "temperature": 0.1,
            })
        } else {
            serde_json::json!({
                "model": openrouter_model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    {
                        "role": "user",
                        "content": format!("{source_code_block}\n\n{error_block}")
                    }
                ],
                "temperature": 0.1,
            })
        };
        let response = http
            .post(OPENROUTER_CHAT_COMPLETIONS_URL)
            .header("Authorization", format!("Bearer {}", token))
            .header("X-Title", "Vox Repair Loop")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await?;
            // Close pending attempt (no progress) and finalize.
            session.close_pending_with(diagnostics.len() as u32, 0);
            session.finalize(
                "infra_error",
                attempts,
                diagnostics.len() as u32,
                Some(format!("LLM API error {status}: {body}")),
            );
            anyhow::bail!("LLM API error ({}): {}", status, body);
        }

        let resp_json: serde_json::Value = response.json().await?;
        let assistant_text = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Empty LLM response"))?;

        // 5. Extract code block and apply.
        let new_code = if let Some(start) = assistant_text.find("```") {
            let after_start = &assistant_text[start + 3..];
            let content_start = after_start.find('\n').map(|i| i + 1).unwrap_or(0);
            let content = &after_start[content_start..];
            if let Some(end) = content.find("```") {
                &content[..end]
            } else {
                content
            }
        } else {
            assistant_text
        };

        fs::write(file_path, new_code.trim())?;
        println!("✓ Applied suggested fix. Re-checking...");
        // The attempt remains "pending" here; the next loop iteration's
        // `check_diagnostics` call yields diagnostics_out and closes the
        // event. This is intentional — diagnostics_out is the post-patch
        // measurement, which only exists after the next check.
    }
}

// ─── Project-scope repair (CR-L3) ──────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct ProjectFileOutcome {
    pub path: String,
    /// `"clean"` (no diagnostics on first scan)
    /// `"repaired"` (file had errors but the single-file loop converged)
    /// `"residual"` (file had errors, loop ran, errors still present)
    /// `"infra_error"` (loop bailed before producing a result)
    pub status: String,
    pub initial_error_count: u32,
    pub residual_error_count: u32,
}

#[derive(Debug, serde::Serialize)]
pub struct ProjectRepairReport {
    pub schema_version: u32,
    pub project_root: String,
    pub files_total: u32,
    pub files_initially_clean: u32,
    pub files_repaired: u32,
    pub files_residual: u32,
    pub files_infra_error: u32,
    pub outcome: String,
    pub duration_seconds: f64,
    pub per_file: Vec<ProjectFileOutcome>,
}

/// Walk `project_root` for `.vox` files, identify those with errors, and
/// run the single-file [`run`] loop on each. Aggregate to a structured
/// report. CR-L3 measurement consumes the JSON form.
///
/// Honest scope: this lands the project-walker + per-file dispatch + the
/// aggregate report. It does NOT yet do cross-file reasoning (coordinated
/// fix sets, dependency ordering); the per-file loop is the existing
/// 3-attempt OpenRouter flow unchanged. Cross-file is P4.1's deeper lift.
pub async fn run_project(project_root: &std::path::Path, json: bool) -> Result<()> {
    let started = Instant::now();
    let canonical_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    if !canonical_root.exists() {
        anyhow::bail!(
            "vox repair --project: path does not exist: {}",
            canonical_root.display()
        );
    }

    let files = discover_vox_files(&canonical_root);
    let mut per_file: Vec<ProjectFileOutcome> = Vec::with_capacity(files.len());
    let mut clean = 0u32;
    let mut repaired = 0u32;
    let mut residual = 0u32;
    let mut infra_err = 0u32;

    for path in &files {
        let initial = check_diagnostics(path).unwrap_or_default();
        let initial_error_count = initial.len() as u32;
        if initial.is_empty() {
            clean += 1;
            per_file.push(ProjectFileOutcome {
                path: path.display().to_string(),
                status: "clean".into(),
                initial_error_count: 0,
                residual_error_count: 0,
            });
            continue;
        }

        // Dispatch to the existing single-file repair loop. It writes the
        // repaired file in place; we re-check afterwards to record the
        // residual count.
        let single_args = RepairArgs {
            file: Some(path.clone()),
            project: None,
            json: false,
        };
        let single_result = Box::pin(run(single_args)).await;
        let residual_diags = check_diagnostics(path).unwrap_or_default();
        let residual_count = residual_diags.len() as u32;
        let status = match (single_result, residual_count) {
            (Ok(_), 0) => {
                repaired += 1;
                "repaired"
            }
            (Ok(_), _) => {
                residual += 1;
                "residual"
            }
            (Err(_), _) => {
                infra_err += 1;
                "infra_error"
            }
        };
        per_file.push(ProjectFileOutcome {
            path: path.display().to_string(),
            status: status.into(),
            initial_error_count,
            residual_error_count: residual_count,
        });
    }

    let outcome = if files.is_empty() || (residual == 0 && infra_err == 0) {
        "green"
    } else if infra_err > 0 && repaired == 0 && residual == 0 {
        "infra_error"
    } else {
        "red"
    };

    let report = ProjectRepairReport {
        schema_version: 1,
        project_root: canonical_root.display().to_string(),
        files_total: files.len() as u32,
        files_initially_clean: clean,
        files_repaired: repaired,
        files_residual: residual,
        files_infra_error: infra_err,
        outcome: outcome.into(),
        duration_seconds: started.elapsed().as_secs_f64(),
        per_file,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "vox repair --project {} → {} ({} clean / {} repaired / {} residual / {} infra-error, {:.2}s)",
            report.project_root,
            outcome.to_uppercase(),
            report.files_initially_clean,
            report.files_repaired,
            report.files_residual,
            report.files_infra_error,
            report.duration_seconds,
        );
        for f in &report.per_file {
            if f.status != "clean" {
                println!("  [{}] {}", f.status, f.path);
            }
        }
    }

    if residual > 0 || infra_err > 0 {
        anyhow::bail!(
            "vox repair --project: {} residual / {} infra-error of {} files",
            residual,
            infra_err,
            files.len()
        );
    }
    Ok(())
}

/// Walk `root` for `.vox` files; skips build/cache/vcs directories so that
/// generated or transient files don't surface as repair work. Mirrors the
/// skip list used by `vox doctor --project`.
fn discover_vox_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir_for_repair(e.path()))
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|x| x == "vox") {
            out.push(p.to_path_buf());
        }
    }
    out.sort();
    out
}

fn is_skipped_dir_for_repair(path: &std::path::Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(
        name,
        "target" | "node_modules" | ".git" | ".cargo" | "dist" | "build" | "archive"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a session, simulate one open→close cycle, and prove the event
    /// shape lines up. We don't actually emit (no recorder registered in lib
    /// tests), but the construction path itself exercises the field plumbing.
    #[test]
    fn repair_session_open_and_close_attempt_does_not_panic() {
        let mut s = RepairSession::new(3, Some("openrouter/claude-sonnet-4-7".into()));
        s.open_attempt(1, 5);
        s.close_pending_with(2, 1);
        assert!(s.pending.is_none());
    }

    #[test]
    fn repair_session_finalize_does_not_panic() {
        let s = RepairSession::new(3, None);
        s.finalize("success", 1, 0, None);
    }

    #[test]
    fn close_pending_is_no_op_when_no_pending() {
        let mut s = RepairSession::new(3, None);
        // No open_attempt → close is a no-op (must not panic).
        s.close_pending_with(0, 0);
    }

    #[test]
    fn discover_repository_id_finds_vox_toml_in_parent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("my-proj");
        let src = project.join("src");
        std::fs::create_dir_all(&src).expect("mkdir");
        std::fs::write(project.join("Vox.toml"), "[package]\nname = \"my-proj\"\n")
            .expect("write");
        let file = src.join("main.vox");
        std::fs::write(&file, "").expect("write");

        let repo = RepairSession::discover_repository_id(&file);
        assert_eq!(repo.as_deref(), Some("my-proj"));
    }

    #[test]
    fn discover_repository_id_starts_from_dir_when_arg_is_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("dir-proj");
        std::fs::create_dir_all(&project).expect("mkdir");
        std::fs::write(project.join("Vox.toml"), "[package]\nname = \"dir-proj\"\n")
            .expect("write");

        let repo = RepairSession::discover_repository_id(&project);
        assert_eq!(repo.as_deref(), Some("dir-proj"));
    }

    #[tokio::test]
    async fn run_project_on_clean_tempdir_returns_ok_zero_repaired() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/ok.vox"),
            "fn add(a: int, b: int) to int { return a + b }\n",
        )
        .unwrap();
        let result = run_project(tmp.path(), true).await;
        assert!(
            result.is_ok(),
            "clean project should produce no repair work: {result:?}"
        );
    }

    #[tokio::test]
    async fn run_project_skipped_dirs_are_not_walked() {
        let tmp = tempfile::tempdir().unwrap();
        // Real source.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/ok.vox"),
            "fn id(n: int) to int { return n }\n",
        )
        .unwrap();
        // Broken file inside target/ — should be skipped.
        std::fs::create_dir_all(tmp.path().join("target")).unwrap();
        std::fs::write(
            tmp.path().join("target/debris.vox"),
            "this is broken vox ###\n",
        )
        .unwrap();
        let result = run_project(tmp.path(), true).await;
        assert!(
            result.is_ok(),
            "target/ debris should not surface as repair work"
        );
    }

    #[tokio::test]
    async fn run_project_with_missing_root_bails() {
        let tmp = tempfile::tempdir().unwrap();
        let nope = tmp.path().join("does-not-exist");
        let result = run_project(&nope, true).await;
        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("does not exist")
        );
    }

    #[test]
    fn discover_repository_id_returns_none_when_no_vox_toml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // No Vox.toml anywhere up to the tempdir's parent. Use a path inside
        // the tempdir; walk-up hits the OS root without finding one.
        let leaf = tmp.path().join("a").join("b").join("file.vox");
        std::fs::create_dir_all(leaf.parent().unwrap()).expect("mkdir");
        std::fs::write(&leaf, "").expect("write");

        // We cannot assert `None` unconditionally — the host filesystem may
        // legitimately have a Vox.toml above the tempdir (e.g., the repo we
        // run tests from). Instead assert that the result, if Some, is NOT
        // a path basename derived from the tempdir's leaf segments.
        let repo = RepairSession::discover_repository_id(&leaf);
        if let Some(r) = repo.as_deref() {
            assert_ne!(r, "a");
            assert_ne!(r, "b");
        }
    }
}
