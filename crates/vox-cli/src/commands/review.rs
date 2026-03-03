//! `vox review` — AI-powered code review command.
//!
//! Performs multi-layer code review:
//! 1. Static analysis (TOESTUB detectors)
//! 2. Context gathering (all files, or only git-diff changed files in --diff mode)
//! 3. LLM review via provider cascade (OpenRouter → OpenAI → Gemini → Ollama → Pollinations)
//! 4. Post-LLM verification and deduplication
//! 5. Optional PR comment posting (GitHub REST API)

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use owo_colors::OwoColorize;
use vox_toestub::rules::SourceFile;
use vox_toestub::scanner::Scanner;
use vox_toestub::{
    self, format_markdown, format_sarif, format_terminal, ReviewClient, ReviewFinding,
    ReviewOutputFormat, ReviewProvider, ReviewResult, Severity, ToestubConfig, ToestubEngine,
};

// ---------------------------------------------------------------------------
// Git diff helpers
// ---------------------------------------------------------------------------

/// A parsed diff for a single file.
///
/// Includes:
/// - `path` — the *new* path (post-rename), or the same path for modifications.
/// - `old_path` — the *old* path when the file was renamed/moved; `None` for modifications.
/// - `hunk` — the full unified diff hunk body.
/// - `first_hunk_start` — the 1-indexed line in the *new* file where the first hunk begins
///   (extracted from the `@@ -a,b +c,d @@` header). Zero if no hunk header is found.
struct FileDiff {
    path: PathBuf,
    old_path: Option<PathBuf>,
    hunk: String,
    first_hunk_start: usize,
}

/// Run `git diff` (staged + unstaged) and return per-file diff hunks.
/// If `base_ref` is Some, diffs against that ref (e.g. "HEAD", "main").
fn collect_git_diffs(base_ref: Option<&str>) -> Vec<FileDiff> {
    // Try staged + unstaged first; fall back to HEAD diff for CI
    let diff_arg = base_ref.unwrap_or("HEAD");
    let output = Command::new("git")
        .args(["diff", "--unified=5", diff_arg])
        .output();

    let raw = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            // Might be a fresh branch with no commits — try staged only
            let staged = Command::new("git")
                .args(["diff", "--unified=5", "--cached"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
            staged
        }
    };

    if raw.is_empty() {
        return Vec::new();
    }

    parse_git_diff_output(&raw)
}

/// Parse the `@@ -a[,b] +c[,d] @@` hunk header and return the new-file start line `c`.
fn parse_hunk_start(line: &str) -> usize {
    // Format: @@ -<old_start>[,<old_count>] +<new_start>[,<new_count>] @@[ section]
    let after_at = line.trim_start_matches('@').trim_start();
    // Find the '+' part
    if let Some(new_part) = after_at.split_whitespace().nth(1) {
        let num = new_part
            .trim_start_matches('+')
            .split(',')
            .next()
            .unwrap_or("0");
        return num.parse().unwrap_or(0);
    }
    0
}

/// Parse raw git diff output into per-file hunks.
///
/// Handles:
/// - Standard modifications (`diff --git a/foo b/foo`)
/// - Renames / moves (`rename from`/`rename to` headers)
/// - Hunk start-line extraction from `@@ -a,b +c,d @@` headers
#[allow(unused_assignments)] // macro resets value after last flush — intentional
fn parse_git_diff_output(raw: &str) -> Vec<FileDiff> {
    let mut diffs: Vec<FileDiff> = Vec::new();

    let mut current_path: Option<PathBuf> = None;
    let mut current_old_path: Option<PathBuf> = None;
    let mut current_hunk = String::new();
    let mut first_hunk_start: usize = 0;
    let mut seen_first_hunk = false;

    // Helper: flush the current file entry into `diffs`
    macro_rules! flush {
        () => {
            if let Some(p) = current_path.take() {
                if !current_hunk.is_empty() {
                    diffs.push(FileDiff {
                        path: p,
                        old_path: current_old_path.take(),
                        hunk: std::mem::take(&mut current_hunk),
                        first_hunk_start,
                    });
                } else {
                    current_old_path = None;
                }
            }
            first_hunk_start = 0;
            seen_first_hunk = false;
        };
    }

    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            flush!();
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if let Some(b_path) = parts.get(3) {
                current_path = Some(PathBuf::from(b_path.trim_start_matches("b/")));
            }
        } else if let Some(old) = line.strip_prefix("rename from ") {
            current_old_path = Some(PathBuf::from(old.trim()));
        } else if let Some(new) = line.strip_prefix("rename to ") {
            // The rename target is the authoritative new path
            current_path = Some(PathBuf::from(new.trim()));
        } else if let Some(src) = line.strip_prefix("copy from ") {
            // Copy: old_path = source being copied from
            current_old_path = Some(PathBuf::from(src.trim()));
        } else if let Some(dst) = line.strip_prefix("copy to ") {
            // Copy: path = the new copy destination
            current_path = Some(PathBuf::from(dst.trim()));
        } else if line.starts_with("@@ ") {
            if !seen_first_hunk {
                first_hunk_start = parse_hunk_start(line);
                seen_first_hunk = true;
            }
            if current_path.is_some() {
                current_hunk.push_str(line);
                current_hunk.push('\n');
            }
        } else if current_path.is_some()
            // Skip git metadata lines — not part of the actual diff content
            && !line.starts_with("index ")
            && !line.starts_with("--- ")
            && !line.starts_with("+++ ")
            && !line.starts_with("new file mode")
            && !line.starts_with("deleted file mode")
            && !line.starts_with("old mode ")
            && !line.starts_with("new mode ")
            && !line.starts_with("similarity index")
            && !line.starts_with("dissimilarity index")
            && !line.starts_with("copy from ")
            && !line.starts_with("copy to ")
        {
            current_hunk.push_str(line);
            current_hunk.push('\n');
        }
    }

    flush!();
    let _ = (current_old_path, first_hunk_start, seen_first_hunk);
    diffs
}

// ---------------------------------------------------------------------------
// GitHub PR comment helper
// ---------------------------------------------------------------------------

/// Post findings as a GitHub Pull Request review via the REST API.
///
/// Requires env vars:
///   GITHUB_TOKEN   — personal access token with repo scope
///   GITHUB_REPO    — "owner/repo"  (e.g. "brbrainerd/vox")
///   GITHUB_PR      — PR number (e.g. "42")
///   GITHUB_COMMIT  — The commit SHA the review is anchored to (GITHUB_SHA in Actions)
async fn post_github_pr_review(
    findings: &[ReviewFinding],
    static_findings: &[vox_toestub::Finding],
) -> anyhow::Result<()> {
    let token =
        std::env::var("GITHUB_TOKEN").map_err(|_| anyhow::anyhow!("GITHUB_TOKEN not set"))?;
    let repo = std::env::var("GITHUB_REPO")
        .map_err(|_| anyhow::anyhow!("GITHUB_REPO not set (format: owner/repo)"))?;
    let pr: u64 = std::env::var("GITHUB_PR")
        .map_err(|_| anyhow::anyhow!("GITHUB_PR not set"))?
        .parse()
        .map_err(|_| anyhow::anyhow!("GITHUB_PR must be a number"))?;
    let commit_sha = std::env::var("GITHUB_SHA")
        .or_else(|_| std::env::var("GITHUB_COMMIT"))
        .map_err(|_| anyhow::anyhow!("GITHUB_SHA or GITHUB_COMMIT not set"))?;

    let url = format!("https://api.github.com/repos/{}/pulls/{}/reviews", repo, pr);

    // Build comment list
    let mut comments: Vec<serde_json::Value> = Vec::new();

    for f in findings {
        let path = f.file.to_string_lossy().replace('\\', "/");
        let body = format!(
            "**[vox review | {}]** `{:?}` (confidence: {}%)\n\n{}\n{}",
            format!("{:?}", f.category).to_uppercase(),
            f.severity,
            f.confidence,
            f.message,
            f.suggestion
                .as_deref()
                .map(|s| format!("\n💡 **Suggestion:** {}", s))
                .unwrap_or_default()
        );

        if f.line > 0 {
            comments.push(serde_json::json!({
                "path": path,
                "line": f.line,
                "side": "RIGHT",
                "body": body
            }));
        }
    }

    // Also include static findings
    for f in static_findings {
        let path = f.file.to_string_lossy().replace('\\', "/");
        let body = format!(
            "**[vox review | STATIC]** `[{}]`\n\n{}{}",
            f.rule_id,
            f.message,
            f.suggestion
                .as_deref()
                .map(|s| format!("\n💡 **Suggestion:** {}", s))
                .unwrap_or_default()
        );
        if f.line > 0 {
            comments.push(serde_json::json!({
                "path": path,
                "line": f.line,
                "side": "RIGHT",
                "body": body
            }));
        }
    }

    let has_errors = findings.iter().any(|f| f.severity >= Severity::Error)
        || static_findings
            .iter()
            .any(|f| f.severity >= Severity::Error);

    let event = if has_errors {
        "REQUEST_CHANGES"
    } else {
        "COMMENT"
    };
    let summary_body = if findings.is_empty() && static_findings.is_empty() {
        "✅ **vox review**: No issues found.".to_string()
    } else {
        format!(
            "🔍 **vox review found {} AI finding(s) and {} static finding(s).**\n\nSee inline comments for details.",
            findings.len(),
            static_findings.len()
        )
    };

    let payload = serde_json::json!({
        "commit_id": commit_sha,
        "body": summary_body,
        "event": event,
        "comments": comments
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "vox-cli/0.1")
        .json(&payload)
        .send()
        .await?;

    if resp.status().is_success() {
        eprintln!("  {} PR review posted to {}", "✓".bright_green(), url);
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API error {}: {}", status, body);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// GitLab MR comment helper
// ---------------------------------------------------------------------------

/// Post findings as a GitLab Merge Request note via the REST API.
async fn post_gitlab_mr_review(
    findings: &[ReviewFinding],
    static_findings: &[vox_toestub::Finding],
) -> anyhow::Result<()> {
    let token =
        std::env::var("GITLAB_TOKEN").map_err(|_| anyhow::anyhow!("GITLAB_TOKEN not set"))?;
    let project_id =
        std::env::var("CI_PROJECT_ID").map_err(|_| anyhow::anyhow!("CI_PROJECT_ID not set"))?;
    let mr_iid = std::env::var("CI_MERGE_REQUEST_IID")
        .map_err(|_| anyhow::anyhow!("CI_MERGE_REQUEST_IID not set"))?;
    let api_url =
        std::env::var("CI_API_V4_URL").unwrap_or_else(|_| "https://gitlab.com/api/v4".to_string());

    let url = format!(
        "{}/projects/{}/merge_requests/{}/notes",
        api_url, project_id, mr_iid
    );

    let mut body = String::new();
    let has_errors = findings.iter().any(|f| f.severity >= Severity::Error)
        || static_findings
            .iter()
            .any(|f| f.severity >= Severity::Error);

    if findings.is_empty() && static_findings.is_empty() {
        body.push_str("✅ **vox review**: No issues found.\n");
    } else {
        body.push_str(&format!(
            "{} **vox review found {} AI finding(s) and {} static finding(s).**\n\n",
            if has_errors { "❌" } else { "⚠️" },
            findings.len(),
            static_findings.len()
        ));

        for f in findings {
            body.push_str(&format!(
                "- **`{:?}`** ({}) in `{}:{}` (confidence: {}%)\n  {}\n",
                f.severity,
                format!("{:?}", f.category).to_uppercase(),
                f.file.display(),
                f.line,
                f.confidence,
                f.message
            ));
            if let Some(ref s) = f.suggestion {
                body.push_str(&format!("  💡 *Suggestion:* {}\n", s));
            }
        }
    }

    let payload = serde_json::json!({ "body": body });
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("PRIVATE-TOKEN", token)
        .header("Content-Type", "application/json")
        .header("User-Agent", "vox-cli/0.1")
        .json(&payload)
        .send()
        .await?;

    if resp.status().is_success() {
        eprintln!("  {} GitLab MR review posted", "✓".bright_green());
    } else {
        let status = resp.status();
        let error_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitLab API error {}: {}", status, error_body);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run the `vox review` command.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    targets: &[PathBuf],
    model: Option<&str>,
    format: Option<&str>,
    severity: Option<&str>,
    free_only: bool,
    use_diff: bool,
    ci: bool,
    pr_comment: bool,
    diff_base: Option<&str>,
) -> anyhow::Result<()> {
    let targets = if targets.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        targets.to_vec()
    };

    let output_format =
        ReviewOutputFormat::parse(format.unwrap_or(if ci { "sarif" } else { "terminal" }));
    let min_severity = match severity.unwrap_or("warning") {
        "info" => Severity::Info,
        "error" => Severity::Error,
        "critical" => Severity::Critical,
        _ => Severity::Warning,
    };

    // ── Step 1: Print header ───────────────────────────────────────────────
    if output_format == ReviewOutputFormat::Terminal {
        eprintln!(
            "{}",
            "🔍 vox review — AI-powered code analysis"
                .bold()
                .bright_cyan()
        );
        eprintln!("{}", "─".repeat(55).dimmed());
    }

    // ── Step 2: Build the review client ───────────────────────────────────
    let client = build_client(model, free_only);

    if output_format == ReviewOutputFormat::Terminal {
        eprintln!(
            "  {} {}",
            "Provider:".dimmed(),
            client.primary_provider_name().bright_white()
        );
        if use_diff {
            eprintln!(
                "  {} {}",
                "Mode:".dimmed(),
                "incremental (git diff)".bright_white()
            );
        }
        eprintln!();
    }

    // ── Step 3: Collect git diffs (if --diff mode) ────────────────────────
    let diff_map: HashMap<PathBuf, String> = if use_diff {
        let diffs = collect_git_diffs(diff_base);
        if output_format == ReviewOutputFormat::Terminal {
            eprintln!(
                "  {} {} changed file(s) in diff",
                "📝".bright_blue(),
                diffs.len()
            );
            // Log renames so the user sees the mapping
            for d in &diffs {
                if let Some(ref old) = d.old_path {
                    eprintln!(
                        "    {} {} → {}",
                        "↻".bright_cyan(),
                        old.display(),
                        d.path.display()
                    );
                }
            }
        }
        diffs
            .into_iter()
            .map(|d| {
                // Prefix hunk with a comment indicating the first changed line so
                // the AI reviewer can anchor its line-number references correctly.
                let enriched_hunk = if d.first_hunk_start > 0 {
                    format!("[first changed line: {}]\n{}", d.first_hunk_start, d.hunk)
                } else {
                    d.hunk
                };
                (d.path, enriched_hunk)
            })
            .collect()
    } else {
        HashMap::new()
    };

    // ── Step 4: Run static analysis first (TOESTUB pass) ──────────────────
    if output_format == ReviewOutputFormat::Terminal {
        eprintln!("{}", "  [1/3] Running static analysis…".dimmed());
    }

    let static_config = ToestubConfig {
        roots: targets.clone(),
        min_severity,
        ..ToestubConfig::default()
    };
    let engine = ToestubEngine::new(static_config);
    let static_result = engine.run();

    if output_format == ReviewOutputFormat::Terminal && !static_result.findings.is_empty() {
        eprintln!(
            "  {} {} static issue(s) from TOESTUB",
            "⚡".bright_yellow(),
            static_result.findings.len()
        );
    }

    // ── Step 5: Collect source files for LLM review ───────────────────────
    if output_format == ReviewOutputFormat::Terminal {
        eprintln!("{}", "  [2/3] Gathering source files…".dimmed());
    }

    let all_files = collect_files(&targets);

    // In diff mode, restrict to only files that appear in the diff
    let files: Vec<SourceFile> = if use_diff && !diff_map.is_empty() {
        all_files
            .into_iter()
            .filter(|f| {
                diff_map.contains_key(&f.path) || diff_map.keys().any(|k| f.path.ends_with(k))
            })
            .collect()
    } else {
        all_files
    };

    if output_format == ReviewOutputFormat::Terminal {
        eprintln!("  {} {} file(s) to review", "📄".bright_blue(), files.len());
        eprintln!();
    }

    // ── Step 6: LLM review pass ────────────────────────────────────────────
    if output_format == ReviewOutputFormat::Terminal {
        eprintln!("{}", "  [3/3] Running AI review…".dimmed());
    }

    let mut all_ai_findings: Vec<ReviewFinding> = Vec::new();
    let mut total_tokens = 0usize;
    let mut provider_used = client.primary_provider_name().to_string();
    let mut files_reviewed = 0usize;

    for file in &files {
        // Get static findings for this specific file
        let file_static: Vec<_> = static_result
            .findings
            .iter()
            .filter(|f| f.file == file.path)
            .cloned()
            .collect();

        // Look for a diff hunk matching this file
        let diff_hunk = diff_map
            .get(&file.path)
            .or_else(|| {
                diff_map
                    .keys()
                    .find(|k| file.path.ends_with(*k))
                    .and_then(|k| diff_map.get(k))
            })
            .map(|s| s.as_str());

        let review_result = if use_diff && diff_hunk.is_some() {
            client
                .review_file_with_diff(file, &file_static, file.language, 6000, diff_hunk)
                .await
        } else {
            client
                .review_file(file, &file_static, file.language, 6000)
                .await
        };

        match review_result {
            Ok((findings, prov, tokens)) => {
                provider_used = prov;
                total_tokens += tokens;
                files_reviewed += 1;

                let filtered: Vec<ReviewFinding> = findings
                    .into_iter()
                    .filter(|f| f.severity >= min_severity)
                    .collect();

                if output_format == ReviewOutputFormat::Terminal && !filtered.is_empty() {
                    eprintln!(
                        "  {} {} — {} issue(s)",
                        "→".dimmed(),
                        file.path.display().to_string().bright_white(),
                        filtered.len().to_string().bright_yellow()
                    );
                }

                all_ai_findings.extend(filtered);
            }
            Err(e) => {
                if output_format == ReviewOutputFormat::Terminal {
                    eprintln!(
                        "  {} Skipping {} — {}",
                        "⚠".bright_yellow(),
                        file.path.display(),
                        e
                    );
                }
            }
        }
    }

    // Estimate cost (rough: $0.003 / 1k tokens for Claude-equivalent)
    let cost_estimate = (total_tokens as f64 / 1000.0) * 0.003;

    // ── Step 7: Combine static + AI findings ──────────────────────────────
    let result = ReviewResult {
        files_reviewed,
        provider_used: provider_used.clone(),
        findings: all_ai_findings,
        cost_estimate_usd: cost_estimate,
        tokens_used: total_tokens,
    };

    // ── Step 8: Format and print ───────────────────────────────────────────
    if output_format == ReviewOutputFormat::Terminal {
        eprintln!();
        // Print static analysis section if any
        if !static_result.findings.is_empty() {
            println!(
                "{}",
                "── Static Analysis (TOESTUB) ──────────────────────────".dimmed()
            );
            for f in &static_result.findings {
                let icon = match f.severity {
                    Severity::Critical => "🔴",
                    Severity::Error => "🟠",
                    Severity::Warning => "🟡",
                    Severity::Info => "🔵",
                };
                println!(
                    "  {} [{}] {}:{} — {}",
                    icon,
                    f.rule_id.bright_white(),
                    f.file.display(),
                    f.line,
                    f.message
                );
            }
            println!();
        }

        println!(
            "{}",
            "── AI Review ─────────────────────────────────────────────".dimmed()
        );
        print!("{}", format_terminal(&result));
    } else if output_format == ReviewOutputFormat::Sarif {
        // SARIF: merge static findings in as well
        let mut merged_findings = result.findings.clone();
        merged_findings.extend(static_result.findings.iter().map(|f| ReviewFinding {
            category: vox_toestub::ReviewCategory::DeadCode,
            severity: f.severity,
            file: f.file.clone(),
            line: f.line,
            message: format!("[{}] {}", f.rule_id, f.message),
            suggestion: f.suggestion.clone(),
            confidence: 100,
        }));
        let merged = ReviewResult {
            files_reviewed: result.files_reviewed,
            provider_used: result.provider_used.clone(),
            findings: merged_findings,
            cost_estimate_usd: result.cost_estimate_usd,
            tokens_used: result.tokens_used,
        };
        println!("{}", format_sarif(&merged));
    } else if output_format == ReviewOutputFormat::Markdown {
        println!("{}", format_markdown(&result));
    } else {
        // JSON: output as JSON array
        let all = serde_json::json!({
            "static": static_result.findings,
            "ai": result.findings,
            "files_reviewed": result.files_reviewed,
            "tokens_used": result.tokens_used,
            "cost_usd": result.cost_estimate_usd,
            "provider": result.provider_used
        });
        println!("{}", serde_json::to_string_pretty(&all).unwrap_or_default());
    }

    // ── Step 9: Post PR review comment ────────────────────────────────────
    if pr_comment {
        if std::env::var("GITHUB_PR").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
            if let Err(e) = post_github_pr_review(&result.findings, &static_result.findings).await {
                eprintln!(
                    "  {} Failed to post GitHub PR review: {}",
                    "⚠".bright_yellow(),
                    e
                );
            }
        } else if std::env::var("CI_MERGE_REQUEST_IID").is_ok()
            || std::env::var("GITLAB_CI").is_ok()
        {
            if let Err(e) = post_gitlab_mr_review(&result.findings, &static_result.findings).await {
                eprintln!(
                    "  {} Failed to post GitLab MR review: {}",
                    "⚠".bright_yellow(),
                    e
                );
            }
        } else {
            eprintln!(
                "  {} --pr-comment requires CI environment variables (GitHub or GitLab).",
                "⚠".bright_yellow()
            );
        }
    }

    // ── Step 10: Exit code ─────────────────────────────────────────────────
    let has_errors = result.has_errors() || static_result.has_errors();
    if has_errors && (ci || output_format != ReviewOutputFormat::Terminal) {
        anyhow::bail!("vox review found error-level issues.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Client builder
// ---------------------------------------------------------------------------

/// Build the review client from flags.
fn build_client(model: Option<&str>, free_only: bool) -> ReviewClient {
    if let Some(m) = model {
        let provider = infer_provider_from_model(m);
        ReviewClient::new(vec![provider])
    } else if free_only {
        ReviewClient::free_only()
    } else {
        ReviewClient::auto()
    }
}

/// Infer the review provider from a model identifier string.
fn infer_provider_from_model(model: &str) -> ReviewProvider {
    if let Some(m) = model.strip_prefix("openrouter:") {
        ReviewProvider::OpenRouter {
            api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            model: m.to_string(),
            site_url: "https://github.com/brbrainerd/vox".to_string(),
        }
    } else if let Some(m) = model.strip_prefix("openai:") {
        ReviewProvider::OpenAi {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: m.to_string(),
            base_url: std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
        }
    } else if let Some(m) = model.strip_prefix("ollama:") {
        ReviewProvider::Ollama {
            url: std::env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            model: m.to_string(),
        }
    } else if let Some(m) = model.strip_prefix("gemini:") {
        ReviewProvider::Gemini {
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            model: m.to_string(),
        }
    } else if model.contains('/') {
        // "anthropic/claude-3.5-sonnet" style → OpenRouter
        ReviewProvider::OpenRouter {
            api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            model: model.to_string(),
            site_url: "https://github.com/brbrainerd/vox".to_string(),
        }
    } else {
        ReviewProvider::OpenAi {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: model.to_string(),
            base_url: std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
        }
    }
}

/// Collect all source files from the given targets.
fn collect_files(targets: &[PathBuf]) -> Vec<SourceFile> {
    let scanner = Scanner::new(targets.to_vec(), &[], None);
    scanner.scan()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1234567..89abcdef 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn foo() {
+    println!("hello");
 }
"#;
        let diffs = parse_git_diff_output(diff);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].path, PathBuf::from("src/lib.rs"));
        assert!(diffs[0].hunk.contains("+    println!(\"hello\");"));
    }

    #[test]
    fn test_parse_multi_file_diff() {
        let diff = r#"diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1 +1 @@
-old
+new
diff --git b/b.rs b/b.rs
--- b/b.rs
+++ b/b.rs
@@ -1 +1 @@
-foo
+bar
"#;
        let diffs = parse_git_diff_output(diff);
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].path, PathBuf::from("a.rs"));
        assert_eq!(diffs[1].path, PathBuf::from("b.rs"));
    }

    #[test]
    fn test_infer_provider() {
        // OpenRouter prefix
        match infer_provider_from_model("openrouter:anthropic/claude-3.5-sonnet") {
            ReviewProvider::OpenRouter { model, .. } => {
                assert_eq!(model, "anthropic/claude-3.5-sonnet")
            }
            _ => panic!("expected OpenRouter for openrouter: prefix"),
        }

        // Slash notation (defaults to OpenRouter)
        match infer_provider_from_model("mistralai/mistral-large") {
            ReviewProvider::OpenRouter { model, .. } => {
                assert_eq!(model, "mistralai/mistral-large")
            }
            _ => panic!("expected OpenRouter for slash notation"),
        }

        // OpenAI prefix
        match infer_provider_from_model("openai:gpt-4o") {
            ReviewProvider::OpenAi { model, .. } => assert_eq!(model, "gpt-4o"),
            _ => panic!("expected OpenAI for openai: prefix"),
        }

        // Ollama prefix
        match infer_provider_from_model("ollama:llama3") {
            ReviewProvider::Ollama { model, .. } => assert_eq!(model, "llama3"),
            _ => panic!("expected Ollama for ollama: prefix"),
        }

        // Gemini prefix
        match infer_provider_from_model("gemini:gemini-1.5-pro") {
            ReviewProvider::Gemini { model, .. } => assert_eq!(model, "gemini-1.5-pro"),
            _ => panic!("expected Gemini for gemini: prefix"),
        }
    }

    #[test]
    fn test_parse_rename_diff() {
        let diff = r#"diff --git a/old/utils.rs b/new/utils.rs
similarity index 85%
rename from old/utils.rs
rename to new/utils.rs
index 1234567..89abcdef 100644
--- a/old/utils.rs
+++ b/new/utils.rs
@@ -10,3 +10,4 @@
 fn helper() {
+    // improved
 }
"#;
        let diffs = parse_git_diff_output(diff);
        assert_eq!(diffs.len(), 1);
        assert_eq!(
            diffs[0].path,
            PathBuf::from("new/utils.rs"),
            "path should be the rename target"
        );
        assert_eq!(
            diffs[0].old_path.as_deref(),
            Some(std::path::Path::new("old/utils.rs"))
        );
    }

    #[test]
    fn test_hunk_start_line_extraction() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1234567..89abcdef 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -42,6 +45,8 @@ fn existing_fn() {
 fn foo() {
+    println!("hello");
 }
"#;
        let diffs = parse_git_diff_output(diff);
        assert_eq!(diffs.len(), 1);
        assert_eq!(
            diffs[0].first_hunk_start, 45,
            "should extract +45 from hunk header"
        );
    }

    #[test]
    fn test_metadata_lines_filtered() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index abcdef1..1234567 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    dbg!("test");
 }
"#;
        let diffs = parse_git_diff_output(diff);
        assert_eq!(diffs.len(), 1);
        // The hunk should NOT contain index, ---, or +++ lines
        assert!(!diffs[0].hunk.contains("index abcdef1"));
        assert!(!diffs[0].hunk.contains("--- a/src/main.rs"));
        assert!(!diffs[0].hunk.contains("+++ b/src/main.rs"));
        // But it should contain the actual diff content
        assert!(diffs[0].hunk.contains("+    dbg!(\"test\");"));
    }

    #[test]
    fn test_parse_hunk_start_helper() {
        assert_eq!(parse_hunk_start("@@ -1,3 +1,4 @@"), 1);
        assert_eq!(parse_hunk_start("@@ -10,6 +45,8 @@ fn existing_fn() {"), 45);
        assert_eq!(parse_hunk_start("@@ -0,0 +1 @@"), 1);
        assert_eq!(parse_hunk_start("@@ -100 +200 @@"), 200);
    }

    #[test]
    fn test_parse_copy_diff() {
        let diff = r#"diff --git a/src/shared.rs b/src/new_copy.rs
similarity index 90%
copy from src/shared.rs
copy to src/new_copy.rs
index abcdef1..1234567 100644
--- a/src/shared.rs
+++ b/src/new_copy.rs
@@ -1,3 +1,4 @@
 struct Shared {}
+impl Shared { fn new() -> Self { Self {} } }
"#;
        let diffs = parse_git_diff_output(diff);
        assert_eq!(diffs.len(), 1);
        assert_eq!(
            diffs[0].path,
            PathBuf::from("src/new_copy.rs"),
            "path should be the copy destination"
        );
        assert_eq!(
            diffs[0].old_path.as_deref(),
            Some(std::path::Path::new("src/shared.rs")),
            "old_path should be the copy source"
        );
        // copy from/to lines should NOT appear in the hunk body
        assert!(!diffs[0].hunk.contains("copy from"));
        assert!(!diffs[0].hunk.contains("copy to"));
    }
}
