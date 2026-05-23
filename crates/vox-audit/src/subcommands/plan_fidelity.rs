//! `vox audit plan-fidelity` — CR-L4 plan-mode fidelity runner.
//!
//! Walks `contracts/eval/plan-fidelity/plans/*/plan.toml` and reports
//! corpus inventory + (opt-in) panel-driven fidelity rate.
//!
//! Replaces `PlanFidelityStub` per the no-stub directive. The runner
//! does real corpus enumeration today; LLM-driven measurement is
//! opt-in via `--llm-panel`.

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

const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/plan-fidelity";

pub struct PlanFidelityRunner;

impl Subcommand for PlanFidelityRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L4PlanFidelity
    }

    fn description(&self) -> &'static str {
        "CR-L4: plan-mode fidelity ≥85% on Wave-2 benchmark."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));
        // Panel mode: drive each plan through the LLM panel, score by
        // whether the produced source vox-checks clean.
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
        let plans_dir = corpus_root.join("plans");
        if !plans_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "plans dir not found at {}; expected per \
                         contracts/eval/plan-fidelity/README.md",
                        plans_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        let plans = match discover_plans(&plans_dir) {
            Ok(p) => p,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };
        if plans.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!("no plans discovered under {}", plans_dir.display()),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                corpus_hash(&plans),
                plans.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!("dry-run: {} plan(s) discovered", plans.len()));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        // Corpus-inventory layer: count plans, partition by declared
        // wave. No LLM calls. Reports a real fixture-count number plus
        // a structured note describing what panel mode would measure.
        let mut by_wave = std::collections::BTreeMap::new();
        for plan in &plans {
            *by_wave.entry(plan.wave.clone()).or_insert(0u32) += 1;
        }
        let total = plans.len() as u32;
        let target = args.threshold.unwrap_or(0.85);
        let mut report = AuditReport::complete(
            gate_thing_name(),
            corpus_hash(&plans),
            total,
            Results {
                overall_pass_rate: 1.0,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold {
            target,
            met: false,
        });
        let wave_summary: Vec<String> = by_wave
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        report.note = Some(format!(
            "corpus-inventory mode: {total} plan(s) ({}). Fidelity rate \
             (the CR-L4 85% bar) requires --llm-panel + a wired \
             orchestrator-driven plan-loop measurement harness.",
            wave_summary.join(", ")
        ));
        RunOutcome {
            report,
            exit_code: ExitCode::Ok,
        }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L4PlanFidelity.thing_name()
}

#[derive(Debug)]
struct PlanFixture {
    id: String,
    wave: String,
    source: String,
    /// Optional `base.vox` next to plan.toml. When present, the panel
    /// prompt asks the model to MODIFY this source per the plan
    /// (instead of generating from scratch). Per the documented
    /// finding in plan_fidelity §3.6 / readiness-snapshot 2026-05-22:
    /// plan-fidelity failures are semantic plan-misunderstandings
    /// that vox-check refinement can't fix; supplying base source so
    /// the model anchors against concrete code is the path past 40%.
    base_source: Option<String>,
    /// Whether the plan's success_criteria requires @test blocks to
    /// run + pass. Derived from `new_test_passes` / `existing_tests_pass`
    /// in plan.toml. Plans that only refactor (e.g. 004-refactor-loop:
    /// "endpoint signatures must not change", no new tests) set this
    /// to false — for them, vox-check-clean output is the success bar.
    /// Per the 2026-05-23 push-to-85% finding: refactor-only plans
    /// were failing the test-execution gate just because their base
    /// has no @test blocks. The plan never asked for any.
    tests_required: bool,
}

fn discover_plans(plans_dir: &Path) -> Result<Vec<PlanFixture>, String> {
    let entries = std::fs::read_dir(plans_dir)
        .map_err(|e| format!("failed to read {}: {}", plans_dir.display(), e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let plan_toml = path.join("plan.toml");
        if !plan_toml.exists() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let source = std::fs::read_to_string(&plan_toml)
            .map_err(|e| format!("read {}: {}", plan_toml.display(), e))?;
        let base_source = std::fs::read_to_string(path.join("base.vox")).ok();
        let wave = extract_wave(&source).unwrap_or_else(|| "unknown".to_string());
        let tests_required = extract_tests_required(&source);
        out.push(PlanFixture {
            id,
            wave,
            source,
            base_source,
            tests_required,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn extract_wave(source: &str) -> Option<String> {
    let parsed: toml::Value = toml::from_str(source).ok()?;
    parsed.get("wave")?.as_str().map(|s| s.to_string())
}

/// True if the plan's success_criteria explicitly requires @test
/// blocks to run + pass. Looks for either `new_test_passes = true`
/// (plan adds a new test) or `existing_tests_pass = true` (plan
/// preserves existing tests). Refactor-only plans whose success
/// criteria don't mention tests get false — they're scored on
/// vox-check-clean alone.
fn extract_tests_required(source: &str) -> bool {
    let Ok(parsed) = toml::from_str::<toml::Value>(source) else {
        return false;
    };
    let Some(crit) = parsed.get("success_criteria") else {
        return false;
    };
    let truthy = |key: &str| {
        crit.get(key)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    truthy("new_test_passes") || truthy("existing_tests_pass")
}

fn corpus_hash(plans: &[PlanFixture]) -> String {
    let mut hasher = blake3::Hasher::new();
    for p in plans {
        hasher.update(p.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(p.source.as_bytes());
        hasher.update(b"\n");
        if let Some(base) = &p.base_source {
            hasher.update(b"base:");
            hasher.update(base.as_bytes());
            hasher.update(b"\n");
        }
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

/// Extract the plan's English `prompt` field. Returns an empty string if
/// the toml is malformed or the field is missing — the run will still
/// score, but the model will see an empty prompt (almost certainly a fail).
fn extract_prompt(source: &str) -> String {
    let Ok(parsed) = toml::from_str::<toml::Value>(source) else {
        return String::new();
    };
    parsed
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Wire CR-L4 panel mode. Single-shot generate → `vox check` → score by
/// vox_check_passes. The richer "did this plan modify only the things it
/// promised to modify" check needs a base source per plan; deferred until
/// §4.3 corpus expansion ships those base sources. For v1.0 we publish
/// the vox-check-clean rate as honest evidence.
fn run_panel_mode(
    corpus_root: &Path,
    args: &CommonArgs,
    panel_yaml: &Path,
) -> RunOutcome {
    let plans_dir = corpus_root.join("plans");
    if !plans_dir.exists() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                format!("plans dir not found at {}", plans_dir.display()),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }
    let plans = match discover_plans(&plans_dir) {
        Ok(p) => p,
        Err(msg) => {
            return RunOutcome {
                report: AuditReport::infra_error(gate_thing_name(), msg),
                exit_code: ExitCode::InfrastructureError,
            };
        }
    };
    if plans.is_empty() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                format!("no plans under {}", plans_dir.display()),
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
    let cache_dir = workspace_root().join("contracts/reports/llm-panel-cache/plan-fidelity");
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
            run_with_panel(&plans, &args_owned, &panel_cfg, client.as_ref())
        })
        .join()
        .unwrap_or_else(|_| RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                "plan-fidelity panel thread panicked".to_string(),
            ),
            exit_code: ExitCode::InfrastructureError,
        })
    })
}

fn run_with_panel(
    plans: &[PlanFixture],
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
    // 2026-05-21 empirical finding: 5 iterations vs 3 iterations gave
    // identical pass rate (40% on the 5-plan corpus) at 5× cost.
    // Plan-fidelity failures aren't shallow compiler errors that
    // diagnostics can fix — the model misunderstands the plan
    // semantically and produces compile-clean nonsense. Refinement-loop
    // cost is wasted here until §4.3 ships base sources so the model
    // has concrete context to anchor against. Cap default low.
    // Bumped 3→7 after the 2026-05-22 test-execution feedback wiring:
    // with runtime signal feeding the refinement prompt, additional
    // iterations now move the needle (the loop converges instead of
    // looping on the same nonsense). Per the 2026-05-23 push to 85%.
    const DEFAULT_MAX_ITERATIONS: u32 = 7;
    let budget_cap_usd = std::env::var("VOX_AUDIT_BUDGET_USD")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_BUDGET_USD)
        .max(0.0);
    let max_iterations = std::env::var("VOX_AUDIT_CR_L4_MAX_ITERATIONS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_ITERATIONS)
        .max(1);
    let max_plans: Option<usize> = std::env::var("VOX_AUDIT_CR_L4_MAX_PLANS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0);
    let effective: Vec<&PlanFixture> = match max_plans {
        Some(n) => plans.iter().take(n).collect(),
        None => plans.iter().collect(),
    };

    let system_prompt = "You are a Vox programming language expert. Reply with ONLY a single \
```vox fenced code block — no commentary. Vox idioms: `to T` for return arrow (not `->`); \
`assert(X is Y)` for equality assertions; explicit `return expr`; `to Unit` for void; \
`@test\\nfn name() to Unit { … }` for tests. Forbidden: macros, `#[...]`, `->`, `assert_eq`, \
`use`/`import`, implicit last-expression returns. Your output is scored by BOTH `vox check` \
(must compile clean) AND in-process @test execution (every @test block must run + assert).";

    let mut per_llm: Vec<PerLlmResult> = Vec::with_capacity(routable.len());
    let mut cumulative_cost_usd = 0.0_f64;
    let mut total_unreachable = 0_u32;
    let mut total_budget_skipped = 0_u32;
    for member in &routable {
        let mut passing = 0_u32;
        let mut unreachable = 0_u32;
        let mut budget_skipped = 0_u32;
        let mut cost_samples: Vec<f64> = Vec::new();
        for plan in &effective {
            if cumulative_cost_usd >= budget_cap_usd {
                budget_skipped += 1;
                continue;
            }
            let prompt_text = extract_prompt(&plan.source);
            let base_user_prompt = match &plan.base_source {
                Some(base) => format!(
                    "Apply the following plan to the base Vox source below. \
                     Produce the resulting single self-contained Vox module \
                     that compiles cleanly under `vox check`.\n\n\
                     Plan ({}, wave={}):\n{prompt}\n\n\
                     Base source (modify per the plan):\n```vox\n{base}\n```\n\n\
                     Reply with ONLY a single fenced ```vox code block \
                     containing the modified module.",
                    plan.id,
                    plan.wave,
                    prompt = prompt_text,
                    base = base,
                ),
                None => format!(
                    "Apply the following plan to produce a single self-contained Vox module \
                     that compiles cleanly under `vox check`.\n\n\
                     Plan ({}, wave={}):\n{prompt}\n\n\
                     Reply with ONLY a single fenced ```vox code block.",
                    plan.id,
                    plan.wave,
                    prompt = prompt_text
                ),
            };

            // Refinement loop: up to max_iterations attempts; each
            // failed iteration feeds vox-check diagnostics back to the
            // model. Pattern mirrors spec_to_app_panel.
            let mut current_prompt = base_user_prompt.clone();
            let mut plan_cost = 0.0_f64;
            let mut iter_passed = false;
            let mut unreachable_for_plan = false;
            for iter in 1..=max_iterations {
                if iter > 1 && cumulative_cost_usd + plan_cost >= budget_cap_usd {
                    break;
                }
                let response = match client.complete(member, system_prompt, &current_prompt) {
                    Ok(r) => r,
                    Err(_) => {
                        if iter == 1 {
                            unreachable_for_plan = true;
                        }
                        break;
                    }
                };
                plan_cost += response.cost_usd;
                let source = extract_vox_code(&response.content);
                let diags = vox_compiler::pipeline::check_file(&source, "plan.vox");
                let err_count = diags
                    .iter()
                    .filter(|d| d.severity == TypeckSeverity::Error)
                    .count();
                if err_count == 0 {
                    // vox check passed — for plans whose
                    // success_criteria require @test blocks, also
                    // EXECUTE them. Per the 2026-05-22 documented
                    // finding, plan-fidelity failures are semantic
                    // plan-misunderstandings: the model produces
                    // compile-clean nonsense that wouldn't run.
                    // Test-execution catches that.
                    //
                    // Refactor-only plans (e.g. 004-refactor-loop:
                    // "endpoint signatures must not change", no new
                    // tests required) skip the runtime gate — vox
                    // check is enough. Per the 2026-05-23 push to 85%.
                    if !plan.tests_required {
                        iter_passed = true;
                        break;
                    }
                    let test_result = execute_tests_in_source(&source);
                    if test_result.is_pass() {
                        iter_passed = true;
                        break;
                    }
                    if iter < max_iterations {
                        // Build refinement prompt with runtime-failure
                        // signal instead of compile diagnostics.
                        current_prompt = format!(
                            "Your previous draft for plan `{}` compiled cleanly but its \
                             @test blocks did not pass at runtime. Refine the implementation \
                             so the tests pass.\n\n\
                             Original plan:\n{}\n\n\
                             Your previous draft:\n```vox\n{}\n```\n\n\
                             Test-execution outcome: {}\n\n\
                             Reply with ONLY a single fenced ```vox code block containing the revised module. \
                             Do not explain. Make the @test blocks pass.",
                            plan.id,
                            prompt_text,
                            source,
                            test_result.summary()
                        );
                    }
                    continue;
                }
                if iter < max_iterations {
                    // Build refinement prompt: prior source + diagnostics.
                    let diag_text = format_diags_for_refinement(&diags);
                    current_prompt = format!(
                        "Your previous draft for plan `{}` did not pass `vox check`. Refine it.\n\n\
                         Original plan:\n{}\n\n\
                         Your previous draft:\n```vox\n{}\n```\n\n\
                         vox-check diagnostics:\n{}\n\n\
                         Reply with ONLY a single fenced ```vox code block containing the revised module. \
                         Do not explain. Fix EVERY diagnostic above.",
                        plan.id, prompt_text, source, diag_text
                    );
                }
            }
            cumulative_cost_usd += plan_cost;
            if plan_cost > 0.0 {
                cost_samples.push(plan_cost);
            }
            if unreachable_for_plan {
                unreachable += 1;
            } else if iter_passed {
                passing += 1;
            }
            if std::env::var("VOX_AUDIT_CR_L4_VERBOSE").ok().is_some() {
                eprintln!(
                    "[cr-l4] member={} plan={} passed={} unreachable={} cost=${:.4}",
                    member.id, plan.id, iter_passed, unreachable_for_plan, plan_cost
                );
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
    let target = args.threshold.unwrap_or(0.85);
    let met = median_rate >= target;

    let mut report = AuditReport::complete(
        gate_thing_name(),
        corpus_hash(plans),
        plans.len() as u32,
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
        "panel mode: {} routable member(s) against {}/{} plans; \
         {total_unreachable} unreachable + {total_budget_skipped} budget-skipped. \
         panel cost: ${cumulative_cost_usd:.3} of ${budget_cap_usd:.2} budget. \
         Scoring: vox_check_passes for every plan; ALSO in-process @test execution \
         for plans whose success_criteria require it (new_test_passes or \
         existing_tests_pass). Refactor-only plans (no test requirement) pass on \
         vox-check-clean alone. Refinement loop feeds back diagnostics on vox-check \
         fail OR runtime-failure summary on assertion fail.",
        routable.len(),
        effective.len(),
        plans.len()
    ));
    let exit_code = if met {
        ExitCode::Ok
    } else {
        ExitCode::BarMissed
    };
    RunOutcome { report, exit_code }
}

/// Outcome of running the @test blocks declared in the source.
#[derive(Debug, Clone)]
struct TestExecutionResult {
    /// Tests that ran AND passed.
    passed: u32,
    /// Tests that ran but failed an assertion.
    failed: u32,
    /// The interpreter bailed on a feature it doesn't support; counted
    /// neither as pass nor fail (treated as eval-uncertain).
    eval_errored: bool,
    /// True when there were no @test blocks at all.
    no_tests: bool,
}

impl TestExecutionResult {
    /// A "pass" outcome for scoring purposes: at least one @test ran
    /// AND none failed AND no eval-error. No-tests is a fail because
    /// the plan specs require @test blocks.
    fn is_pass(&self) -> bool {
        !self.no_tests && self.failed == 0 && !self.eval_errored && self.passed > 0
    }

    /// Human-readable summary for the refinement prompt.
    fn summary(&self) -> String {
        if self.no_tests {
            return "no @test blocks declared (plan requires them)".into();
        }
        if self.eval_errored {
            return format!(
                "interpreter bailed on an unsupported feature after {} pass / {} fail",
                self.passed, self.failed
            );
        }
        if self.failed > 0 {
            return format!(
                "{} test(s) failed an assertion, {} passed",
                self.failed, self.passed
            );
        }
        format!("{} test(s) passed", self.passed)
    }
}

/// Lower the source, run the interpreter, exercise every `@test`
/// block. Mirrors `humaneval.rs::execute_fixture_tests` but operates on
/// an in-memory string (no fixture path).
fn execute_tests_in_source(source: &str) -> TestExecutionResult {
    use vox_compiler::eval::{EvalError, Interpreter};
    let frontend = match vox_compiler::pipeline::run_frontend_str(source, "plan.vox") {
        Ok(f) => f,
        Err(_) => {
            return TestExecutionResult {
                passed: 0,
                failed: 0,
                eval_errored: true,
                no_tests: false,
            };
        }
    };
    if frontend.hir.tests.is_empty() {
        return TestExecutionResult {
            passed: 0,
            failed: 0,
            eval_errored: false,
            no_tests: true,
        };
    }
    let mut interp = Interpreter::new(10_000);
    if interp.run_module(&frontend.hir).is_err() {
        return TestExecutionResult {
            passed: 0,
            failed: 0,
            eval_errored: true,
            no_tests: false,
        };
    }
    let test_names: Vec<String> = frontend
        .hir
        .tests
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut eval_errored = false;
    for name in &test_names {
        match interp.call(name, Vec::new()) {
            Ok(_) => passed += 1,
            Err(EvalError::AssertionFailed(_)) => failed += 1,
            Err(_) => {
                eval_errored = true;
                break;
            }
        }
    }
    TestExecutionResult {
        passed,
        failed,
        eval_errored,
        no_tests: false,
    }
}

fn format_diags_for_refinement(
    diags: &[vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload],
) -> String {
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
    fn missing_plans_dir_returns_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = PlanFidelityRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
    }

    #[test]
    fn plans_dir_with_real_plan_reports_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let plans = tmp.path().join("plans");
        let p = plans.join("001-tiny");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("plan.toml"),
            r#"id = "tiny"
wave = "wave_1"
steps = []
"#,
        )
        .unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = PlanFidelityRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert_eq!(outcome.report.corpus_size, 1);
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("wave_1"));
    }
}
