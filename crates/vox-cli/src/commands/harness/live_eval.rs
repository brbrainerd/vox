//! Live-model-calling golden tasks for `vox harness eval --live` (chat harness continuous eval
//! design, 2026-08-02). Separate from `eval.rs`'s hermetic `GoldenTask`/`golden_tasks()` by
//! design — that gate must stay hermetic and CI-safe on every commit; this module is only
//! invoked via the explicit `--live` flag, scheduled nightly (see
//! `.github/workflows/harness-eval-nightly.yml`).

use anyhow::Result;

/// One turn's real, observed outcome — what a [`Checker`] evaluates. Deliberately has no
/// `tool_calls_made`/internal-tool-log field: `chat_message`'s public JSON envelope (Task 5) does
/// not expose one, and adding new envelope plumbing to introspect it is out of the design's
/// scope (spec §6.1) — tool-calling tasks are verified purely by observable end-state
/// (`end_state_check`), which is a more robust check anyway (it doesn't care how the model got
/// there, only whether the real-world effect happened).
pub struct EvalTurnResult {
    pub reply_text: String,
    pub model_id: String,
    pub cost_tier: vox_orchestrator::models::CostTier,
    pub end_state_check: Option<Result<(), String>>,
    pub latency_ms: u64,
    pub cost_usd: f64,
}

/// How a [`LiveEvalTask`] is scored.
pub enum Checker {
    /// A plain Rust predicate against the real observed outcome. No judge model involved.
    Deterministic(fn(&EvalTurnResult) -> Result<(), String>),
    /// An odd-sized ensemble of judge calls (majority vote), each also checked for
    /// style-invariance (does the same verdict hold on a paraphrased/reordered reply) — see
    /// `judge_ensemble_score` below. `rubric` is the grading instruction given to each judge.
    LlmJudgeEnsemble { rubric: &'static str, ensemble_size: usize },
}

/// One live-eval golden task.
pub struct LiveEvalTask {
    pub id: &'static str,
    pub category: &'static str,
    pub prompt: &'static str,
    pub checker: Checker,
}

/// A single judge call's verdict — abstracted so scoring logic can be unit-tested with fixture
/// judges, without a live model call. The real judge implementation (Task 5) wraps a live LLM
/// call producing this type.
pub struct JudgeVerdict {
    pub passed: bool,
}

/// Majority-vote an ensemble of judge verdicts, requiring the SAME verdict on both the original
/// reply and its style-invariance paraphrase for each judge to "count" — a judge that flips its
/// verdict between the two is treated as abstaining (not counted either way), since a swing on
/// style alone is exactly the failure mode this ensemble exists to catch (per the harness-testing
/// research doc's finding that judges can swing up to 98% on stylistic artifacts).
pub fn judge_ensemble_score(
    original_verdicts: &[JudgeVerdict],
    paraphrase_verdicts: &[JudgeVerdict],
) -> Result<(), String> {
    assert_eq!(
        original_verdicts.len(),
        paraphrase_verdicts.len(),
        "judge_ensemble_score requires one paraphrase verdict per original verdict"
    );
    let mut pass_votes = 0usize;
    let mut fail_votes = 0usize;
    for (orig, para) in original_verdicts.iter().zip(paraphrase_verdicts.iter()) {
        if orig.passed == para.passed {
            if orig.passed {
                pass_votes += 1;
            } else {
                fail_votes += 1;
            }
        }
        // else: this judge abstains (style-swing detected), counted toward neither total.
    }
    if pass_votes > fail_votes {
        Ok(())
    } else {
        Err(format!(
            "judge ensemble did not reach majority pass: {pass_votes} pass vs {fail_votes} fail \
             (of {} judges, {} abstained on a style-invariance mismatch)",
            original_verdicts.len(),
            original_verdicts.len() - pass_votes - fail_votes
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_ensemble_majority_pass_when_all_agree() {
        let orig = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
        ];
        let para = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
        ];
        assert!(judge_ensemble_score(&orig, &para).is_ok());
    }

    #[test]
    fn judge_ensemble_majority_fail_when_all_agree_fail() {
        let orig = vec![JudgeVerdict { passed: false }, JudgeVerdict { passed: false }];
        let para = vec![JudgeVerdict { passed: false }, JudgeVerdict { passed: false }];
        assert!(judge_ensemble_score(&orig, &para).is_err());
    }

    #[test]
    fn judge_that_swings_on_paraphrase_abstains_rather_than_counting() {
        // Judge 1: agrees pass on both -> counts as a pass vote.
        // Judge 2: says pass on original, fail on paraphrase -> abstains (style swing).
        // Judge 3: agrees fail on both -> counts as a fail vote.
        // Net: 1 pass vote vs 1 fail vote -> not a majority pass -> Err.
        let orig = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: false },
        ];
        let para = vec![
            JudgeVerdict { passed: true },
            JudgeVerdict { passed: false },
            JudgeVerdict { passed: false },
        ];
        let result = judge_ensemble_score(&orig, &para);
        assert!(
            result.is_err(),
            "1 pass vote vs 1 fail vote (1 abstention) must not reach majority pass"
        );
    }

    #[test]
    fn deterministic_checker_runs_against_a_fixture_turn_result() {
        let checker: fn(&EvalTurnResult) -> Result<(), String> = |r| {
            if r.reply_text.contains("4") {
                Ok(())
            } else {
                Err(format!("expected '4' in reply, got {:?}", r.reply_text))
            }
        };
        let turn = EvalTurnResult {
            reply_text: "The answer is 4.".to_string(),
            model_id: "test/model".to_string(),
            cost_tier: vox_orchestrator::models::CostTier::Free,
            end_state_check: None,
            latency_ms: 100,
            cost_usd: 0.0001,
        };
        assert!(checker(&turn).is_ok());
    }
}
