//! Unified **plan refinement** signals: MCP-style per-task gap heuristics plus plan-level **adequacy**
//! (detecting structurally thin plans for a goal's estimated complexity).
//!
//! Used by MCP `vox_plan` and orchestrator-native `PlanNode` DAGs so behavior stays aligned.

use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Minimal task shape for analysis (MCP `PlanTask` maps 1:1).
#[derive(Debug, Clone, Serialize)]
pub struct PlanAdequacyTask {
    /// Monotonic task index (1-based for MCP).
    pub id: usize,
    pub description: String,
    pub files: Vec<String>,
    pub estimated_complexity: u8,
    pub depends_on: Vec<usize>,
    #[serde(default)]
    pub test_decision: Option<crate::planning::test_decision::TestDecision>,
    /// Explicit preconditions that must hold before this task executes.
    /// Research (Plan Adequacy §3) proves state-mutating steps without
    /// preconditions are a primary source of silent plan failures.
    #[serde(default)]
    pub preconditions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskGapFinding {
    pub task_id: usize,
    pub reason_codes: Vec<String>,
    /// 0.0 (weak / risky) .. 1.0 (concrete / safe)
    pub task_confidence: f32,
}

/// LLM-as-judge rubric scores from Socrates evaluation (each 0–10).
/// Research (Plan Adequacy §2) proves regex/keyword vagueness detection is
/// trivially gamed; structured rubric evaluation catches semantic gaps.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RubricScores {
    /// Does the plan cover all goal requirements?
    pub coverage: u8,
    /// Are preconditions and dependencies correctly ordered?
    pub dependency_ordering: u8,
    /// Are destructive operations properly safeguarded?
    pub destructive_safety: u8,
    /// Are action verbs clear, unvague, and distinct?
    pub concreteness: u8,
    /// Is there a test, assertion, or validation step?
    pub verification: u8,
}

impl RubricScores {
    /// Weighted aggregate score normalised to `[0.0, 1.0]`.
    pub fn weighted_score(&self) -> f32 {
        let raw = (self.coverage as f32 * 0.25)
            + (self.dependency_ordering as f32 * 0.15)
            + (self.destructive_safety as f32 * 0.25)
            + (self.concreteness as f32 * 0.20)
            + (self.verification as f32 * 0.15);
        (raw / 10.0).clamp(0.0, 1.0)
    }

    /// Build from the tuple returned by [`super::orient::SocratesPlanJudge::parse_evaluation_scores`].
    pub fn from_tuple(t: (u8, u8, u8, u8, u8)) -> Self {
        Self {
            coverage: t.0,
            dependency_ordering: t.1,
            destructive_safety: t.2,
            concreteness: t.3,
            verification: t.4,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanAdequacySummary {
    /// 0.0 = structurally thin .. 1.0 = adequate for estimated complexity
    pub score: f32,
    pub is_too_thin: bool,
    pub reason_codes: Vec<String>,
    pub detail_target_min_tasks: usize,
    pub estimated_goal_complexity: u8,
    /// LLM-as-judge rubric scores (populated when Socrates evaluation is available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubric_scores: Option<RubricScores>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanRefinementReport {
    pub per_task: Vec<TaskGapFinding>,
    pub aggregate_unresolved_risk: f32,
    pub critical_count: usize,
    pub suggested_clarifying_questions: Vec<String>,
    pub adequacy: PlanAdequacySummary,
}

/// Replaced word-count proxy. Now relies exclusively on explicit router or Socrates complexity sizing.
pub fn effective_goal_complexity(goal: &str, router_hint: Option<u8>) -> u8 {
    let band =
        vox_orchestrator_types::socrates_policy::SocratesComplexityJudge::estimate_complexity(
            goal,
            router_hint,
        );
    match band {
        vox_orchestrator_types::socrates_policy::ComplexityBand::Simple => 2,
        vox_orchestrator_types::socrates_policy::ComplexityBand::Moderate => 4,
        vox_orchestrator_types::socrates_policy::ComplexityBand::Complex => 6,
        vox_orchestrator_types::socrates_policy::ComplexityBand::MultiHop => 8,
    }
}

/// Structural text checks for orchestrator [`super::quality_gate`].
///
/// Research (Plan Adequacy §2) proves keyword blacklists (`TBD`, `figure out`)
/// generate mass false negatives. These checks focus on *structural* defects
/// that are objectively detectable without LLM inference:
/// - Descriptions too short to be actionable
/// - Missing action verbs (passive/vague phrasing)
/// - Unguarded destructive operations
pub fn orchestrator_node_text_findings(description: &str) -> Vec<&'static str> {
    let mut findings = Vec::new();
    let trimmed = description.trim();
    let lower = trimmed.to_ascii_lowercase();

    // ── Too short to be actionable (< 12 chars or < 3 words)
    if trimmed.len() < 12 || trimmed.split_whitespace().count() < 3 {
        findings.push("vague_short");
    }

    // ── Destructive operations without safeguard language
    let destructive_patterns = [
        "rm -rf",
        "drop table",
        "drop database",
        "truncate",
        "delete all",
        "wipe",
        "nuke",
        "format disk",
        "purge",
    ];
    let safeguard_patterns = [
        "backup",
        "snapshot",
        "confirm",
        "dry-run",
        "dry_run",
        "safeguard",
        "rollback",
        "revert",
    ];
    let has_destructive = destructive_patterns.iter().any(|p| lower.contains(p));
    let has_safeguard = safeguard_patterns.iter().any(|p| lower.contains(p));
    if has_destructive && !has_safeguard {
        findings.push("risk_destructive");
    }

    // ── No action verb in first 6 words (passive/vague phrasing)
    let action_verbs = [
        "add",
        "create",
        "implement",
        "build",
        "fix",
        "refactor",
        "remove",
        "update",
        "migrate",
        "replace",
        "wire",
        "integrate",
        "test",
        "verify",
        "validate",
        "audit",
        "review",
        "document",
        "deploy",
        "configure",
        "define",
        "extend",
        "extract",
        "move",
        "rename",
        "delete",
        "run",
        "check",
        "ensure",
        "enforce",
        "emit",
        "parse",
        "generate",
        "write",
        "read",
        "load",
        "save",
        "send",
        "receive",
        "handle",
        "process",
        "transform",
        "convert",
        "normalize",
        "compile",
        "link",
        "export",
        "import",
        "inject",
        "wrap",
        "unwrap",
        "merge",
        "split",
        "set",
    ];
    let first_words: Vec<&str> = lower.split_whitespace().take(6).collect();
    let has_verb = first_words
        .iter()
        .any(|w| action_verbs.iter().any(|v| w.starts_with(v)));
    if !has_verb && trimmed.len() >= 12 {
        findings.push("vague_phrase");
    }

    findings
}

fn tasks_with_nonempty_files(tasks: &[PlanAdequacyTask]) -> usize {
    tasks
        .iter()
        .filter(|t| {
            t.files
                .iter()
                .any(|f| !f.is_empty() && !f.eq_ignore_ascii_case("tbd"))
        })
        .count()
}

/// Many tasks share the same opening phrase — common failure mode for “detailed” rewrites.
fn repeated_task_opening(tasks: &[PlanAdequacyTask]) -> bool {
    if tasks.len() < 4 {
        return false;
    }
    let threshold = ((tasks.len() as f32) * 0.4).ceil() as usize;
    let threshold = threshold.max(3);
    let mut prefix_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for t in tasks {
        let p = t
            .description
            .to_ascii_lowercase()
            .split_whitespace()
            .take(4)
            .collect::<Vec<_>>()
            .join(" ");
        if p.len() < 10 {
            continue;
        }
        *prefix_counts.entry(p).or_insert(0) += 1;
    }
    prefix_counts.values().any(|&c| c >= threshold)
}

fn structural_noise_penalties(
    tasks: &[PlanAdequacyTask],
    estimated: u8,
    detail_target: usize,
) -> (Vec<String>, f32) {
    let mut codes = Vec::new();
    let mut mul = 1.0_f32;
    if estimated >= 6 && tasks.len() >= detail_target.max(3) {
        let n = tasks.len().max(1);
        let sum_len: usize = tasks.iter().map(|t| t.description.len()).sum();
        let avg = sum_len / n;
        let linked = tasks_with_nonempty_files(tasks);
        if avg < 28 && linked * 3 < n {
            codes.push("verbose_but_low_surface".to_string());
            mul *= 0.86;
        }
    }
    if repeated_task_opening(tasks) {
        codes.push("repeated_task_phrasing".to_string());
        mul *= 0.88;
    }
    (codes, mul)
}

fn refinement_regression_penalties(
    prev: &[PlanAdequacyTask],
    cur: &[PlanAdequacyTask],
    detail_target: usize,
) -> (Vec<String>, f32) {
    if prev.is_empty() || cur.is_empty() {
        return (Vec::new(), 1.0);
    }
    let mut codes = Vec::new();
    let mut mul = 1.0_f32;
    if cur.len() < prev.len() && prev.len() >= detail_target.saturating_sub(1).max(2) {
        codes.push("possible_rewrite_compression".to_string());
        mul *= 0.87;
    }
    let lp = tasks_with_nonempty_files(prev);
    let lc = tasks_with_nonempty_files(cur);
    if lp > 0 && lc < lp {
        codes.push("file_linkage_reduced".to_string());
        mul *= 0.85;
    }
    let sp: usize = prev.iter().map(|t| t.description.len()).sum();
    let sc: usize = cur.iter().map(|t| t.description.len()).sum();
    if sc + 50 < (sp * 9 / 10).max(1) && cur.len() <= prev.len() {
        codes.push("description_mass_reduced".to_string());
        mul *= 0.9;
    }
    (codes, mul)
}

fn detail_target_min_tasks(complexity: u8, depth_bonus: i8) -> usize {
    let mut t = match complexity {
        0..=3 => 1,
        4 | 5 => 2,
        6..=8 => 4,
        9 => 6,
        _ => 8,
    };
    let b = depth_bonus.clamp(-2, 4);
    t = (t as i32 + b as i32).max(1) as usize;
    t
}

fn goal_has_path_like_token(goal: &str) -> bool {
    let exts = [
        ".rs", ".toml", ".md", ".yaml", ".yml", ".json", ".vox", ".tsx", ".ts", ".js",
    ];
    for w in goal.split_whitespace() {
        let w = w.trim_matches(|c: char| {
            c == '`' || c == '"' || c == '\'' || c == '(' || c == ')' || c == ',' || c == ';'
        });
        let n = w.replace('\\', "/");
        if n.contains('/')
            && (exts.iter().any(|e| n.ends_with(e))
                || n.starts_with("crates/")
                || n.starts_with("src/")
                || n.starts_with("docs/"))
        {
            return true;
        }
    }
    false
}

fn apply_shared_text_gap_codes(
    codes: &mut Vec<String>,
    score: &mut f32,
    task_critical: &mut bool,
    findings: Vec<&'static str>,
) {
    for finding in findings {
        match finding {
            "vague_short" => {
                codes.push("vague_short".to_string());
                *score *= 0.65;
            }
            "vague_phrase" => {
                codes.push("vague_phrase".to_string());
                *score *= 0.75;
            }
            "risk_destructive" => {
                codes.push("risk_destructive".to_string());
                *score *= 0.35;
                *task_critical = true;
            }
            _ => {}
        }
    }
}

fn per_task_findings(tasks: &[PlanAdequacyTask]) -> (Vec<TaskGapFinding>, usize) {
    let id_set: HashSet<usize> = tasks.iter().map(|t| t.id).collect();
    let mut per_task = Vec::with_capacity(tasks.len());
    let mut critical = 0usize;

    for t in tasks {
        let mut codes = Vec::new();
        let mut score: f32 = 1.0;
        let mut task_critical = false;
        let dl = t.description.to_ascii_lowercase();

        apply_shared_text_gap_codes(
            &mut codes,
            &mut score,
            &mut task_critical,
            orchestrator_node_text_findings(&t.description),
        );

        if dl.contains("tbd") || t.files.iter().any(|f| f.eq_ignore_ascii_case("tbd")) {
            codes.push("tbd_placeholder".to_string());
            score *= 0.7;
        }

        // ── Precondition check (Task 3.2.2): state-mutating tasks must have preconditions
        let is_state_mutating = !t.files.is_empty()
            && t.files
                .iter()
                .any(|f| !f.is_empty() && !f.eq_ignore_ascii_case("tbd"))
            && t.estimated_complexity >= 6;
        if is_state_mutating && t.preconditions.is_empty() {
            codes.push("precondition_missing".to_string());
            score *= 0.70;
        }
        if t.test_decision == Some(crate::planning::test_decision::TestDecision::Required) {
            let looks_like_test = dl.contains("test")
                || dl.contains("verify")
                || t.files
                    .iter()
                    .any(|f| f.to_ascii_lowercase().contains("test"));
            if !looks_like_test {
                codes.push("heavy_without_test_hint".to_string());
                score *= 0.65; // stricter penalty for defying explicit test requirements
                task_critical = true;
            }
        }
        for &d in &t.depends_on {
            if !id_set.contains(&d) || d >= t.id {
                codes.push("deps_incomplete_or_order".to_string());
                score *= 0.55;
                break;
            }
        }

        score = score.clamp(0.0, 1.0);
        if task_critical {
            critical += 1;
        }

        per_task.push(TaskGapFinding {
            task_id: t.id,
            reason_codes: codes,
            task_confidence: score,
        });
    }

    (per_task, critical)
}

fn aggregate_risk(per_task: &[TaskGapFinding]) -> f32 {
    let n = per_task.len().max(1) as f32;
    per_task
        .iter()
        .map(|p| 1.0 - p.task_confidence)
        .sum::<f32>()
        / n
}

fn build_suggested_questions(
    tasks: &[PlanAdequacyTask],
    findings: &[TaskGapFinding],
) -> Vec<String> {
    let mut out = Vec::new();
    for (task, gap) in tasks.iter().zip(findings.iter()) {
        if gap.reason_codes.is_empty() {
            continue;
        }
        if gap.reason_codes.iter().any(|c| c == "risk_destructive") {
            out.push(format!(
                "Task {} proposes potentially destructive actions — confirm scope and safeguards?",
                task.id
            ));
        } else if gap
            .reason_codes
            .iter()
            .any(|c| c == "deps_incomplete_or_order")
        {
            out.push(format!(
                "Task {} has dependency issues — which prerequisites must finish first?",
                task.id
            ));
        } else if gap
            .reason_codes
            .iter()
            .any(|c| c == "vague_short" || c == "vague_phrase" || c == "tbd_placeholder")
        {
            out.push(format!(
                "Task {} is underspecified — what files, interfaces, or acceptance criteria apply?",
                task.id
            ));
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

fn compute_adequacy_summary(
    goal: &str,
    tasks: &[PlanAdequacyTask],
    router_complexity_hint: Option<u8>,
    depth_bonus: i8,
    aggregate_unresolved_risk: f32,
    fatigued: bool,
) -> PlanAdequacySummary {
    let mut estimated = effective_goal_complexity(goal, router_complexity_hint);
    if fatigued {
        // Escalate complexity expectation when human is burnt out (forcing more precise plan decomposition).
        estimated = estimated.max(8);
    }
    let mut detail_target = detail_target_min_tasks(estimated, depth_bonus);
    if fatigued {
        detail_target = (detail_target * 2).max(4); // Force much higher task decomposition when fatigued.
    }

    let mut thin_codes = Vec::new();
    let mut score: f32 = 1.0;

    if estimated >= 5 && tasks.len() < detail_target {
        thin_codes.push("too_few_tasks".to_string());
        let gap = (detail_target - tasks.len()) as f32 / detail_target as f32;
        score *= (1.0 - gap * 0.45).clamp(0.35, 1.0);
    }

    let mega = tasks.iter().filter(|t| t.estimated_complexity >= 9).count();
    if mega >= 2 && tasks.len() < detail_target + 2 {
        thin_codes.push("mega_task_cluster".to_string());
        score *= 0.78;
    }

    // Test verification is now evaluated per-task above. We no longer use holistic string math
    // on the goal description to force verification hints into the plan structure.

    if tasks.len() >= 4 && tasks.iter().all(|t| t.depends_on.is_empty()) {
        thin_codes.push("flat_dependency_graph".to_string());
        score *= 0.85;
    }

    if goal_has_path_like_token(goal)
        && tasks.iter().all(|t| {
            t.files.is_empty()
                || t.files
                    .iter()
                    .all(|f| f.is_empty() || f.eq_ignore_ascii_case("tbd"))
        })
    {
        thin_codes.push("goal_paths_unlinked".to_string());
        score *= 0.8;
    }

    score = score.clamp(0.0, 1.0);

    // Thin if structural signals fire or score is low or overall gap risk is high
    let is_too_thin =
        !thin_codes.is_empty() && score < 0.82 || score < 0.58 || aggregate_unresolved_risk > 0.32;

    PlanAdequacySummary {
        score,
        is_too_thin,
        reason_codes: thin_codes,
        detail_target_min_tasks: detail_target,
        estimated_goal_complexity: estimated,
        rubric_scores: None, // Populated by callers with LLM access
    }
}

fn adequacy_thinness_gate(
    reason_codes: &[String],
    score: f32,
    aggregate_unresolved_risk: f32,
) -> bool {
    !reason_codes.is_empty() && score < 0.82 || score < 0.58 || aggregate_unresolved_risk > 0.32
}

/// Full report: per-task gaps + plan adequacy. `scope_file_count` nudges complexity when the user
/// pinned files (optional).
///
/// `prior_tasks_after_refine`: when set, compares the plan **before** a refinement LLM pass to
/// `tasks` (**after**) to flag rewrite compression and lost file linkage.
pub fn analyze_plan_refinement_report(
    goal: &str,
    scope_file_count: usize,
    router_complexity_hint: Option<u8>,
    plan_depth_bonus: i8,
    tasks: &[PlanAdequacyTask],
    fatigued: bool,
) -> PlanRefinementReport {
    analyze_plan_refinement_report_with_prior(
        goal,
        scope_file_count,
        router_complexity_hint,
        plan_depth_bonus,
        tasks,
        None,
        fatigued,
    )
}

pub fn analyze_plan_refinement_report_with_prior(
    goal: &str,
    scope_file_count: usize,
    router_complexity_hint: Option<u8>,
    plan_depth_bonus: i8,
    tasks: &[PlanAdequacyTask],
    prior_tasks_after_refine: Option<&[PlanAdequacyTask]>,
    fatigued: bool,
) -> PlanRefinementReport {
    let mut router = router_complexity_hint;
    if scope_file_count >= 4 {
        router = Some(router.unwrap_or(5).max(7));
    } else if scope_file_count >= 1 {
        router = Some(router.unwrap_or(5).max(6));
    }

    let (per_task, critical_count) = per_task_findings(tasks);
    let aggregate_unresolved_risk = aggregate_risk(&per_task);
    let suggested = build_suggested_questions(tasks, &per_task);

    let mut adequacy = compute_adequacy_summary(
        goal,
        tasks,
        router,
        plan_depth_bonus,
        aggregate_unresolved_risk,
        fatigued,
    );

    let (sn_codes, sn_mul) = structural_noise_penalties(
        tasks,
        adequacy.estimated_goal_complexity,
        adequacy.detail_target_min_tasks,
    );
    adequacy.reason_codes.extend(sn_codes);
    adequacy.score = (adequacy.score * sn_mul).clamp(0.0, 1.0);

    if let Some(prior) = prior_tasks_after_refine {
        let (rg_codes, rg_mul) =
            refinement_regression_penalties(prior, tasks, adequacy.detail_target_min_tasks);
        adequacy.reason_codes.extend(rg_codes);
        adequacy.score = (adequacy.score * rg_mul).clamp(0.0, 1.0);
    }

    adequacy.is_too_thin = adequacy_thinness_gate(
        &adequacy.reason_codes,
        adequacy.score,
        aggregate_unresolved_risk,
    );

    PlanRefinementReport {
        per_task,
        aggregate_unresolved_risk,
        critical_count,
        suggested_clarifying_questions: suggested,
        adequacy,
    }
}

/// Map orchestrator [`super::PlanNode`]s to adequacy tasks. Complexity is inferred from description
/// length when not present on the node type.
pub fn plan_nodes_to_adequacy_tasks(nodes: &[super::PlanNode]) -> Vec<PlanAdequacyTask> {
    let id_by_node: HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.node_id.clone(), i + 1))
        .collect();

    nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let deps: Vec<usize> = n
                .depends_on
                .iter()
                .filter_map(|d| id_by_node.get(d).copied())
                .collect();
            let files: Vec<String> = n
                .execution_policy
                .file_manifest
                .iter()
                .map(|f| f.path.to_string_lossy().into_owned())
                .collect();
            let inferred_cx = (crate::compaction::CompactionEngine::estimate_tokens(&n.description)
                / 10)
                .clamp(1, 10) as u8;
            PlanAdequacyTask {
                id: i + 1,
                description: n.description.clone(),
                files,
                estimated_complexity: inferred_cx.max(3),
                depends_on: deps,
                test_decision: None,
                preconditions: vec![],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_task(id: usize, desc: &str, cx: u8) -> PlanAdequacyTask {
        PlanAdequacyTask {
            id,
            description: desc.to_string(),
            files: vec![],
            estimated_complexity: cx,
            depends_on: vec![],
            test_decision: None,
            preconditions: vec![],
        }
    }

    #[test]
    fn flags_too_few_tasks_for_complex_goal() {
        let goal = "migrate authentication across crates/vox-auth, crates/vox-mcp, and update docs; add regression tests";
        let tasks = vec![sample_task(1, "do the migration work", 8)];
        let r = analyze_plan_refinement_report(goal, 0, None, 0, &tasks, false);
        assert!(r.adequacy.is_too_thin);
        assert!(r.adequacy.reason_codes.iter().any(|c| c == "too_few_tasks"));
    }

    #[test]
    fn per_task_destructive_still_critical() {
        let goal = "small cleanup";
        let tasks = vec![sample_task(1, "rm -rf /unused", 3)];
        let r = analyze_plan_refinement_report(goal, 0, None, 0, &tasks, false);
        assert!(r.critical_count >= 1);
        assert!(r.aggregate_unresolved_risk > 0.2);
    }

    #[test]
    fn prior_snapshot_flags_task_compression() {
        let goal = "migrate authentication across crates/vox-auth and crates/vox-mcp with regression tests";
        let prior = vec![
            sample_task(1, "audit current auth flows in crates/vox-auth", 5),
            sample_task(2, "port token handling in crates/vox-mcp", 6),
            sample_task(3, "add integration tests for auth edge cases", 5),
            sample_task(4, "update docs for new auth contract", 4),
        ];
        let shrunk = vec![
            sample_task(1, "audit current auth flows in crates/vox-auth", 5),
            sample_task(2, "finish migration and testing", 8),
        ];
        let r = analyze_plan_refinement_report_with_prior(
            goal,
            0,
            None,
            0,
            &shrunk,
            Some(&prior),
            false,
        );
        assert!(
            r.adequacy
                .reason_codes
                .iter()
                .any(|c| c == "possible_rewrite_compression"),
            "{:?}",
            r.adequacy.reason_codes
        );
    }

    #[test]
    fn precondition_missing_penalises_complex_mutating_task() {
        let goal = "migrate database schema across all services";
        let mut task = sample_task(
            1,
            "Update crates/vox-db schema and run migration scripts",
            7,
        );
        task.files = vec!["crates/vox-db/src/schema.rs".to_string()];
        // No preconditions set — should be flagged
        let r = analyze_plan_refinement_report(goal, 0, None, 0, &[task], false);
        assert!(
            r.per_task[0]
                .reason_codes
                .iter()
                .any(|c| c == "precondition_missing"),
            "complex mutating task without preconditions should be flagged: {:?}",
            r.per_task[0].reason_codes
        );
    }

    #[test]
    fn precondition_present_not_penalised() {
        let goal = "migrate database schema";
        let mut task = sample_task(
            1,
            "Update crates/vox-db schema and run migration scripts",
            7,
        );
        task.files = vec!["crates/vox-db/src/schema.rs".to_string()];
        task.preconditions = vec!["database_backup_complete == true".to_string()];
        let r = analyze_plan_refinement_report(goal, 0, None, 0, &[task], false);
        assert!(
            !r.per_task[0]
                .reason_codes
                .iter()
                .any(|c| c == "precondition_missing"),
            "task with preconditions should not be flagged"
        );
    }

    #[test]
    fn rubric_scores_weighted_aggregate() {
        let scores = RubricScores {
            coverage: 10,
            dependency_ordering: 10,
            destructive_safety: 10,
            concreteness: 10,
            verification: 10,
        };
        assert!((scores.weighted_score() - 1.0).abs() < 0.01);

        let low = RubricScores {
            coverage: 0,
            dependency_ordering: 0,
            destructive_safety: 0,
            concreteness: 0,
            verification: 0,
        };
        assert!((low.weighted_score() - 0.0).abs() < 0.01);
    }

    #[test]
    fn text_findings_detect_destructive_without_safeguard() {
        let findings = orchestrator_node_text_findings("rm -rf /unused directory from disk");
        assert!(findings.contains(&"risk_destructive"));
    }

    #[test]
    fn text_findings_allow_destructive_with_safeguard() {
        let findings = orchestrator_node_text_findings(
            "rm -rf /unused directory after backup and dry-run verification",
        );
        assert!(!findings.contains(&"risk_destructive"));
    }

    #[test]
    fn text_findings_detect_vague_short() {
        let findings = orchestrator_node_text_findings("do stuff");
        assert!(findings.contains(&"vague_short"));
    }

    #[test]
    fn text_findings_detect_vague_phrase_no_verb() {
        let findings =
            orchestrator_node_text_findings("the database schema changes for the new version");
        assert!(findings.contains(&"vague_phrase"));
    }

    #[test]
    fn repeated_openings_reduce_adequacy_score() {
        let goal = "implement several related parser fixes across the compiler frontend";
        let open =
            "Add detailed error recovery for unexpected tokens in the grammar pipeline with tests";
        let tasks: Vec<PlanAdequacyTask> =
            (1..=5usize).map(|id| sample_task(id, open, 5)).collect();
        let r = analyze_plan_refinement_report(goal, 0, None, 0, &tasks, false);
        assert!(
            r.adequacy
                .reason_codes
                .iter()
                .any(|c| c == "repeated_task_phrasing"),
            "{:?}",
            r.adequacy.reason_codes
        );
    }
}
