//! Shared compiler pipeline for the Vox CLI.
//!
//! Provides a single entry point (`run_frontend`) that runs the full
//! lex → parse → typecheck → HIR validation pass and returns structured
//! results. All CLI commands (`build`, `check`) and the LSP use this so
//! that error formatting stays consistent and pipeline changes need to be
//! made in exactly one place.
//!
//! **Separation of concerns:** this module owns compiler integration and
//! human/JSON rendering helpers for diagnostics. Domain policy (what counts
//! as a successful build artifact, deploy graphs, etc.) stays in command
//! adapters under `commands/` and downstream crates — keep new presentation-only
//! tweaks here and push reusable compiler logic toward `vox-compiler`.

use anyhow::{Context, Result};
use miette::{
    Diagnostic as MietteDiagnostic, GraphicalReportHandler, LabeledSpan, NamedSource, SourceCode,
    SourceOffset, SourceSpan,
};
use owo_colors::OwoColorize;
use std::path::Path;
use vox_compiler::ast::decl::Module;
use vox_compiler::hir::HirModule;
use vox_compiler::pipeline::PipelineOptions;
use vox_compiler::typeck::Diagnostic;
use vox_compiler::typeck::diagnostics::TypeckSeverity;

use vox_bounded_fs::read_utf8_path_capped;

fn line_col_for_byte_offset(source: &str, byte_idx: usize) -> (usize, usize) {
    let (l0, c0) = vox_compiler::ast::span::byte_offset_to_line_col_zero_based(source, byte_idx);
    (l0 as usize + 1, c0 as usize + 1)
}

fn source_line_at(source: &str, line_1based: usize) -> Option<&str> {
    source.lines().nth(line_1based.saturating_sub(1))
}

/// The result of running the frontend pipeline (lex → parse → typecheck → HIR).
pub struct FrontendResult {
    /// Parsed AST module root.
    pub module: Module,
    /// Lowered and validated HIR module.
    pub hir: HirModule,
    /// Diagnostics emitted during typecheck and HIR validation.
    pub diagnostics: Vec<Diagnostic>,
    /// Full source text (for span rendering and line snippets).
    pub source: String,
}

impl FrontendResult {
    /// Count of error-severity diagnostics.
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == TypeckSeverity::Error)
            .count()
    }

    /// Count of warning-severity diagnostics.
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == TypeckSeverity::Warning)
            .count()
    }

    /// Returns `true` if any error-severity diagnostic was produced.
    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    /// Returns `true` if any warning-severity diagnostic was produced.
    pub fn has_warnings(&self) -> bool {
        self.warning_count() > 0
    }
}

/// Run the frontend pipeline on a source file.
///
/// Steps:
/// 1. Lex
/// 2. Parse (returns `Err` on parse failure with pretty-printed errors)
/// 3. Type-check
/// 4. Lower to HIR + run HIR validation
///
/// Parse errors are printed to stderr in rustc style and returned as `Err`.
/// Type/HIR diagnostics are stored in [`FrontendResult::diagnostics`]; it is
/// the caller's responsibility to decide whether to treat them as fatal.
pub async fn run_frontend(file: &Path, json: bool) -> Result<FrontendResult> {
    run_frontend_with_options(file, json, &PipelineOptions::default()).await
}

pub async fn run_frontend_with_options(
    file: &Path,
    json: bool,
    options: &PipelineOptions,
) -> Result<FrontendResult> {
    let source = read_utf8_path_capped(file)
        .with_context(|| format!("Failed to read source file: {}", file.display()))?;

    run_frontend_str_with_options(&source, file, json, options)
}

/// Same as [`run_frontend`] but takes an already-loaded source string.
pub fn run_frontend_str(source: &str, file: &Path, json: bool) -> Result<FrontendResult> {
    run_frontend_str_with_options(source, file, json, &PipelineOptions::default())
}

pub fn run_frontend_str_with_options(
    source: &str,
    file: &Path,
    json: bool,
    options: &PipelineOptions,
) -> Result<FrontendResult> {
    let file_path = file.to_string_lossy();
    match vox_compiler::pipeline::run_frontend_str_with_options(source, &file_path, options) {
        Ok(res) => {
            // Check for gamified features if compile succeeded and there are no typeck/HIR errors
            if !res
                .diagnostics
                .iter()
                .any(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
            {
                // See Cargo.toml: `vox-gamify` is now a declared feature
                // (`vox-gamify = ["dep:vox-gamify"]`). Before that it named no
                // feature — `dep:vox-gamify` suppresses the implicit one — so
                // this gamified-feature detection never ran. Kept gated so the
                // dependency stays optional for lean builds.
                #[cfg(feature = "vox-gamify")]
                {
                    let mut has_remote = false;
                    let mut has_durable = false;
                    let mut has_actor = false;

                    for f in &res.hir.functions {
                        if f.is_remote {
                            has_remote = true;
                        }
                        if let Some(kind) = f.durability {
                            match kind {
                                vox_compiler::hir::DurabilityKind::Workflow
                                | vox_compiler::hir::DurabilityKind::Activity => {
                                    has_durable = true;
                                }
                                vox_compiler::hir::DurabilityKind::Actor => {
                                    has_actor = true;
                                }
                            }
                        }
                    }

                    let mut features = Vec::new();
                    if has_remote {
                        features.push("@remote");
                    }
                    if has_durable {
                        features.push("@durable");
                    }
                    if has_actor {
                        features.push("actor");
                    }

                    if !features.is_empty() {
                        let trigger_milestones = move || match tokio::runtime::Handle::try_current()
                        {
                            Ok(handle) => {
                                handle.spawn(async move {
                                    if let Ok(db) = vox_db::Codex::connect_default().await {
                                        for feature in features {
                                            let ev = serde_json::json!({
                                                "type": "vox_feature_milestone",
                                                "source": "vox-compiler",
                                                "payload": { "feature": feature },
                                            });
                                            let _ =
                                                vox_gamify::event_router::route_event_auto_user(
                                                    &db, &ev,
                                                )
                                                .await;
                                        }
                                    }
                                });
                            }
                            Err(_) => {
                                static COMPILER_EVENT_RT: std::sync::OnceLock<
                                    tokio::runtime::Runtime,
                                > = std::sync::OnceLock::new();
                                let rt = COMPILER_EVENT_RT.get_or_init(|| {
                                    tokio::runtime::Builder::new_multi_thread()
                                        .worker_threads(1)
                                        .enable_all()
                                        .build()
                                        .expect("failed to build compiler event runtime")
                                });
                                rt.spawn(async move {
                                    if let Ok(db) = vox_db::Codex::connect_default().await {
                                        for feature in features {
                                            let ev = serde_json::json!({
                                                "type": "vox_feature_milestone",
                                                "source": "vox-compiler",
                                                "payload": { "feature": feature },
                                            });
                                            let _ =
                                                vox_gamify::event_router::route_event_auto_user(
                                                    &db, &ev,
                                                )
                                                .await;
                                        }
                                    }
                                });
                            }
                        };
                        trigger_milestones();
                    }
                }
            }
            Ok(FrontendResult {
                module: res.module,
                hir: res.hir,
                diagnostics: res.diagnostics,
                source: res.source,
            })
        }
        Err(e) => {
            if json {
                let diagnostics = vox_compiler::pipeline::check_file(source, &file_path);
                if let Ok(s) = serde_json::to_string_pretty(&diagnostics) {
                    println!("{}", s);
                }
            } else {
                // We need the parse errors to print them pretty.
                // For now, we'll re-lex/parse if we need pretty printing,
                // but usually, run_frontend_str failure means parse failure.
                let tokens = vox_compiler::lexer::lex(source);
                if let Err(errors) = vox_compiler::parser::parse(tokens) {
                    if human_diagnostics_enabled(false) {
                        print_parse_errors_human(&errors, source, file);
                    } else {
                        print_parse_errors(&errors, source, file);
                    }
                }
            }
            Err(e)
        }
    }
}

#[must_use]
pub fn format_diagnostics_json_pretty(result: &FrontendResult, file: &Path) -> String {
    use vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload;
    let file_path = file.to_string_lossy();
    let output: Vec<VoxCompilerDiagnosticPayload> = result
        .diagnostics
        .iter()
        .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(d, &file_path, &result.source))
        .collect();
    serde_json::to_string_pretty(&output).unwrap_or_default()
}

/// A lint finding from `vox-code-audit` surfaced in the `vox check --for-llm` envelope.
///
/// This is a separate type from `VoxCompilerDiagnosticPayload` because lint findings
/// originate from pattern-based static analysis (not the type-checker) and carry
/// additional metadata — `rationale`, `confidence`, `alternatives` — that does not
/// apply to compiler diagnostics.
///
/// Present in [`CheckForLlmEnvelope::lint_findings`] when the `stub-check` feature
/// is compiled in; the field is omitted from JSON (via `skip_serializing_if`) when
/// the feature is absent so the envelope schema stays stable.
#[derive(serde::Serialize)]
pub struct LintFindingPayload {
    /// Stable `vox/<category>/<name>` rule identifier.
    pub rule_id: String,
    /// Normalized severity: `"info"` | `"warning"` | `"error"` | `"critical"`.
    pub severity: String,
    /// Short description of the specific problem found at this location.
    pub message: String,
    /// 1-based line number within the file.
    pub line: usize,
    /// 1-based column within the line (0 if unknown).
    pub column: usize,
    /// Prose explaining *why* this rule exists — constant per rule, useful for
    /// LLM rationale and `vox check --explain <id>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// Detector-estimated confidence in the finding: `"high"` | `"medium"` | `"low"`.
    /// Absent when the detector does not assign a confidence level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    /// Primary fix suggestion (the single most likely correct approach).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// Alternative fix approaches when multiple valid strategies exist.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    /// Stable URL for the human/LLM-readable explanation page (`vox-lang.org/diag/<id>`).
    /// Only present when `rule_id` follows the `vox/<category>/<name>` scheme.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain_url: Option<String>,
    /// Minimal code snippet (≤ 8 lines) that reproduces a typical violation for this rule,
    /// together with the canonical fix. Provided by the detector's `DetectionRule::minimal_repro()`
    /// implementation; absent when the detector does not supply one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal_repro: Option<String>,
}

/// Stable JSON envelope for `vox check --for-llm` (machine / LLM consumers).
#[derive(serde::Serialize)]
pub struct CheckForLlmEnvelope {
    pub envelope_version: u32,
    pub file_path: String,
    pub ok: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload>,
    /// Static-analysis (TOESTUB) lint findings for this file.
    ///
    /// Populated when the `stub-check` feature is compiled in; absent from the
    /// serialized JSON when empty so the schema stays stable across feature configs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lint_findings: Vec<LintFindingPayload>,
}

/// Full frontend diagnostics as one JSON object (`check_file`), including parse failures.
///
/// When the `stub-check` feature is enabled the envelope also includes
/// [`CheckForLlmEnvelope::lint_findings`] from `vox-code-audit`'s Vox-language
/// detectors (auth, decorator-position, retired APIs, security rules, …).
#[must_use]
pub fn format_check_for_llm_json(source: &str, file: &Path) -> String {
    let file_path = file.to_string_lossy().to_string();
    let diagnostics = vox_compiler::pipeline::check_file(source, &file_path);
    let error_count = diagnostics
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .count();
    let warning_count = diagnostics
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Warning)
        .count();

    #[cfg(feature = "stub-check")]
    let lint_findings: Vec<LintFindingPayload> = {
        use vox_code_audit::engine::{ToestubConfig, ToestubEngine};
        use vox_code_audit::rules::{FindingConfidence, Language, Severity, SourceFile};

        let source_file = SourceFile::new(file.to_path_buf(), source.to_string());
        // Only Vox files produce useful lint findings here; Rust/Unknown files go
        // through the full engine scan path (vox stub-check), not --for-llm.
        if matches!(source_file.language, Language::Vox | Language::Unknown) {
            let config = ToestubConfig {
                min_severity: Severity::Warning,
                ..ToestubConfig::default()
            };
            let engine = ToestubEngine::new(config);
            let repro_table = engine.minimal_repro_table();
            engine
                .check_source_file(&source_file)
                .into_iter()
                .map(|f| {
                    let explain_url = f
                        .diagnostic_id
                        .as_deref()
                        .filter(|id| id.starts_with("vox/"))
                        .map(|id| format!("https://vox-lang.org/diag/{id}"));
                    let confidence = f.confidence.map(|c| {
                        match c {
                            FindingConfidence::High => "high",
                            FindingConfidence::Medium => "medium",
                            FindingConfidence::Low => "low",
                        }
                        .to_string()
                    });
                    let severity = match f.severity {
                        Severity::Info => "info",
                        Severity::Warning => "warning",
                        Severity::Error => "error",
                        Severity::Critical => "critical",
                    }
                    .to_string();
                    let minimal_repro = repro_table.get(f.rule_id.as_str()).map(|s| s.to_string());
                    LintFindingPayload {
                        rule_id: f.rule_id,
                        severity,
                        message: f.message,
                        line: f.line,
                        column: f.column,
                        rationale: f.rationale,
                        confidence,
                        suggestion: f.suggestion,
                        alternatives: f.alternatives,
                        explain_url,
                        minimal_repro,
                    }
                })
                .collect()
        } else {
            vec![]
        }
    };

    #[cfg(not(feature = "stub-check"))]
    let lint_findings: Vec<LintFindingPayload> = vec![];

    let env = CheckForLlmEnvelope {
        envelope_version: 1,
        file_path,
        ok: error_count == 0,
        error_count,
        warning_count,
        diagnostics,
        lint_findings,
    };
    serde_json::to_string_pretty(&env).unwrap_or_default()
}

/// True when the user passed the root `--json` flag —
/// [`crate::apply_global_opts`] / `run_vox_cli_from_parsed` set
/// `VOX_CLI_GLOBAL_JSON=1` before command dispatch.
#[must_use]
pub fn global_json_enabled() -> bool {
    std::env::var("VOX_CLI_GLOBAL_JSON").ok().as_deref() == Some("1")
}

/// Stable single-line JSON envelope for build-lane commands (`vox build`,
/// `vox test`, `vox run --mode script`). Mirrors [`CheckForLlmEnvelope`]
/// field naming; `command` discriminates the lane. Compact (one line) so
/// multiple envelopes on one stdout stream parse as JSONL.
#[derive(serde::Serialize)]
pub struct BuildLaneEnvelope {
    pub envelope_version: u32,
    pub command: String,
    pub file_path: String,
    pub ok: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload>,
    /// Child-process exit code (`vox test`'s `cargo test`); absent for
    /// compile-only envelopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// Envelope for a lane that just ran the frontend (diagnostics attached).
#[must_use]
pub fn format_build_lane_envelope_json(
    command: &str,
    file: &Path,
    result: &FrontendResult,
    exit_code: Option<i32>,
) -> String {
    use vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload;
    let file_path = file.to_string_lossy().to_string();
    let diagnostics: Vec<VoxCompilerDiagnosticPayload> = result
        .diagnostics
        .iter()
        .map(|d| VoxCompilerDiagnosticPayload::from_diagnostic(d, &file_path, &result.source))
        .collect();
    let env = BuildLaneEnvelope {
        envelope_version: 1,
        command: command.to_string(),
        file_path,
        ok: !result.has_errors(),
        error_count: result.error_count(),
        warning_count: result.warning_count(),
        diagnostics,
        exit_code,
    };
    serde_json::to_string(&env).unwrap_or_default()
}

/// Envelope for lane results with no [`FrontendResult`] at hand (e.g.
/// `vox test` after `cargo test` — the preceding build envelope already
/// carried the diagnostics).
#[must_use]
pub fn format_command_result_envelope_json(
    command: &str,
    file: &Path,
    ok: bool,
    exit_code: Option<i32>,
) -> String {
    let env = BuildLaneEnvelope {
        envelope_version: 1,
        command: command.to_string(),
        file_path: file.to_string_lossy().to_string(),
        ok,
        error_count: 0,
        warning_count: 0,
        diagnostics: Vec::new(),
        exit_code,
    };
    serde_json::to_string(&env).unwrap_or_default()
}

/// True when caret-style human diagnostics are requested.
///
/// Enabled by `VOX_DIAG_FORMAT=human` or `vox check --human-diagnostics`.
#[must_use]
pub fn human_diagnostics_enabled(cli_flag: bool) -> bool {
    cli_flag
        || std::env::var("VOX_DIAG_FORMAT")
            .map(|v| v.eq_ignore_ascii_case("human"))
            .unwrap_or(false)
}

/// Print diagnostics in rustc-style to stderr, or JSON to stdout if `json` is true.
///
/// When `human` is true (or `VOX_DIAG_FORMAT=human`), renders caret underlines via miette.
pub fn print_diagnostics(result: &FrontendResult, file: &Path, json: bool) {
    print_diagnostics_with_mode(result, file, json, false);
}

/// Like [`print_diagnostics`] but accepts an explicit human-rendering flag from the CLI.
pub fn print_diagnostics_with_mode(result: &FrontendResult, file: &Path, json: bool, human: bool) {
    if json {
        println!("{}", format_diagnostics_json_pretty(result, file));
        return;
    }
    if human_diagnostics_enabled(human) {
        print_diagnostics_human(result, file);
        return;
    }
    for (i, d) in result.diagnostics.iter().enumerate() {
        let code = format!("E{:04}", i + 1);
        let (line, col) = line_col_for_byte_offset(&result.source, d.span.start);
        let sev = match d.severity {
            TypeckSeverity::Error => "error",
            TypeckSeverity::Warning => "warning",
        };
        eprintln!(
            "{sev}[{code}]: {} at {}:{}:{}",
            d.message,
            file.display(),
            line,
            col
        );
    }
}

#[derive(Debug)]
struct HumanSourceDiag {
    message: String,
    label: String,
    span: SourceSpan,
    source: NamedSource<String>,
}

impl std::fmt::Display for HumanSourceDiag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for HumanSourceDiag {}

impl MietteDiagnostic for HumanSourceDiag {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.source as &dyn SourceCode)
    }
}

/// Caret-style diagnostic rendering for terminal humans (miette fancy).
pub fn print_diagnostics_human(result: &FrontendResult, file: &Path) {
    let file_label = file.display().to_string();
    let named = NamedSource::new(file_label.clone(), result.source.clone());
    for (i, d) in result.diagnostics.iter().enumerate() {
        let code = d.code.clone().unwrap_or_else(|| format!("E{:04}", i + 1));
        let sev = match d.severity {
            TypeckSeverity::Error => "error",
            TypeckSeverity::Warning => "warning",
        };
        let start = SourceOffset::from(d.span.start);
        let len = d.span.end.saturating_sub(d.span.start).max(1);
        let span = SourceSpan::new(start, len);
        let message = format!("[{code}] {}", d.message);
        let diag = HumanSourceDiag {
            message,
            label: sev.to_string(),
            span,
            source: named.clone(),
        };
        let handler = GraphicalReportHandler::new();
        let mut rendered = String::new();
        if handler.render_report(&mut rendered, &diag).is_ok() {
            eprint!("{rendered}");
        }
    }
}

/// Parse errors with caret underlines (miette).
pub fn print_parse_errors_human(
    errors: &[vox_compiler::parser::ParseError],
    source: &str,
    file: &Path,
) {
    let named = NamedSource::new(file.display().to_string(), source.to_string());
    for e in errors {
        let start = SourceOffset::from(e.span.start);
        let len = e.span.end.saturating_sub(e.span.start).max(1);
        let span = SourceSpan::new(start, len);
        let diag = HumanSourceDiag {
            message: e.message.clone(),
            label: "parse error".to_string(),
            span,
            source: named.clone(),
        };
        let handler = GraphicalReportHandler::new();
        let mut rendered = String::new();
        if handler.render_report(&mut rendered, &diag).is_ok() {
            eprint!("{rendered}");
        }
    }
    eprintln!(
        "{} aborting due to {} previous {}",
        "error".red().bold(),
        errors.len(),
        if errors.len() == 1 { "error" } else { "errors" }
    );
}

/// Print parse errors to stderr in rustc style.
pub fn print_parse_errors_to_stderr(
    errors: &[vox_compiler::parser::ParseError],
    source: &str,
    file: &Path,
) {
    print_parse_errors(errors, source, file);
}

fn print_parse_errors(errors: &[vox_compiler::parser::ParseError], source: &str, file: &Path) {
    for e in errors {
        let (line, col) = line_col_for_byte_offset(source, e.span.start);
        let context_line = source_line_at(source, line).unwrap_or("");
        eprintln!("{} {}", "error[parse]".red().bold(), e.message.bold());
        eprintln!(
            "  {} {}:{}:{}",
            "-->".blue().bold(),
            file.display(),
            line,
            col
        );
        eprintln!("   {}", "|".blue().bold());
        eprintln!("   {} {}", format!("{line} |").blue().bold(), context_line);
        let arrow = " ".repeat(col.saturating_sub(1)) + "^";
        eprintln!("   {} {}", "|".blue().bold(), arrow.red().bold());
        eprintln!();
    }
    eprintln!(
        "{} aborting due to {} previous {}",
        "error".red().bold(),
        errors.len(),
        if errors.len() == 1 { "error" } else { "errors" }
    );
}
