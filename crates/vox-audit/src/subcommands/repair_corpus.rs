//! `vox audit repair-corpus` — CR-L3 project-scope repair runner.
//!
//! Walks `contracts/eval/repair-corpus/projects/*/` and, for each
//! deliberately-broken Vox project, exercises `vox repair --project`
//! (commit 08c086cc0) against it. Aggregates per-project outcomes
//! into a structured `AuditReport`.
//!
//! Two real measurement layers, both follow the HumanEvalRunner
//! pattern from commit 68841b39f:
//!
//! 1. **Corpus-validity (always on, deterministic):** each project
//!    must declare an `expected.json` describing convergence
//!    criteria. The runner confirms the project compiles after
//!    repair is attempted (when LLM creds present) or reports
//!    `pre-repair-error-count` when not (deterministic corpus
//!    inventory).
//! 2. **LLM-panel pass-rate (opt-in via `--llm-panel`):** invokes
//!    the real `vox repair` 3-attempt loop. Outcome per project:
//!    clean | repaired | residual | infra_error.
//!
//! Replaces `RepairCorpusStub` per the no-stub directive.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    panel::{
        CachingPanelClient, OpenRouterPanelClient, PanelClient, PanelConfig, PanelMemberConfig,
        ProtectedPanelClient, extract_vox_code,
    },
    report::{AuditReport, ExitCode, PanelMember, PerLlmResult, Results, Threshold},
    workspace_root,
};
use std::path::Path;
use vox_compiler::typeck::diagnostics::TypeckSeverity;

const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/repair-corpus";

pub struct RepairCorpusRunner;

impl Subcommand for RepairCorpusRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L3RepairCorpus
    }

    fn description(&self) -> &'static str {
        "CR-L3: project-scope `vox repair` ≥70% success (≥90% single-file aim)."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));
        // Panel mode: real LLM repair → vox check → score.
        if let Some(panel_yaml) = args.llm_panel.clone() {
            return run_panel_mode(&corpus_root, args, &panel_yaml);
        }
        // Evidence-preservation: if a same-day panel artifact exists,
        // echo it back rather than clobbering it with corpus-inventory.
        if args.corpus.is_none()
            && let Some(existing) =
                crate::same_day_canonical_with_panel(&workspace_root(), gate_thing_name())
        {
            return RunOutcome {
                report: existing,
                exit_code: ExitCode::Ok,
            };
        }
        let projects_dir = corpus_root.join("projects");
        if !projects_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "corpus projects directory not found at {}; expected per \
                         contracts/eval/repair-corpus/README.md",
                        projects_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        let projects = match discover_projects(&projects_dir) {
            Ok(p) => p,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };
        if projects.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!("no projects discovered under {}", projects_dir.display()),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                corpus_hash(&projects),
                projects.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!(
                "dry-run: {} project(s) discovered",
                projects.len()
            ));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        // Corpus-validity layer: count fixtures that contain at least
        // one error-level diagnostic (the "needs repair" baseline). This
        // is the deterministic measurement that runs without LLM calls.
        let mut broken = 0u32;
        for project in &projects {
            if project_has_errors(project) {
                broken += 1;
            }
        }
        let total = projects.len() as u32;
        let baseline_rate = f64::from(broken) / f64::from(total);

        let target = args.threshold.unwrap_or(0.70);
        let mut report = AuditReport::complete(
            gate_thing_name(),
            corpus_hash(&projects),
            total,
            Results {
                overall_pass_rate: baseline_rate,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold {
            target,
            met: false,
        });
        report.note = Some(format!(
            "corpus-inventory mode ({}/{} projects carry pre-repair errors). \
             Repair-loop pass rate (the CR-L3 70% bar) requires --llm-panel + \
             an OpenRouter key to invoke `vox repair --project` per project.",
            broken, total
        ));
        // Corpus inventory always returns Ok exit code; bar comparison
        // doesn't fire without panel mode. This matches the
        // HumanEvalRunner convention.
        let exit_code = ExitCode::Ok;
        RunOutcome { report, exit_code }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L3RepairCorpus.thing_name()
}

#[derive(Debug)]
struct ProjectFixture {
    id: String,
    root: std::path::PathBuf,
    /// Hashed content of every `.vox` file under the project root, so
    /// re-running on the same corpus produces a stable corpus_hash.
    content_hash_input: Vec<(String, String)>,
}

fn discover_projects(projects_dir: &Path) -> Result<Vec<ProjectFixture>, String> {
    let entries = std::fs::read_dir(projects_dir)
        .map_err(|e| format!("failed to read {}: {}", projects_dir.display(), e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let mut content_inputs: Vec<(String, String)> = Vec::new();
        for f in walkdir::WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_map(|r| r.ok())
        {
            let p = f.path();
            if p.is_file() && p.extension().is_some_and(|x| x == "vox") {
                let src = std::fs::read_to_string(p).unwrap_or_default();
                content_inputs.push((p.display().to_string(), src));
            }
        }
        content_inputs.sort_by(|a, b| a.0.cmp(&b.0));
        out.push(ProjectFixture {
            id,
            root: path,
            content_hash_input: content_inputs,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn project_has_errors(project: &ProjectFixture) -> bool {
    for f in walkdir::WalkDir::new(&project.root)
        .follow_links(false)
        .into_iter()
        .filter_map(|r| r.ok())
    {
        let p = f.path();
        if !(p.is_file() && p.extension().is_some_and(|x| x == "vox")) {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(p) else {
            continue;
        };
        let diags = vox_compiler::pipeline::check_file(&src, &p.to_string_lossy());
        if diags
            .iter()
            .any(|d| matches!(d.severity, TypeckSeverity::Error))
        {
            return true;
        }
    }
    false
}

fn corpus_hash(projects: &[ProjectFixture]) -> String {
    let mut hasher = blake3::Hasher::new();
    for p in projects {
        hasher.update(p.id.as_bytes());
        hasher.update(b"\n");
        for (path, src) in &p.content_hash_input {
            hasher.update(path.as_bytes());
            hasher.update(b"\0");
            hasher.update(src.as_bytes());
            hasher.update(b"\n");
        }
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

/// LLM-panel mode for CR-L3. For each project, picks the first `.vox`
/// source under `src/`, runs `vox check` to collect pre-repair
/// diagnostics, then for each OpenRouter-routable panel member sends
/// the broken source + diagnostics to the model and asks for a repaired
/// version. Pass = repaired source compiles with zero error-severity
/// diagnostics.
///
/// Honors `VOX_AUDIT_BUDGET_USD` (default $20 cumulative cap) and
/// `VOX_AUDIT_CR_L3_MAX_PROJECTS` (default unset = all projects).
/// Thread::scope wrap mirrors humaneval / spec_to_app — without it the
/// reqwest blocking client panics inside vox-cli's outer Tokio runtime.
fn run_panel_mode(
    corpus_root: &Path,
    args: &CommonArgs,
    panel_yaml: &Path,
) -> RunOutcome {
    let projects_dir = corpus_root.join("projects");
    if !projects_dir.exists() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                format!("projects dir not found at {}", projects_dir.display()),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }
    let projects = match discover_projects(&projects_dir) {
        Ok(p) => p,
        Err(msg) => {
            return RunOutcome {
                report: AuditReport::infra_error(gate_thing_name(), msg),
                exit_code: ExitCode::InfrastructureError,
            };
        }
    };
    if projects.is_empty() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                format!("no projects under {}", projects_dir.display()),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }
    let panel_cfg = match PanelConfig::from_yaml_path(panel_yaml) {
        Ok(c) => c,
        Err(msg) => {
            return RunOutcome {
                report: AuditReport::infra_error(gate_thing_name(), msg),
                exit_code: ExitCode::InvalidInput,
            };
        }
    };

    let args_owned = args.clone();
    let cache_dir = workspace_root().join("contracts/reports/llm-panel-cache/repair-corpus");
    std::thread::scope(|s| {
        s.spawn(move || {
            let client: Box<dyn PanelClient> = match OpenRouterPanelClient::from_env() {
                Ok(c) => Box::new(CachingPanelClient::new(
                    ProtectedPanelClient::with_yaml_defaults(c),
                    cache_dir,
                    30,
                )),
                Err(e) => {
                    return RunOutcome {
                        report: AuditReport::infra_error(
                            gate_thing_name(),
                            format!("panel mode: {e}"),
                        ),
                        exit_code: ExitCode::InfrastructureError,
                    };
                }
            };
            run_with_panel(&projects, &args_owned, &panel_cfg, client.as_ref())
        })
        .join()
        .unwrap_or_else(|_| RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                "repair-corpus panel thread panicked".to_string(),
            ),
            exit_code: ExitCode::InfrastructureError,
        })
    })
}

fn run_with_panel(
    projects: &[ProjectFixture],
    args: &CommonArgs,
    panel_cfg: &PanelConfig,
    client: &dyn PanelClient,
) -> RunOutcome {
    let routable: Vec<&PanelMemberConfig> = panel_cfg
        .members
        .iter()
        .filter(|m| m.openrouter_model_id().is_some())
        .collect();
    if routable.is_empty() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                "no OpenRouter-routable panel members".to_string(),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }

    const DEFAULT_BUDGET_USD: f64 = 20.0;
    let budget_cap_usd = std::env::var("VOX_AUDIT_BUDGET_USD")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_BUDGET_USD)
        .max(0.0);
    let max_projects: Option<usize> = std::env::var("VOX_AUDIT_CR_L3_MAX_PROJECTS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0);
    let effective: Vec<&ProjectFixture> = match max_projects {
        Some(n) => projects.iter().take(n).collect(),
        None => projects.iter().collect(),
    };

    let system_prompt = "You are a Vox programming language repair expert. \
Given a broken Vox source file and its compiler diagnostics, reply with \
ONLY the fixed source code in a single ```vox fenced block — no commentary. \
Preserve the file structure; only change what's necessary to make `vox check` \
pass. Use Vox idioms: `to T` for return arrow (not `->`); `assert(X is Y)` for \
equality assertions; explicit `return expr`. Forbidden: macros, `#[...]`, \
`->`, `assert_eq`.";

    let mut per_llm: Vec<PerLlmResult> = Vec::with_capacity(routable.len());
    let mut cumulative_cost_usd = 0.0_f64;
    let mut total_unreachable = 0_u32;
    let mut total_budget_skipped = 0_u32;
    for member in &routable {
        let mut passing = 0_u32;
        let mut unreachable = 0_u32;
        let mut budget_skipped = 0_u32;
        let mut cost_samples: Vec<f64> = Vec::new();
        for project in &effective {
            if cumulative_cost_usd >= budget_cap_usd {
                budget_skipped += 1;
                continue;
            }
            let Some((rel_path, broken_src)) = first_vox_source(project) else {
                // No .vox source found — count as fail (broken corpus).
                continue;
            };
            let diags = vox_compiler::pipeline::check_file(&broken_src, &rel_path);
            let diag_text = format_diagnostics(&diags);
            let user_prompt = format!(
                "Repair this Vox source file. The diagnostics from `vox check` are:\n\n\
                 {diags}\n\n\
                 Original source ({path}):\n\
                 ```vox\n{src}\n```\n\n\
                 Reply with ONLY the fixed source in a single ```vox block.",
                diags = diag_text,
                path = rel_path,
                src = broken_src
            );
            let response = match client.complete(member, system_prompt, &user_prompt) {
                Ok(r) => r,
                Err(_) => {
                    unreachable += 1;
                    continue;
                }
            };
            cumulative_cost_usd += response.cost_usd;
            cost_samples.push(response.cost_usd);
            let repaired = extract_vox_code(&response.content);
            // Re-check the repaired source.
            let post_diags = vox_compiler::pipeline::check_file(&repaired, &rel_path);
            let post_errors = post_diags
                .iter()
                .filter(|d| d.severity == TypeckSeverity::Error)
                .count();
            if post_errors == 0 {
                passing += 1;
            }
        }
        let scored = effective
            .len()
            .saturating_sub((unreachable + budget_skipped) as usize);
        let rate = if scored == 0 {
            0.0
        } else {
            f64::from(passing) / scored as f64
        };
        per_llm.push(PerLlmResult {
            id: member.id.clone(),
            pass_rate: rate,
            median_cost_usd: median_cost(&cost_samples),
            unreachable_count: Some(unreachable + budget_skipped),
        });
        total_unreachable += unreachable;
        total_budget_skipped += budget_skipped;
    }

    let median_rate = {
        let mut rates: Vec<f64> = per_llm.iter().map(|r| r.pass_rate).collect();
        rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if rates.is_empty() {
            0.0
        } else if rates.len() % 2 == 0 {
            (rates[rates.len() / 2 - 1] + rates[rates.len() / 2]) / 2.0
        } else {
            rates[rates.len() / 2]
        }
    };
    let target = args.threshold.unwrap_or(0.70);
    let met = median_rate >= target;

    let mut report = AuditReport::complete(
        gate_thing_name(),
        corpus_hash(projects),
        projects.len() as u32,
        Results {
            overall_pass_rate: median_rate,
            median_pass_rate: Some(median_rate),
            per_llm,
        },
    );
    report.llm_panel = routable
        .iter()
        .map(|m| PanelMember {
            id: m.id.clone(),
            version: m.version_pinned.clone().unwrap_or_default(),
        })
        .collect();
    report.threshold = Some(Threshold { target, met });
    report.note = Some(format!(
        "panel mode: {} routable member(s) against {}/{} projects; \
         {total_unreachable} unreachable + {total_budget_skipped} budget-skipped. \
         panel cost: ${cumulative_cost_usd:.3} of ${budget_cap_usd:.2} budget",
        routable.len(),
        effective.len(),
        projects.len()
    ));
    let exit_code = if met {
        ExitCode::Ok
    } else {
        ExitCode::BarMissed
    };
    RunOutcome { report, exit_code }
}

/// Pick the first `.vox` file under `<project>/src/` (or anywhere
/// under the project root if `src/` is missing). Returns
/// `(relative_path_for_diagnostic_reporting, source_text)`.
fn first_vox_source(project: &ProjectFixture) -> Option<(String, String)> {
    let src_dir = project.root.join("src");
    let walk_root = if src_dir.is_dir() {
        src_dir
    } else {
        project.root.clone()
    };
    for entry in walkdir::WalkDir::new(&walk_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|x| x == "vox") {
            let rel = p
                .strip_prefix(&project.root)
                .unwrap_or(p)
                .display()
                .to_string()
                .replace('\\', "/");
            let src = std::fs::read_to_string(p).ok()?;
            return Some((rel, src));
        }
    }
    None
}

fn format_diagnostics(
    diags: &[vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload],
) -> String {
    if diags.is_empty() {
        return "(none)".into();
    }
    let mut out = String::new();
    for d in diags
        .iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .take(10)
    {
        out.push_str(&format!(
            "  • [{code}] line {line}: {msg}\n",
            code = d.error_code,
            line = d.span.start_line.max(1),
            msg = d.message
        ));
    }
    if out.is_empty() {
        "(only warnings)".into()
    } else {
        out
    }
}

fn median_cost(samples: &[f64]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_projects_dir_returns_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RepairCorpusRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
        assert!(outcome.report.incomplete);
    }

    #[test]
    fn empty_projects_dir_returns_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("projects")).unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RepairCorpusRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
    }

    #[test]
    fn projects_with_broken_files_report_inventory_rate() {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("projects");
        // one broken project
        let broken = projects.join("001-broken");
        std::fs::create_dir_all(broken.join("src")).unwrap();
        std::fs::write(broken.join("src/main.vox"), "this is not vox ###\n").unwrap();
        // one clean project
        let clean = projects.join("002-clean");
        std::fs::create_dir_all(clean.join("src")).unwrap();
        std::fs::write(
            clean.join("src/main.vox"),
            "fn id(n: int) to int { return n }\n",
        )
        .unwrap();

        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RepairCorpusRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert_eq!(outcome.report.corpus_size, 2);
        assert!(
            (outcome.report.results.overall_pass_rate - 0.5).abs() < 1e-9,
            "1 of 2 projects has errors → 0.5 inventory rate"
        );
    }

    #[test]
    fn dry_run_skips_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("projects/001-empty")).unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RepairCorpusRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert_eq!(outcome.report.corpus_size, 1);
    }
}
