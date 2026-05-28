//! CLI surface for **`vox oratio`** (speech-to-text).

use anyhow::{Context as _, Result};
use clap::Parser;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Transcribe an audio file via the `oratio` plugin + deterministic refinement.
/// Falls back to `vox_oratio::transcribe_path_detailed` for .txt/.md files.
fn transcribe_path_via_plugin(
    path: &Path,
    ctx: &vox_oratio::refine::CorrectionContext,
    language_hint: Option<&str>,
) -> anyhow::Result<vox_oratio::TranscribeDetail> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if matches!(ext.as_str(), "txt" | "md") {
        return vox_oratio::transcribe_path_detailed(path, ctx, language_hint)
            .map_err(|e| anyhow::anyhow!("{e}"));
    }

    let plugin = vox_plugin_host::cached_code_plugin("oratio")
        .map_err(|e| anyhow::anyhow!("oratio plugin load: {e}"))?;
    let stt = plugin
        .plugin
        .as_speech_to_text()
        .into_option()
        .ok_or_else(|| anyhow::anyhow!("oratio plugin missing SpeechToText accessor"))?;

    let path_str = path.to_string_lossy().to_string();
    let config_json = serde_json::json!({ "language": language_hint }).to_string();

    let transcription_json = stt
        .transcribe_path(path_str.as_str().into(), config_json.as_str().into())
        .into_result()
        .map_err(|e| anyhow::anyhow!("transcribe_path plugin: {e}"))?;

    let v: serde_json::Value = serde_json::from_str(transcription_json.as_str())
        .map_err(|e| anyhow::anyhow!("plugin returned invalid JSON: {e}"))?;
    let raw_text = v
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(vox_oratio::refine_raw_text(&raw_text, ctx))
}

/// Wrapper returning `Transcript` shape for simple transcribe commands.
fn transcribe_path_simple_via_plugin(path: &Path) -> anyhow::Result<vox_oratio::Transcript> {
    let ctx = vox_oratio::refine::CorrectionContext::default();
    let d = transcribe_path_via_plugin(path, &ctx, None)?;
    Ok(vox_oratio::Transcript {
        raw_text: d.raw_text,
        refined_text: Some(d.refined_text),
    })
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CorrectionProfileCli {
    Conservative,
    Balanced,
    Aggressive,
}

impl From<CorrectionProfileCli> for vox_oratio::refine::OratioCorrectionProfile {
    fn from(value: CorrectionProfileCli) -> Self {
        match value {
            CorrectionProfileCli::Conservative => Self::Conservative,
            CorrectionProfileCli::Balanced => Self::Balanced,
            CorrectionProfileCli::Aggressive => Self::Aggressive,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum RouteModeCli {
    None,
    Tool,
    Chat,
    Orchestrator,
}

fn wait_for_enter_or_timeout(timeout_ms: u64, heartbeat_ms: u64) -> Result<bool> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        let _ = tx.send(());
    });

    let timeout = Duration::from_millis(timeout_ms);
    let hb = Duration::from_millis(heartbeat_ms.max(500));
    let start = Instant::now();
    loop {
        if rx.recv_timeout(hb).is_ok() {
            return Ok(true);
        }
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return Ok(false);
        }
        let remaining_ms = timeout.saturating_sub(elapsed).as_millis();
        tracing::info!(
            target: "vox_oratio_cli",
            remaining_ms,
            "Waiting for Enter; session heartbeat"
        );
    }
}

fn append_asr_refine_pair(
    path: &std::path::Path,
    noisy_text: &str,
    corrected_text: &str,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid asr_refine output path: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    use std::io::Write;
    writeln!(
        f,
        "{}",
        serde_json::to_string(&serde_json::json!({
            "noisy_text": noisy_text,
            "corrected_text": corrected_text,
        }))?
    )?;
    Ok(())
}

impl From<RouteModeCli> for vox_oratio::RouteMode {
    fn from(value: RouteModeCli) -> Self {
        match value {
            RouteModeCli::None => Self::None,
            RouteModeCli::Tool => Self::Tool,
            RouteModeCli::Chat => Self::Chat,
            RouteModeCli::Orchestrator => Self::Orchestrator,
        }
    }
}

fn resolve_ide_context() -> vox_oratio::routing::IdeContext {
    let mut ctx = vox_oratio::routing::IdeContext::default();

    // 1. IDE State File (highest stable precedence)
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let repo = vox_repository::discover_repository_or_fallback(&cwd);
    let state_file = repo.root.join(".vox").join("ide_state.json");
    if state_file.is_file() {
        let meta = state_file.metadata();
        let is_fresh = meta
            .and_then(|m| m.modified())
            .map(|mt| mt.elapsed().map(|e| e.as_secs() < 300).unwrap_or(false))
            .unwrap_or(false);

        if is_fresh {
            if let Ok(content) = std::fs::read_to_string(&state_file) {
                if let Ok(file_ctx) =
                    serde_json::from_str::<vox_oratio::routing::IdeContext>(&content)
                {
                    ctx = file_ctx;
                }
            }
        }
    }

    // 2. Environment Variables (session-scoped overrides)
    if let Ok(f) = std::env::var("VOX_ACTIVE_FILE") {
        ctx.active_file = Some(f);
    } else if let Ok(f) = std::env::var("ACTIVE_FILE") {
        ctx.active_file = Some(f);
    }
    if let Ok(l) = std::env::var("VOX_ACTIVE_LINE") {
        ctx.cursor_line = l.parse::<usize>().ok();
    }
    if let Ok(s) = std::env::var("VOX_ACTIVE_SYMBOL") {
        ctx.symbol_stack.push(s);
    }

    // Best-effort pull of build errors from vox-db
    if let Ok(rt) = tokio::runtime::Runtime::new() {
        if let Ok(db) =
            rt.block_on(async { crate::workspace_db::connect_cli_workspace_voxdb().await })
        {
            // Use repository_id if available, otherwise "workspace"
            let repo_id =
                std::env::var("VOX_REPOSITORY_ID").unwrap_or_else(|_| "workspace".to_string());
            if let Ok(warnings) = rt.block_on(async { db.query_build_warnings(&repo_id, 3).await })
            {
                for w in warnings {
                    ctx.recent_errors
                        .push(format!("[{}] {}", w.crate_name, w.message));
                }
            }
        }
    }

    ctx
}

/// Subcommands for Oratio (STT / transcripts).
#[derive(Parser, Debug)]
pub enum OratioAction {
    /// Transcribe a file to text (native STT when enabled; `.txt`/`.md` fixtures always)
    Transcribe {
        /// Audio or transcript fixture path
        path: PathBuf,
        /// Print JSON instead of plain text
        #[arg(long, default_value = "false")]
        json: bool,
        /// Emit refined text when available (default: yes)
        #[arg(long, default_value = "true")]
        refined: bool,
    },
    /// Sessionized transcript workflow: Enter-to-confirm or timeout, then optional route loop.
    Listen {
        /// Audio sample path to transcribe
        path: PathBuf,
        /// Wait this long for Enter before timing out (ms)
        #[arg(long, default_value_t = 12_000)]
        timeout_ms: u64,
        /// Hard cap for whole session duration (ms)
        #[arg(long, default_value_t = 120_000)]
        max_ms: u64,
        /// Stricter cap for transcription + refine only (ms); 0 = use runtime / max_ms
        #[arg(long, default_value_t = 0)]
        inference_deadline_ms: u64,
        /// Optional language hint
        #[arg(long)]
        language: Option<String>,
        /// Correction strictness profile
        #[arg(long, value_enum, default_value_t = CorrectionProfileCli::Balanced)]
        profile: CorrectionProfileCli,
        /// Session route mode after transcript
        #[arg(long, value_enum, default_value_t = RouteModeCli::None)]
        route: RouteModeCli,
        /// Print JSON instead of plain text
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Emit parser payload debug logs
        #[arg(long, default_value_t = false)]
        debug_parser_payload: bool,
        /// Require explicit Enter confirmation before route execution
        #[arg(long, default_value_t = true)]
        confirm_route: bool,
        /// Optional JSONL output path for `asr_refine` pair collection
        #[arg(long)]
        emit_asr_refine: Option<PathBuf>,
    },
    /// Record the default microphone to a WAV file, then run native STT (requires `--features oratio-mic`).
    #[cfg(feature = "oratio-mic")]
    RecordTranscribe {
        /// Seconds of audio to capture (default ~8s; max 300).
        #[arg(long, default_value_t = 8.0)]
        seconds: f32,
        /// Save the WAV here; default is a temp file under the system temp directory.
        #[arg(long)]
        output_wav: Option<PathBuf>,
        /// Print JSON instead of plain text
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Emit refined transcript text when available
        #[arg(long, default_value_t = true)]
        refined: bool,
    },
    /// Diagnose current Oratio runtime and configuration health.
    Doctor,
    /// Show which Oratio backends and passthrough modes are available
    Status,
    /// Evaluate a JSONL dataset of expected transcripts to calculate WER.
    Eval {
        /// JSONL file with {"path": "...", "expected": "...", "language": "..."}
        dataset: std::path::PathBuf,
        /// Limit number of samples to evaluate
        #[arg(long)]
        limit: Option<usize>,
        /// Write metrics to VoxDB for long-term trending
        #[arg(long, default_value_t = false)]
        persist: bool,
    },
    /// View recent ASR evaluation runs.
    EvalHistory {
        /// Limit number of runs to display
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
    /// Generate an SRT subtitle from a video or audio file.
    Subtitle {
        /// Path to audio or video input file.
        path: String,
        /// Explicit output path (defaults to beside input with .srt extension).
        #[arg(short, long)]
        output: Option<String>,
        /// Optional explicit ISO language code constraint (e.g. `en`).
        #[arg(long)]
        language: Option<String>,
        /// Max characters per line.
        #[arg(long, default_value_t = 42)]
        line_width: usize,
        /// Max lines per subtitle block.
        #[arg(long, default_value_t = 2)]
        max_lines: usize,
        /// Optional ground truth SRT for WER calculation.
        #[arg(long)]
        ground_truth_srt: Option<String>,
        /// Write metrics to VoxDB.
        #[arg(long, default_value_t = false)]
        persist: bool,
    },
    /// Start a local STT serve worker for cloud mesh offloading.
    Serve {
        /// Port to bind to (0 for OS-assigned)
        #[arg(long)]
        port: u16,
    },
}

/// Run **`vox oratio …`**.
pub async fn run(action: OratioAction, global_json: bool) -> Result<()> {
    let runtime = vox_oratio::resolved_runtime_config();
    match action {
        OratioAction::Transcribe {
            path,
            json,
            refined,
        } => {
            let use_json = json || global_json;
            let t = transcribe_path_simple_via_plugin(&path)?;
            let text = if refined {
                t.display_text().to_string()
            } else {
                t.raw_text.clone()
            };
            if use_json {
                let payload = serde_json::json!({
                    "path": path,
                    "raw_text": t.raw_text,
                    "refined_text": t.refined_text,
                    "text": text,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("{text}");
            }
            Ok(())
        }
        #[cfg(feature = "oratio-mic")]
        OratioAction::RecordTranscribe {
            seconds,
            output_wav,
            json,
            refined,
        } => {
            let use_json = json || global_json;
            let (wav_path, delete_after) = match output_wav {
                Some(p) => (p, false),
                None => (
                    std::env::temp_dir()
                        .join(format!("vox_oratio_mic_{}.wav", uuid::Uuid::new_v4())),
                    true,
                ),
            };
            crate::commands::oratio_mic::record_default_input_wav(&wav_path, seconds)?;
            let t = transcribe_path_simple_via_plugin(&wav_path)?;
            let text = if refined {
                t.display_text().to_string()
            } else {
                t.raw_text.clone()
            };
            if delete_after {
                let _ = std::fs::remove_file(&wav_path);
            }
            if use_json {
                let payload = serde_json::json!({
                    "path": wav_path,
                    "raw_text": t.raw_text,
                    "refined_text": t.refined_text,
                    "text": text,
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("{text}");
            }
            Ok(())
        }
        OratioAction::Listen {
            path,
            timeout_ms,
            max_ms,
            inference_deadline_ms,
            language,
            profile,
            route,
            json,
            debug_parser_payload,
            confirm_route,
            emit_asr_refine,
        } => {
            let use_json = json || global_json;
            let heartbeat_ms = runtime.session_timing.heartbeat_ms.max(500);
            println!(
                "Provide audio sample at {}. Press Enter to run transcription, waiting up to {} ms...",
                path.display(),
                timeout_ms
            );
            let confirmed = wait_for_enter_or_timeout(timeout_ms, heartbeat_ms)?;
            if !confirmed {
                anyhow::bail!("oratio_capture_timeout: no Enter received within {timeout_ms} ms");
            }
            let session = vox_oratio::transcribe_path_session_with_runtime(
                &path,
                &vox_oratio::OratioSessionConfig {
                    timeout_ms,
                    max_duration_ms: max_ms,
                    inference_deadline_ms: (inference_deadline_ms > 0)
                        .then_some(inference_deadline_ms),
                    language_hint: language,
                    correction_profile: profile.into(),
                    debug_parser_payload,
                    heartbeat_ms,
                    session_id: None,
                },
                runtime,
            )?;
            if let Some(out_path) = emit_asr_refine {
                append_asr_refine_pair(&out_path, &session.raw_text, &session.refined_text)?;
            }
            if confirm_route {
                println!(
                    "Transcript preview:\n{}\nPress Enter to execute route, waiting up to {} ms...",
                    session.text, timeout_ms
                );
                let route_confirmed = wait_for_enter_or_timeout(timeout_ms, heartbeat_ms)?;
                if !route_confirmed {
                    anyhow::bail!(
                        "oratio_capture_timeout: route confirm gate timed out after {timeout_ms} ms"
                    );
                }
            }
            let mut ctx = resolve_ide_context();

            // Best-effort global symbol matching based on transcript keywords
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                if let Ok(db) =
                    rt.block_on(async { crate::workspace_db::connect_cli_workspace_voxdb().await })
                {
                    // Extract potential symbol names from transcript
                    let keywords: Vec<&str> = session
                        .text
                        .split_whitespace()
                        .filter(|w| w.len() > 3) // Ignore short words
                        .collect();
                    for k in keywords {
                        let clean = k.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                        if !clean.is_empty() {
                            if let Ok(symbols) =
                                rt.block_on(async { db.search_project_symbols(clean, 3).await })
                            {
                                for (_, label, _) in symbols {
                                    if !ctx.symbol_stack.contains(&label) {
                                        ctx.symbol_stack.push(label);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let mut route_payload = vox_oratio::route_transcript_with_options(
                route.into(),
                &session.session_id,
                &session.text,
                session.confidence,
                runtime,
                &ctx,
            );

            if matches!(route_payload.mode, vox_oratio::routing::RouteMode::Clarify) {
                println!(
                    "{}",
                    route_payload.payload["message"]
                        .as_str()
                        .unwrap_or("Clarification needed:")
                );
                if let Some(options) = route_payload.payload["options"].as_array() {
                    let items: Vec<String> = options
                        .iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect();
                    if let Ok(selection) = dialoguer::Select::new()
                        .with_prompt("Choose an option")
                        .items(&items)
                        .default(0)
                        .interact()
                    {
                        println!("Selected: {}", items[selection]);
                        // Recalculate route with the specific target identified by the user
                        // For now, we just upgrade to matched with the selected text
                        route_payload.mode = vox_oratio::routing::RouteMode::Tool;
                        route_payload.status = "clarified".to_string();
                        route_payload.payload = serde_json::json!({
                            "selected": items[selection],
                            "note": "User manually clarified the ambiguous target"
                        });
                    }
                }
            }
            if use_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "session": session,
                        "route": route_payload,
                    }))?
                );
            } else {
                println!("{}", session.text);
                println!("{}", serde_json::to_string_pretty(&route_payload.payload)?);
            }
            Ok(())
        }
        OratioAction::Doctor => {
            let mut checks = Vec::new();
            checks.push(serde_json::json!({
                "name": "status_summary",
                "ok": true,
                "value": vox_oratio::transcript_status(),
            }));
            let candle = vox_oratio::candle_backend_status_json();
            checks.push(serde_json::json!({
                "name": "candle_backend",
                "ok": true,
                "value": candle,
            }));
            checks.push(serde_json::json!({
                "name": "runtime_config",
                "ok": true,
                "value": vox_oratio::runtime_config_diagnostic_json(runtime),
            }));
            checks.push(serde_json::json!({
                "name": "ide_context",
                "ok": true,
                "value": resolve_ide_context(),
            }));
            let payload = serde_json::json!({
                "checks": checks,
                "result": "ok"
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
            Ok(())
        }
        OratioAction::Status => {
            println!("{}", vox_oratio::transcript_status());
            println!(
                "{}",
                serde_json::to_string_pretty(&vox_oratio::candle_backend_status_json())?
            );
            Ok(())
        }
        OratioAction::Eval {
            dataset,
            limit,
            persist,
        } => {
            let file = std::fs::read_to_string(&dataset)?;
            let mut total_words = 0;
            let mut total_errors = 0;
            let mut count = 0;

            let db_opt = if persist {
                crate::workspace_db::connect_cli_workspace_voxdb()
                    .await
                    .ok()
            } else {
                None
            };
            let run_id = uuid::Uuid::new_v4().to_string();

            if let Some(ref db) = db_opt {
                let ds_name = dataset
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let params = vox_db::OratioEvalRunStartParams {
                    run_id: run_id.clone(),
                    run_type: "general_subtitle".to_string(),
                    backend: "candle-whisper".to_string(),
                    model_id: None,
                    dataset_name: ds_name,
                };
                let _ = db.record_oratio_eval_run_start(&params).await;
            }

            let mut rec_idx = 0usize;
            for line in file.lines().filter(|l| !l.trim().is_empty()) {
                rec_idx += 1;
                if let Some(l) = limit {
                    if count >= l {
                        break;
                    }
                }
                let val: serde_json::Value = serde_json::from_str(line)
                    .with_context(|| format!("JSONL parse error at record {rec_idx}"))?;
                let path = val["path"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("JSONL record {rec_idx} missing 'path' string field")
                })?;
                let expected = val["expected"].as_str().ok_or_else(|| {
                    anyhow::anyhow!("JSONL record {rec_idx} missing 'expected' string field")
                })?;
                let lang = val.get("language").and_then(|v| v.as_str());

                let p = std::path::Path::new(path);
                let ctx = vox_oratio::refine::CorrectionContext::from_runtime(
                    runtime,
                    vox_oratio::refine::OratioCorrectionProfile::Balanced,
                    false,
                );
                let detail = transcribe_path_via_plugin(p, &ctx, lang)?;
                let actual = detail.refined_text;

                let expected_words: Vec<&str> = expected.split_whitespace().collect();

                let wer_val = vox_oratio::eval::word_error_rate(expected, &actual);
                let cer_val = vox_oratio::eval::char_error_rate(expected, &actual);
                let errs = (wer_val * expected_words.len() as f64).round() as usize;

                total_words += expected_words.len();
                total_errors += errs;
                count += 1;

                if let Some(ref db) = db_opt {
                    let _ = db
                        .append_oratio_eval_sample(
                            &run_id,
                            path,
                            expected,
                            &actual,
                            wer_val as f32,
                            cer_val as f32,
                            None,
                            None,
                            0,
                        )
                        .await;
                }

                println!("File: {}", path);
                println!("Expected: {}", expected);
                println!("Actual:   {}", actual);
                println!(
                    "Errors: {} / {} (WER: {:.1}%, CER: {:.1}%)",
                    errs,
                    expected_words.len(),
                    wer_val * 100.0,
                    cer_val * 100.0
                );
                println!("Confidence: {:.3}\n", detail.confidence);
            }

            let wer = if total_words > 0 {
                (total_errors as f64 / total_words as f64) * 100.0
            } else {
                0.0
            };

            if let Some(ref db) = db_opt {
                let _ = db
                    .complete_oratio_eval_run(&run_id, Some(wer as f32 / 100.0), None, None, None)
                    .await;
            }

            println!("Processed {} samples.", count);
            println!("Total Words: {}", total_words);
            println!("Total Errors: {}", total_errors);
            println!("Overall WER: {:.2}%", wer);

            Ok(())
        }
        OratioAction::EvalHistory { limit } => {
            let rt = tokio::runtime::Runtime::new()?;
            let db =
                rt.block_on(async { crate::workspace_db::connect_cli_workspace_voxdb().await })?;
            let runs = rt.block_on(async { db.get_recent_oratio_eval_runs(limit).await })?;
            if runs.is_empty() {
                println!("No evaluation runs found.");
            } else {
                for r in runs {
                    let wer_display = r
                        .global_wer
                        .map(|w| format!("{:.2}%", w * 100.0))
                        .unwrap_or_else(|| "N/A".to_string());
                    println!(
                        "{} | {} | {} | {} samples | WER: {}",
                        r.created_at, r.run_type, r.dataset_name, r.sample_count, wer_display
                    );
                }
            }
            Ok(())
        }
        OratioAction::Subtitle {
            path,
            output,
            language,
            line_width,
            max_lines,
            ground_truth_srt,
            persist,
        } => {
            let metrics = vox_oratio::subtitle::generate_srt_file(
                path.clone(),
                output,
                language,
                line_width,
                max_lines,
                ground_truth_srt.clone(),
                persist,
            )?;
            if persist {
                if let Some((wer, cer, offset)) = metrics {
                    let rt = tokio::runtime::Runtime::new()?;
                    let db_opt = rt.block_on(async {
                        crate::workspace_db::connect_cli_workspace_voxdb()
                            .await
                            .ok()
                    });
                    if let Some(db) = db_opt {
                        let run_id = uuid::Uuid::new_v4().to_string();
                        let params = vox_db::OratioEvalRunStartParams {
                            run_id: run_id.clone(),
                            run_type: "srt_ground_truth".to_string(),
                            backend: "candle-whisper".to_string(),
                            model_id: None,
                            dataset_name: ground_truth_srt.unwrap_or_default(),
                        };
                        let _ = rt.block_on(async {
                            if db.record_oratio_eval_run_start(&params).await.is_ok() {
                                let _ = db
                                    .append_oratio_eval_sample(
                                        &run_id, &path, "srts", "srts", wer as f32, cer as f32,
                                        None, None, 0,
                                    )
                                    .await;
                                let _ = db
                                    .complete_oratio_eval_run(
                                        &run_id,
                                        Some(wer as f32),
                                        Some(cer as f32),
                                        None,
                                        Some(offset),
                                    )
                                    .await;
                            }
                        });
                    }
                }
            }
            Ok(())
        }
        OratioAction::Serve { port } => {
            vox_oratio::serve::run_serve_worker(port).await?;
            Ok(())
        }
    }
}
