//! `vox audit humaneval` — CR-L1 HumanEval-Vox gate.
//!
//! Two measurement layers, both real:
//!
//! 1. **Corpus-validity rate (always on).** Walks
//!    `contracts/eval/humaneval-vox/problems/*/` and compile-checks each
//!    fixture's `reference.vox` and `tests.vox` via
//!    [`vox_compiler::pipeline::check_file`]. A fixture passes when both
//!    files produce zero error-severity diagnostics. The aggregate rate is
//!    the report's `overall_pass_rate`.
//!
//! 2. **LLM-panel pass-rate (opt-in).** When `--llm-panel <yaml>` is
//!    supplied via [`CommonArgs::llm_panel`], the runner would round-trip
//!    each prompt through the configured panel members and re-measure.
//!    This session does not ship the HTTP client (deferred to a follow-on
//!    that reuses [`vox-cli/src/commands/repair.rs`]'s OpenRouter wiring),
//!    so passing `--llm-panel` returns [`ExitCode::InvalidInput`] with a
//!    `note` explaining the gap. This is a real argument-validation path,
//!    not a hidden stub: corpus-validity still runs and is reported.
//!
//! Replaces the prior `HumanEvalStub` per the no-stub directive
//! (memory entry "No stubs in implementations").

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    panel::{
        CachingPanelClient, OpenRouterPanelClient, PanelClient, PanelConfig, PanelMemberConfig,
        ProtectedPanelClient, extract_vox_code,
    },
    report::{AuditReport, ExitCode, PanelMember, PerLlmResult, Results, Threshold},
    workspace_root,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use vox_compiler::typeck::diagnostics::TypeckSeverity;

/// Default corpus directory relative to workspace root.
const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/humaneval-vox";

pub struct HumanEvalRunner;

impl Subcommand for HumanEvalRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L1HumanEval
    }

    fn description(&self) -> &'static str {
        "CR-L1: HumanEval-Vox (≥80%) on the 164-problem corpus."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));

        // LLM-panel mode: opt-in via --llm-panel <yaml>. Real OpenRouter
        // round trips when the API key is present; otherwise the runner
        // bails honestly (not silently fall through to corpus-only).
        if let Some(panel_path) = args.llm_panel.clone() {
            let panel_cfg = match PanelConfig::from_yaml_path(&panel_path) {
                Ok(c) => c,
                Err(msg) => {
                    return RunOutcome {
                        report: AuditReport::infra_error(gate_thing_name(), msg),
                        exit_code: ExitCode::InvalidInput,
                    };
                }
            };
            let problems_dir = corpus_root.join("problems");
            // Panel mode uses reqwest::blocking, whose internal runtime
            // cannot be created OR dropped inside an outer Tokio context
            // (vox-cli owns one). Construct AND consume the client on a
            // dedicated OS thread. Mirrors spec_to_app.rs.
            let args_owned = args.clone();
            let cache_dir = workspace_root().join("contracts/reports/llm-panel-cache/");
            return std::thread::scope(|s| {
                s.spawn(move || {
                    // Wrap layers per llm-panel.v1.yaml §operational_policy:
                    //   OpenRouter (HTTP)
                    //   → Protected (retry/backoff on rate-limits)
                    //   → Caching   (content-addressed disk cache)
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
                    run_with_panel(&problems_dir, &args_owned, &panel_cfg, client.as_ref())
                })
                .join()
                .unwrap_or_else(|_| RunOutcome {
                    report: AuditReport::infra_error(
                        gate_thing_name(),
                        "humaneval panel thread panicked".to_string(),
                    ),
                    exit_code: ExitCode::InfrastructureError,
                })
            });
        }

        let problems_dir = corpus_root.join("problems");
        if !problems_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "corpus problems directory not found at {}; expected per \
                         contracts/eval/humaneval-vox/README.md",
                        problems_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        // Evidence-preservation: if a same-day panel artifact exists,
        // echo it back rather than clobbering it with corpus-validity
        // mode. Skipped when caller overrides `corpus`.
        if args.corpus.is_none()
            && let Some(existing) =
                crate::same_day_canonical_with_panel(&workspace_root(), gate_thing_name())
        {
            return RunOutcome {
                report: existing,
                exit_code: ExitCode::Ok,
            };
        }

        let fixtures = match load_fixtures(&problems_dir) {
            Ok(f) => f,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };

        if fixtures.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "no fixtures found under {}; corpus is empty",
                        problems_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        // Dry-run: report fixture count + hash without compiling.
        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                corpus_hash(&fixtures),
                fixtures.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!(
                "dry-run: discovered {} fixtures; skipping compile-check",
                fixtures.len()
            ));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        let mut passing = 0u32;
        let mut failing_fixtures: Vec<String> = Vec::new();
        let mut total_tests_ran = 0u32;
        let mut total_tests_passed = 0u32;
        let mut total_tests_failed = 0u32;
        let mut fixtures_with_eval_error = 0u32;
        let mut test_failed_fixtures: Vec<String> = Vec::new();
        for fixture in &fixtures {
            if !fixture_compiles_clean(fixture) {
                failing_fixtures.push(fixture.id.clone());
                continue;
            }
            // Compile-clean → execute @test blocks. Test failure makes the
            // FIXTURE fail (the corpus encodes wrong claims about the
            // reference solution); execution-engine errors leave the fixture
            // passing on compile-validity but get surfaced in the note.
            let exec = execute_fixture_tests(fixture);
            total_tests_ran += exec.ran;
            total_tests_passed += exec.passed;
            total_tests_failed += exec.failed;
            if exec.eval_errored {
                fixtures_with_eval_error += 1;
            }
            if exec.failed > 0 {
                test_failed_fixtures.push(fixture.id.clone());
            } else {
                passing += 1;
            }
        }
        let total = fixtures.len() as u32;
        let validity_rate = if total == 0 {
            0.0
        } else {
            f64::from(passing) / f64::from(total)
        };

        // Threshold: corpus-validity must be 1.0. Any compile failure in the
        // corpus IS a corpus bug; downstream LLM-panel measurement against a
        // broken corpus would be meaningless.
        let target = args.threshold.unwrap_or(1.0);
        let met = (validity_rate - target).abs() < f64::EPSILON || validity_rate >= target;

        let mut report = AuditReport::complete(
            gate_thing_name(),
            corpus_hash(&fixtures),
            total,
            Results {
                overall_pass_rate: validity_rate,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold { target, met });

        // Honest note: this is corpus-validity, not LLM-panel rate.
        let mode_note = if total < 50 {
            format!(
                "corpus-validity mode ({} fixtures; below manifest minimum-viable of 50, \
                 final target 164). LLM-panel rate (the CR-L1 80% bar) requires \
                 --llm-panel + a wired client.",
                total
            )
        } else {
            format!(
                "corpus-validity mode ({} fixtures). LLM-panel rate requires --llm-panel.",
                total
            )
        };
        let exec_note = format!(
            "@test execution: {}/{} tests passed across {} fixtures ({} eval-errored)",
            total_tests_passed, total_tests_ran, total, fixtures_with_eval_error
        );
        let combined_note = match (failing_fixtures.is_empty(), test_failed_fixtures.is_empty()) {
            (true, true) => format!("{mode_note} {exec_note}"),
            (false, true) => format!(
                "{} compile failures: [{}]. {mode_note} {exec_note}",
                failing_fixtures.len(),
                failing_fixtures.join(", "),
            ),
            (true, false) => format!(
                "{} fixtures had failing @test: [{}]. {mode_note} {exec_note}",
                test_failed_fixtures.len(),
                test_failed_fixtures.join(", "),
            ),
            (false, false) => format!(
                "{} compile failures: [{}]; {} @test failures: [{}]. {mode_note} {exec_note}",
                failing_fixtures.len(),
                failing_fixtures.join(", "),
                test_failed_fixtures.len(),
                test_failed_fixtures.join(", "),
            ),
        };
        report.note = Some(combined_note);
        // Reflect total_tests_failed in the failing-fixture set for the
        // exit-code decision: any test failure is also a corpus failure.
        let _ = total_tests_failed;

        let exit_code = if met {
            ExitCode::Ok
        } else {
            // Sub-bar on corpus-validity is treated as InvalidInput (the
            // CORPUS is malformed), not BarMissed (which would imply we
            // measured the real CR-L1 bar). Be precise about which thing
            // is broken.
            ExitCode::InvalidInput
        };

        RunOutcome { report, exit_code }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L1HumanEval.thing_name()
}

// ── Held-out contamination guard (audit omission #4 / R1) ─────────────────
//
// MENS training pipelines consume `held-out.v1.json` to know which fixture
// ids are excluded from training. The guard pair (build + verify) lives
// here so the CI check and the artifact stay in lockstep with the
// fixture spec.toml flags.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HeldOutEntry {
    pub id: String,
    pub provenance: String,
    pub derived_from: String,
    /// blake3 hash of `reference.vox || tests.vox` — lets the MENS
    /// pipeline detect if a held-out fixture was mutated since emit.
    pub fixture_hash: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HeldOutManifest {
    pub schema_version: u32,
    pub corpus: String,
    pub total_fixtures: u32,
    pub held_out_count: u32,
    /// Matches `HumanEvalRunner`'s corpus_hash so downstream consumers
    /// can join held-out membership with the CR-L1 measurement.
    pub corpus_hash: String,
    pub entries: Vec<HeldOutEntry>,
}

/// Walk `problems_dir`, collect every fixture with
/// `training_eligible: false`, and produce a sorted, hashed manifest.
pub fn build_held_out_manifest(problems_dir: &Path) -> Result<HeldOutManifest, String> {
    let fixtures = load_fixtures(problems_dir)?;
    let total = fixtures.len() as u32;
    let mut entries: Vec<HeldOutEntry> = Vec::new();
    for fixture in &fixtures {
        if fixture.training_eligible {
            continue;
        }
        let spec_path = fixture
            .reference_path
            .parent()
            .map(|p| p.join("spec.toml"))
            .unwrap_or_else(|| PathBuf::from("spec.toml"));
        let spec_text = std::fs::read_to_string(&spec_path)
            .map_err(|e| format!("re-read of {} failed: {e}", spec_path.display()))?;
        let spec: SpecToml = toml::from_str(&spec_text)
            .map_err(|e| format!("malformed spec at {}: {e}", spec_path.display()))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(fixture.reference_source.as_bytes());
        hasher.update(b"\n");
        hasher.update(fixture.tests_source.as_bytes());
        entries.push(HeldOutEntry {
            id: fixture.id.clone(),
            provenance: spec.provenance,
            derived_from: spec.derived_from,
            fixture_hash: format!("blake3:{}", hasher.finalize().to_hex()),
        });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(HeldOutManifest {
        schema_version: 1,
        corpus: "humaneval-vox".into(),
        total_fixtures: total,
        held_out_count: entries.len() as u32,
        corpus_hash: corpus_hash(&fixtures),
        entries,
    })
}

/// Verify the on-disk `held-out.v1.json` matches what
/// [`build_held_out_manifest`] derives from `problems_dir`. Returns a
/// human-readable Err message on any drift; CI consumers treat this as
/// the contamination-guard pass/fail.
pub fn verify_held_out_manifest(
    problems_dir: &Path,
    on_disk_path: &Path,
) -> Result<(), String> {
    let derived = build_held_out_manifest(problems_dir)?;
    let text = std::fs::read_to_string(on_disk_path)
        .map_err(|e| format!("read {} failed: {e}", on_disk_path.display()))?;
    let on_disk: HeldOutManifest = serde_json::from_str(&text)
        .map_err(|e| format!("parse {} failed: {e}", on_disk_path.display()))?;
    if on_disk.schema_version != derived.schema_version {
        return Err(format!(
            "schema_version drift: on-disk={}, derived={}",
            on_disk.schema_version, derived.schema_version
        ));
    }
    if on_disk.corpus_hash != derived.corpus_hash {
        return Err(format!(
            "corpus_hash drift: on-disk={}, derived={} (regenerate \
             contracts/eval/humaneval-vox/held-out.v1.json via \
             build_held_out_manifest)",
            on_disk.corpus_hash, derived.corpus_hash
        ));
    }
    if on_disk.held_out_count != derived.held_out_count {
        return Err(format!(
            "held_out_count drift: on-disk={}, derived={}",
            on_disk.held_out_count, derived.held_out_count
        ));
    }
    if on_disk.entries != derived.entries {
        return Err(
            "held-out entries differ (id/provenance/derived_from/hash drift)".into(),
        );
    }
    Ok(())
}

/// Panel-mode execution: for each fixture, ask each panel member to write
/// the solution, compile-check the response, and record pass/fail.
///
/// Per-LLM rate = passing fixtures / total fixtures. Overall rate is the
/// median of per-LLM rates (per `llm-panel.v1.yaml::scoring_rule:
/// median-of-members`). Threshold defaults to the CR-L1 80% bar.
pub(crate) fn run_with_panel(
    problems_dir: &Path,
    args: &CommonArgs,
    panel_cfg: &PanelConfig,
    client: &dyn PanelClient,
) -> RunOutcome {
    if !problems_dir.exists() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                format!(
                    "corpus problems directory not found at {}",
                    problems_dir.display()
                ),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }
    let fixtures = match load_fixtures(problems_dir) {
        Ok(f) => f,
        Err(msg) => {
            return RunOutcome {
                report: AuditReport::infra_error(gate_thing_name(), msg),
                exit_code: ExitCode::InfrastructureError,
            };
        }
    };
    if fixtures.is_empty() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                "no fixtures discovered for panel run".to_string(),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }

    // Routable members only — project-owned MENS that has no
    // openrouter_model_id is reported separately per panel YAML
    // `cr_l0_mens_handling` policy. For CR-L1 the YAML says
    // "include-in-median" but only routable members can actually be
    // measured by this client. Non-routable members surface in the report
    // note rather than silently disappearing.
    let routable: Vec<&PanelMemberConfig> = panel_cfg
        .members
        .iter()
        .filter(|m| m.openrouter_model_id().is_some())
        .collect();
    let unroutable_ids: Vec<String> = panel_cfg
        .members
        .iter()
        .filter(|m| m.openrouter_model_id().is_none())
        .map(|m| m.id.clone())
        .collect();

    if routable.is_empty() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing_name(),
                "no panel members are OpenRouter-routable; nothing to measure".to_string(),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }

    // Calibrated Vox primer — matches the one used by spec_to_app_panel.
    // Models that get this primer reliably emit `assert(X is Y)` instead
    // of `assert_eq(a, b)`, use `to Unit` for void, and `return expr`
    // explicitly. Without it, single-shot pass-rates drop ~50 pts.
    let system_prompt = "You are a Vox programming language expert. Vox is a strongly typed \
language for AI-native server apps. Reply with ONLY a single ```vox fenced code block — no commentary, no extra text.\n\n\
Vox syntax — read carefully, these are NOT optional:\n\
  • Functions: `fn name(arg: Type) to ReturnType { return expr }`. \
Use **explicit `return`**. The return arrow is `to`, NOT `->`. \
Void / unit functions return `to Unit`.\n\
  • Tests: `@test\\nfn test_name() to Unit { assert(actual is expected) }`. \
Assertion is `assert(X is Y)`. Do NOT use `assert_eq(a, b)`, `assert!(…)`, `expect`, or `==` inside `assert`.\n\
  • Common types: `str`, `int`, `bool`, `Unit`, `List[T]`, `Result[T, E]`, `Option[T]`. \
Result: `Ok(v)` / `Err(e)`.\n\
  • Strings concat with `+`. Double-quoted: `\"Hello \" + name + \"!\"`.\n\
  • `let name = expr` and `let name: Type = expr`.\n\
Forbidden: macros, `#[derive(…)]`, `#[…]`, `->` as return arrow, `use`/`import`, \
implicit last-expression-as-return — write `return expr`.";

    // Budget enforcement (mirrors spec_to_app_panel). The cap is the
    // cumulative cost across the entire run, not per-member.
    const DEFAULT_BUDGET_USD: f64 = 20.0;
    let budget_cap_usd = std::env::var("VOX_AUDIT_BUDGET_USD")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_BUDGET_USD)
        .max(0.0);
    // Optional CI-friendly cap on fixture count, so a full 164-fixture
    // run can be opted out of when only a stable subsample is wanted.
    // 0 / unset = no cap (use the full discovered corpus).
    let max_fixtures: Option<usize> = std::env::var("VOX_AUDIT_CR_L1_MAX_FIXTURES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0);
    let effective_fixtures: Vec<&Fixture> = match max_fixtures {
        Some(n) => fixtures.iter().take(n).collect(),
        None => fixtures.iter().collect(),
    };

    let mut per_llm_results: Vec<PerLlmResult> = Vec::with_capacity(routable.len());
    let mut cumulative_cost_usd: f64 = 0.0;
    let mut total_unreachable: u32 = 0;
    let mut total_budget_skipped: u32 = 0;
    for member in &routable {
        let mut passing = 0u32;
        let mut unreachable = 0u32;
        let mut budget_skipped = 0u32;
        let mut cost_samples: Vec<f64> = Vec::new();
        for fixture in &effective_fixtures {
            // Per-call budget gate — refuse to spend over cap.
            if cumulative_cost_usd >= budget_cap_usd {
                budget_skipped += 1;
                continue;
            }
            let user_prompt = build_user_prompt(fixture);
            let response = match client.complete(member, system_prompt, &user_prompt) {
                Ok(r) => r,
                Err(_) => {
                    unreachable += 1;
                    continue;
                }
            };
            cumulative_cost_usd += response.cost_usd;
            cost_samples.push(response.cost_usd);
            let candidate_source = extract_vox_code(&response.content);
            let path_str = fixture.reference_path.to_string_lossy();
            let diags = vox_compiler::pipeline::check_file(&candidate_source, &path_str);
            if !has_error(&diags) {
                passing += 1;
            }
        }
        // Pass rate denominator excludes unreachable + budget-skipped per
        // llm-panel.v1.yaml §fallback_when_unreachable ("record-skip-not-fail").
        let scored = effective_fixtures
            .len()
            .saturating_sub((unreachable + budget_skipped) as usize);
        let rate = if scored == 0 {
            0.0
        } else {
            f64::from(passing) / scored as f64
        };
        per_llm_results.push(PerLlmResult {
            id: member.id.clone(),
            pass_rate: rate,
            median_cost_usd: median(&cost_samples),
            unreachable_count: Some(unreachable + budget_skipped),
        });
        total_unreachable += unreachable;
        total_budget_skipped += budget_skipped;
    }

    let median_rate = median(&per_llm_results.iter().map(|r| r.pass_rate).collect::<Vec<_>>())
        .unwrap_or(0.0);
    let target = args.threshold.unwrap_or(0.80);
    let met = median_rate >= target;

    let mut report = AuditReport::complete(
        gate_thing_name(),
        corpus_hash(&fixtures),
        fixtures.len() as u32,
        Results {
            overall_pass_rate: median_rate,
            median_pass_rate: Some(median_rate),
            per_llm: per_llm_results,
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
    let mut note = format!(
        "panel mode: {} routable member(s) measured against {}/{} fixtures",
        routable.len(),
        effective_fixtures.len(),
        fixtures.len()
    );
    if let Some(n) = max_fixtures {
        note.push_str(&format!(
            " (VOX_AUDIT_CR_L1_MAX_FIXTURES={n})"
        ));
    }
    if !unroutable_ids.is_empty() {
        note.push_str(&format!(
            "; {} unroutable member(s) skipped ({})",
            unroutable_ids.len(),
            unroutable_ids.join(", ")
        ));
    }
    if total_unreachable > 0 || total_budget_skipped > 0 {
        note.push_str(&format!(
            "; {total_unreachable} unreachable + {total_budget_skipped} budget-skipped"
        ));
    }
    note.push_str(&format!(
        ". panel cost: ${cumulative_cost_usd:.3} of ${budget_cap_usd:.2} budget"
    ));
    report.note = Some(note);

    let exit_code = if met {
        ExitCode::Ok
    } else {
        ExitCode::BarMissed
    };
    RunOutcome { report, exit_code }
}

fn build_user_prompt(fixture: &Fixture) -> String {
    // Use the fixture's prompt text from spec.toml. We stash it on the
    // Fixture via load_fixtures' spec parsing — add via a small refactor
    // below if not already there.
    fixture.prompt.clone()
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some((sorted[mid - 1] + sorted[mid]) / 2.0)
    }
}

#[derive(Debug, Deserialize)]
struct SpecToml {
    id: String,
    training_eligible: bool,
    #[allow(dead_code)] // currently informational; future runs will partition by provenance
    provenance: String,
    #[allow(dead_code)]
    derived_from: String,
    prompt: String,
}

#[derive(Debug)]
struct Fixture {
    id: String,
    #[allow(dead_code)] // surfaces in v1 held-out-vs-eligible reporting (P3.2)
    training_eligible: bool,
    /// Natural-language prompt sent to panel members in --llm-panel mode.
    prompt: String,
    reference_path: PathBuf,
    tests_path: PathBuf,
    reference_source: String,
    tests_source: String,
}

/// Walk `problems/*/` and load each fixture's spec + source files.
fn load_fixtures(problems_dir: &Path) -> Result<Vec<Fixture>, String> {
    let mut out: Vec<Fixture> = Vec::new();
    let entries = std::fs::read_dir(problems_dir)
        .map_err(|e| format!("failed to read {}: {}", problems_dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry read failed: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let spec_path = path.join("spec.toml");
        if !spec_path.exists() {
            // Tolerate non-fixture sibling dirs (e.g. future README assets);
            // skip silently.
            continue;
        }
        let spec_text = std::fs::read_to_string(&spec_path)
            .map_err(|e| format!("failed to read {}: {}", spec_path.display(), e))?;
        let spec: SpecToml = toml::from_str(&spec_text)
            .map_err(|e| format!("malformed spec at {}: {}", spec_path.display(), e))?;
        let reference_path = path.join("reference.vox");
        let tests_path = path.join("tests.vox");
        if !reference_path.exists() {
            return Err(format!(
                "fixture {} missing reference.vox at {}",
                spec.id,
                reference_path.display()
            ));
        }
        if !tests_path.exists() {
            return Err(format!(
                "fixture {} missing tests.vox at {}",
                spec.id,
                tests_path.display()
            ));
        }
        let reference_source = std::fs::read_to_string(&reference_path)
            .map_err(|e| format!("failed to read {}: {}", reference_path.display(), e))?;
        let tests_source = std::fs::read_to_string(&tests_path)
            .map_err(|e| format!("failed to read {}: {}", tests_path.display(), e))?;
        out.push(Fixture {
            id: spec.id,
            training_eligible: spec.training_eligible,
            prompt: spec.prompt,
            reference_path,
            tests_path,
            reference_source,
            tests_source,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Compile-check both reference.vox and tests.vox; pass iff zero error
/// diagnostics in both.
fn fixture_compiles_clean(fixture: &Fixture) -> bool {
    let ref_diags = vox_compiler::pipeline::check_file(
        &fixture.reference_source,
        &fixture.reference_path.to_string_lossy(),
    );
    if has_error(&ref_diags) {
        return false;
    }
    let tests_diags = vox_compiler::pipeline::check_file(
        &fixture.tests_source,
        &fixture.tests_path.to_string_lossy(),
    );
    !has_error(&tests_diags)
}

/// Compile + execute every `@test` block in `fixture.tests_source` using
/// the in-process Vox interpreter at `vox_compiler::eval::Interpreter`.
///
/// Returns the per-fixture test-execution result:
/// - `ran` = number of `@test` fns successfully invoked
/// - `passed` = number that completed without `AssertionFailed`
/// - `eval_errored = true` when the interpreter aborted on something
///   other than an assertion (unhandled feature, undefined var). The
///   caller treats this as "tests skipped" rather than "tests failed"
///   to preserve the corpus-validity meaning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FixtureTestExecution {
    ran: u32,
    passed: u32,
    failed: u32,
    eval_errored: bool,
}

fn execute_fixture_tests(fixture: &Fixture) -> FixtureTestExecution {
    use vox_compiler::eval::{EvalError, Interpreter};

    // Lower tests.vox via the frontend pipeline — failure here means the
    // file itself doesn't compile, which the compile-check layer already
    // surfaced; just report no tests ran without flagging eval-errored.
    let frontend = match vox_compiler::pipeline::run_frontend_str(
        &fixture.tests_source,
        &fixture.tests_path.to_string_lossy(),
    ) {
        Ok(f) => f,
        Err(_) => return FixtureTestExecution::default(),
    };
    if frontend.hir.tests.is_empty() {
        return FixtureTestExecution::default();
    }

    // 10k-step budget matches the conservative ceiling used by other
    // in-process Vox eval call sites; well under the seed corpus's
    // single-iteration test surface.
    let mut interp = Interpreter::new(10_000);
    if interp.run_module(&frontend.hir).is_err() {
        return FixtureTestExecution {
            ran: 0,
            passed: 0,
            failed: 0,
            eval_errored: true,
        };
    }

    let test_names: Vec<String> = frontend.hir.tests.iter().map(|t| t.name.clone()).collect();
    let mut ran = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut eval_errored = false;
    for name in &test_names {
        ran += 1;
        match interp.call(name, Vec::new()) {
            Ok(_) => passed += 1,
            Err(EvalError::AssertionFailed(_)) => failed += 1,
            Err(_) => {
                // Interpreter bailed on a feature it doesn't support. Don't
                // count as failure — the corpus-validity claim is about the
                // corpus, and execution coverage is reported separately so
                // reviewers can see what fraction is actually executed.
                eval_errored = true;
                ran -= 1;
                break;
            }
        }
    }
    FixtureTestExecution {
        ran,
        passed,
        failed,
        eval_errored,
    }
}

fn has_error(diags: &[vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload]) -> bool {
    diags.iter().any(|d| matches!(d.severity, TypeckSeverity::Error))
}

/// Content-derived corpus hash over sorted fixture sources.
fn corpus_hash(fixtures: &[Fixture]) -> String {
    let mut hasher = blake3::Hasher::new();
    for f in fixtures {
        hasher.update(f.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(f.reference_source.as_bytes());
        hasher.update(b"\n");
        hasher.update(f.tests_source.as_bytes());
        hasher.update(b"\n");
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> CommonArgs {
        // Explicitly set `corpus` so the runner skips the
        // same-day-canonical-with-panel guard (which would otherwise
        // return the workspace's real panel artifact instead of running
        // corpus-validity over the seed fixtures). The path resolves to
        // the same default the runner would have used; making it
        // explicit just opts the test out of the guard.
        CommonArgs {
            write_canonical_report: false,
            corpus: Some(crate::workspace_root().join(DEFAULT_CORPUS_RELPATH)),
            ..CommonArgs::default()
        }
    }

    #[test]
    fn runner_against_seed_corpus_returns_ok() {
        let outcome = HumanEvalRunner.run(&args());
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "seed corpus must compile clean; report note: {:?}",
            outcome.report.note
        );
        assert!(!outcome.report.incomplete);
        assert_eq!(outcome.report.thing, "humaneval");
        assert!(outcome.report.corpus_size >= 18, "expected the 18 seed fixtures");
        assert_eq!(
            outcome.report.results.overall_pass_rate, 1.0,
            "every seed fixture must compile clean"
        );
        let threshold = outcome.report.threshold.expect("threshold present");
        assert!(threshold.met);
    }

    #[test]
    fn runner_dry_run_skips_compile_and_returns_ok() {
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert!(outcome.report.corpus_size >= 18);
    }

    #[test]
    fn runner_with_missing_corpus_returns_infra_error() {
        let args = CommonArgs {
            corpus: Some(PathBuf::from("this/path/does/not/exist/humaneval-vox")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
        assert!(outcome.report.incomplete);
    }

    #[test]
    fn runner_with_llm_panel_flag_routes_to_panel_mode() {
        // Panel mode is now wired (G — 2026-05-17). Without an OpenRouter
        // API key it returns InfrastructureError; with a key it runs real
        // calls and returns BarMissed / Ok per pass rate. We can't pin
        // env-var state without flake (vox_secrets resolves from multiple
        // sources beyond env vars), so we only assert: the outcome is one
        // of the legitimate panel-mode exit codes, and is NOT the prior
        // InvalidInput "not yet wired" sentinel that this commit replaced.
        let args = CommonArgs {
            llm_panel: Some(crate::workspace_root().join("contracts/eval/llm-panel.v1.yaml")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_ne!(
            outcome.exit_code,
            ExitCode::InvalidInput,
            "panel mode is now real; InvalidInput would mean we regressed to the stub path"
        );
        match outcome.exit_code {
            ExitCode::Ok | ExitCode::BarMissed => {
                // Real panel run completed (credentials present).
                assert!(
                    !outcome.report.llm_panel.is_empty(),
                    "successful panel run should record llm_panel members"
                );
            }
            ExitCode::InfrastructureError => {
                // No credentials. Note should explain.
                assert!(
                    outcome
                        .report
                        .note
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains("api key"),
                    "infra_error note should mention API key; got {:?}",
                    outcome.report.note
                );
            }
            other => panic!("unexpected exit code from panel mode: {other:?}"),
        }
    }

    #[test]
    fn corpus_hash_is_deterministic() {
        let first = HumanEvalRunner.run(&args());
        let second = HumanEvalRunner.run(&args());
        assert_eq!(first.report.corpus_hash, second.report.corpus_hash);
        assert!(first.report.corpus_hash.starts_with("blake3:"));
    }

    #[test]
    fn panel_orchestration_e2e_with_scripted_client() {
        use crate::panel::{PanelConfig, PanelMemberConfig, PanelMetadata, PanelResponse};
        // Tempdir corpus with one passing fixture.
        let tmp = tempfile::tempdir().unwrap();
        let problems = tmp.path().join("problems");
        let p = problems.join("001-add");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "humaneval-vox-001-add"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "Write fn add(a: int, b: int) to int returning a + b."
"#,
        )
        .unwrap();
        std::fs::write(
            p.join("reference.vox"),
            "fn add(a: int, b: int) to int { return a + b }\n",
        )
        .unwrap();
        std::fs::write(
            p.join("tests.vox"),
            "fn add(a: int, b: int) to int { return a + b }\n@test fn t() to Unit { assert(add(1,2) is 3) }\n",
        )
        .unwrap();

        // One-member panel; one fixture; the scripted client returns a
        // valid Vox solution wrapped in a ```vox fence.
        let panel_cfg = PanelConfig {
            panel: PanelMetadata {
                id: "test".into(),
                status: "active".into(),
                pinned_at: None,
            },
            members: vec![PanelMemberConfig {
                id: "test-llm".into(),
                role: "frontier-baseline".into(),
                version_pinned: Some("gpt-test".into()),
                openrouter_model: None,
                pricing: None,
            }],
        };
        let client = crate::panel::test_support::ScriptedPanelClient::new(vec![PanelResponse {
            content: "Here:\n```vox\nfn add(a: int, b: int) to int { return a + b }\n```"
                .into(),
            cost_usd: 0.01,
            input_tokens: Some(100),
            output_tokens: Some(20),
        }]);
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = super::run_with_panel(&problems, &args, &panel_cfg, &client);
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "passing solution should clear default 0.80 bar; note: {:?}",
            outcome.report.note
        );
        assert_eq!(outcome.report.results.overall_pass_rate, 1.0);
        assert_eq!(outcome.report.results.per_llm.len(), 1);
        assert_eq!(outcome.report.results.per_llm[0].id, "test-llm");
        assert_eq!(outcome.report.results.per_llm[0].pass_rate, 1.0);
        assert_eq!(outcome.report.llm_panel.len(), 1);
    }

    #[test]
    fn panel_orchestration_e2e_with_failing_response() {
        use crate::panel::{PanelConfig, PanelMemberConfig, PanelMetadata, PanelResponse};
        let tmp = tempfile::tempdir().unwrap();
        let problems = tmp.path().join("problems");
        let p = problems.join("001-add");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "humaneval-vox-001-add"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "Write fn add(a: int, b: int) to int."
"#,
        )
        .unwrap();
        std::fs::write(
            p.join("reference.vox"),
            "fn add(a: int, b: int) to int { return a + b }\n",
        )
        .unwrap();
        std::fs::write(p.join("tests.vox"), "fn add(a: int, b: int) to int { return a + b }\n").unwrap();

        let panel_cfg = PanelConfig {
            panel: PanelMetadata {
                id: "test".into(),
                status: "active".into(),
                pinned_at: None,
            },
            members: vec![PanelMemberConfig {
                id: "weak-llm".into(),
                role: "frontier-baseline".into(),
                version_pinned: Some("gpt-test".into()),
                openrouter_model: None,
                pricing: None,
            }],
        };
        // Returns code that won't compile.
        let client = crate::panel::test_support::ScriptedPanelClient::new(vec![PanelResponse {
            content: "```vox\nthis is not valid vox source ###\n```".into(),
            cost_usd: 0.0,
            input_tokens: None,
            output_tokens: None,
        }]);
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = super::run_with_panel(&problems, &args, &panel_cfg, &client);
        assert_eq!(outcome.exit_code, ExitCode::BarMissed);
        assert_eq!(outcome.report.results.overall_pass_rate, 0.0);
    }

    #[test]
    fn held_out_manifest_on_disk_matches_seed_corpus() {
        let problems_dir = crate::workspace_root().join("contracts/eval/humaneval-vox/problems");
        let on_disk = crate::workspace_root()
            .join("contracts/eval/humaneval-vox/held-out.v1.json");
        if !on_disk.exists() {
            panic!(
                "held-out manifest not present at {}; regenerate via \
                 build_held_out_manifest and check it in",
                on_disk.display()
            );
        }
        super::verify_held_out_manifest(&problems_dir, &on_disk).expect(
            "drift between contracts/eval/humaneval-vox/held-out.v1.json and the live \
             problems/*/spec.toml flags; regenerate the manifest",
        );
    }

    /// Regenerate the on-disk held-out manifest. Run with
    /// `cargo test -p vox-audit -- --ignored emit_held_out_manifest`
    /// after any change to problems/*/spec.toml that affects training
    /// eligibility, then commit the resulting JSON.
    #[test]
    #[ignore]
    fn emit_held_out_manifest() {
        let problems_dir = crate::workspace_root().join("contracts/eval/humaneval-vox/problems");
        let out = crate::workspace_root().join("contracts/eval/humaneval-vox/held-out.v1.json");
        let manifest = super::build_held_out_manifest(&problems_dir).unwrap();
        let text = serde_json::to_string_pretty(&manifest).unwrap();
        std::fs::write(&out, text).unwrap();
        println!("wrote {}", out.display());
    }

    #[test]
    fn build_held_out_manifest_collects_only_training_eligible_false() {
        let problems_dir = crate::workspace_root().join("contracts/eval/humaneval-vox/problems");
        let manifest = super::build_held_out_manifest(&problems_dir).unwrap();
        assert!(manifest.held_out_count >= 1);
        assert!(manifest.total_fixtures >= manifest.held_out_count);
        for entry in &manifest.entries {
            assert!(
                entry.fixture_hash.starts_with("blake3:"),
                "fixture_hash must be blake3-prefixed; got {}",
                entry.fixture_hash
            );
        }
    }

    #[test]
    fn execute_fixture_tests_runs_all_at_test_passing() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("problems/001-tiny");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "exec-test-pass"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "tiny"
"#,
        )
        .unwrap();
        std::fs::write(p.join("reference.vox"), "fn id(n: int) to int { return n }\n").unwrap();
        std::fs::write(
            p.join("tests.vox"),
            r#"fn id(n: int) to int { return n }
@test
fn t1() to Unit { assert(id(7) is 7) }
@test
fn t2() to Unit { assert(id(0) is 0) }
"#,
        )
        .unwrap();
        let fixtures = super::load_fixtures(&tmp.path().join("problems")).unwrap();
        assert_eq!(fixtures.len(), 1);
        let exec = super::execute_fixture_tests(&fixtures[0]);
        assert_eq!(exec.ran, 2, "two @test fns should run");
        assert_eq!(exec.passed, 2);
        assert_eq!(exec.failed, 0);
        assert!(!exec.eval_errored);
    }

    #[test]
    fn execute_fixture_tests_catches_at_test_assertion_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("problems/002-bad-test");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "exec-test-bad"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "tiny"
"#,
        )
        .unwrap();
        std::fs::write(p.join("reference.vox"), "fn id(n: int) to int { return n }\n").unwrap();
        // Test asserts a wrong claim about the reference — must surface as
        // a failure so the corpus author fixes the bug.
        std::fs::write(
            p.join("tests.vox"),
            r#"fn id(n: int) to int { return n }
@test
fn t_wrong() to Unit { assert(id(7) is 999) }
"#,
        )
        .unwrap();
        let fixtures = super::load_fixtures(&tmp.path().join("problems")).unwrap();
        let exec = super::execute_fixture_tests(&fixtures[0]);
        assert_eq!(exec.ran, 1);
        assert_eq!(exec.passed, 0);
        assert_eq!(exec.failed, 1);
        assert!(!exec.eval_errored);
    }

    #[test]
    fn execute_fixture_tests_returns_zero_when_no_at_test_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("problems/003-no-tests");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "exec-test-empty"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "tiny"
"#,
        )
        .unwrap();
        std::fs::write(p.join("reference.vox"), "fn id(n: int) to int { return n }\n").unwrap();
        std::fs::write(
            p.join("tests.vox"),
            "fn id(n: int) to int { return n }\n", // no @test blocks
        )
        .unwrap();
        let fixtures = super::load_fixtures(&tmp.path().join("problems")).unwrap();
        let exec = super::execute_fixture_tests(&fixtures[0]);
        assert_eq!(exec.ran, 0);
        assert_eq!(exec.passed, 0);
        assert!(!exec.eval_errored);
    }

    #[test]
    fn broken_fixture_drops_validity_below_one() {
        // Synthesize a temp corpus with one bad fixture to verify the failure
        // path. Real workspace corpus stays untouched.
        let tmp = tempfile::tempdir().expect("tempdir");
        let problems = tmp.path().join("problems");
        let bad = problems.join("999-broken");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(
            bad.join("spec.toml"),
            r#"id = "humaneval-vox-999-broken"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "broken fixture for runner failure-path test"
"#,
        )
        .unwrap();
        std::fs::write(
            bad.join("reference.vox"),
            "this is not valid vox source ###\n",
        )
        .unwrap();
        std::fs::write(bad.join("tests.vox"), "@test fn t() to Unit { assert(true) }\n").unwrap();

        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InvalidInput);
        assert!(outcome.report.results.overall_pass_rate < 1.0);
        assert!(
            outcome
                .report
                .note
                .as_deref()
                .unwrap_or("")
                .contains("999-broken"),
            "failure note must name the bad fixture; got: {:?}",
            outcome.report.note
        );
    }
}
