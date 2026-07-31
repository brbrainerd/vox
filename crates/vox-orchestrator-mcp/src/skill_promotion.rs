//! Task 3.3 (harness parity plan): promotion pipeline that turns a
//! `skill_candidates` row (`status = 'pending'`, written by the miners in
//! `vox-skill-discovery` via `vox skill suggest`, see Task 3.2) into a real,
//! usable skill.
//!
//! ## What this module deliberately is and is not
//!
//! This is a scoped-down **vertical slice**, not full-fidelity implementation
//! of all 8 gates described in the harness parity plan. Each gate function
//! below documents its own fidelity; three are explicitly reduced-fidelity
//! starting points with a documented gap, not fake "always pass" stubs:
//!
//! * **Gate 1 (execution)** — `MinedOp` (`vox_skill_discovery::op_miner`),
//!   the only miner wired to `skill_candidates` today, records tool-call
//!   sequences with **no success/failure field at all**. So "did this
//!   actually run successfully" cannot be checked from `raw_json` as it
//!   exists today — this is a real gap, not something satisfied by
//!   construction. [`gate_execution`] instead does the check that *is*
//!   possible: the raw trajectory must deserialize to a well-formed,
//!   non-empty `vox_skill_discovery::Candidate`. Tracking real
//!   success/failure needs a schema change to `MinedOp`/`agent_operations`
//!   (`is_error` already exists on `agent_operations` — a follow-up could
//!   thread it through the miner) — flagged as a follow-up task, not solved
//!   here.
//! * **Gate 3 (dedupe)** — uses `vox-similarity`'s real simhash/hamming API
//!   (this crate *does* exist with a usable dedup primitive, confirming the
//!   plan's design target), comparing the candidate's name+description
//!   shingle simhash against existing skills' simhash. This is the
//!   documented-as-simple form: whole-value simhash + a hamming threshold,
//!   not `vox-similarity`'s fuller LSH/cluster index — good enough to catch
//!   near-identical candidates, not a semantic dedup.
//! * **Gate 5 (generality)** — counts rows sharing `candidate_name` via the
//!   new `count_skill_candidates_by_name` op; "same underlying skill" is
//!   approximated by exact name match (the miner already derives
//!   `candidate_name` deterministically from the mined pattern, so repeat
//!   mining runs naturally re-propose the same name for the same recurring
//!   pattern). A follow-up could cluster by trajectory similarity instead of
//!   exact name for a looser notion of "same skill".
//! * **Gate 6 (no known counter-examples)** — nothing in this codebase
//!   currently records *failures* of an installed/attempted skill tied back
//!   to a `skill_candidates` row (no `skill_failures` table, no negative
//!   signal in `reliability_scores` scoped to candidates pre-promotion).
//!   [`gate_no_known_counterexamples`] is therefore an honest pass-through
//!   documented as vacuously true today — flagged as a follow-up, not
//!   fabricated.
//!
//! Gates 2, 4, 7, 8 are implemented at real fidelity:
//!
//! * **Gate 2 (abstraction)** — [`abstract_candidate`] turns the raw
//!   `vox_skill_discovery::Candidate` into a [`CandidateSkillDraft`] (name,
//!   description, source body) sized to what mined data can actually
//!   support today. It is deliberately *not* a full
//!   `vox_plugin_types::skill_manifest::SkillManifest` — a real manifest
//!   requires `category`/`permissions` that mined trajectories don't carry
//!   (permissions in particular must never be inferred; they need an
//!   explicit human or policy decision). Constructing the installable
//!   manifest from a confirmed draft is left as a follow-up once a
//!   permission-assignment story exists.
//! * **Gate 4 (independent verification)** — [`gate_independent_verification`]
//!   is generic over an async judge closure so it is unit-testable without a
//!   network call; [`llm_chat_judge`] is the real production judge wired to
//!   `vox_actor_runtime::llm::llm_chat`, gated by the same
//!   `VOX_INFERENCE_PRIVACY` hard filter used elsewhere (Task 2.2) via
//!   `vox_orchestrator::route_policy::is_local_http_provider`. Returns a
//!   3-way [`VerificationOutcome`] (`Approved`/`Rejected`/
//!   `VerificationError`), not a plain pass/fail — a judge *rejection* is a
//!   permanent verdict, while a failed verification *call* (timeout,
//!   unreachable model, privacy denial) is transient and should leave the
//!   candidate `pending` for retry rather than kill it. Known limitation:
//!   `skill_candidates` does not record which model produced the
//!   mined trajectory (the miners are static analyzers, not model calls —
//!   there is no "generating model" to diff against), so "verification by a
//!   *different* model" reduces to "verification by *a* model the operator
//!   configured as a judge, respecting the privacy hard filter" rather than
//!   an enforced difference from an originating model. Documented, not
//!   silently reinterpreted.
//! * **Gate 7 (shadow-period state machine)** — [`Lifecycle`] mirrors
//!   `vox_orchestrator::models::autonomic::ModelConfidence` exactly in shape
//!   (Provisional/Confirmed/Deprecated; there is no `Shadowed` distinct
//!   state here because "shadow period" for a skill *is* the Provisional
//!   state — it is presented for use but not yet promotion-eligible).
//! * **Gate 8 (provenance binding)** — [`gate_provenance`] hashes `raw_json`
//!   with the same `blake3` dependency `vox-similarity` already uses, and
//!   compares against the `source_hash` bound at first promotion; a mismatch
//!   forces `Lifecycle::Provisional` (re-verification) instead of silently
//!   overwriting a `Confirmed` skill.

use vox_db::SkillCandidateRow;
use vox_similarity::{hamming, shingle, simhash64};

/// Mirrors `vox_orchestrator::models::autonomic::ModelConfidence`'s shape
/// (Task 3.3 gate 7) for a promoted skill's shadow-period lifecycle. Stored
/// in `skill_candidates.lifecycle_state` as the lowercase variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    /// Passed gates 1-6 and was abstracted into a draft, but has not yet
    /// accumulated enough real-world shadow-period evidence to route
    /// production use to it.
    Provisional,
    /// Cleared the shadow period; eligible for real use.
    Confirmed,
    /// Superseded, revoked, or failed a provenance re-check.
    Deprecated,
}

impl Lifecycle {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisional => "provisional",
            Self::Confirmed => "confirmed",
            Self::Deprecated => "deprecated",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "confirmed" => Self::Confirmed,
            "deprecated" => Self::Deprecated,
            _ => Self::Provisional,
        }
    }

    /// Mirrors `ModelConfidence::eligible_for_routing`: only `Confirmed`
    /// skills should be handed to real use (e.g. surfaced by the skill
    /// registry / MCP tool listing), matching the parity plan's "provisional
    /// -> confirmed -> deprecated" target exactly.
    #[must_use]
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Confirmed)
    }
}

/// Gate 2 output: what a `skill_candidates` row is abstracted into. Narrower
/// than a full `SkillManifest` — see module docs for why permissions and
/// category are deliberately absent here.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateSkillDraft {
    pub name: String,
    pub description: String,
    /// Free-text summary of the mined trajectory members, used as the
    /// dedup/verification prompt body — not a runnable skill body.
    pub source_body: String,
}

/// A gate's outcome: pass/fail plus a human-readable reason, so a caller
/// updating `skill_candidates.status`/`lifecycle_state` can record *why*.
#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    pub passed: bool,
    pub reason: String,
}

impl GateResult {
    fn pass(reason: impl Into<String>) -> Self {
        Self { passed: true, reason: reason.into() }
    }
    fn fail(reason: impl Into<String>) -> Self {
        Self { passed: false, reason: reason.into() }
    }
}

/// Gate 1: structural-validity check on the raw mined trajectory. See module
/// docs — "ran successfully" is not checkable against today's `MinedOp`
/// shape, so this checks the weaker (but real) property that the row holds a
/// well-formed, non-trivial `vox_skill_discovery::Candidate`.
pub fn gate_execution(row: &SkillCandidateRow) -> GateResult {
    let parsed: Result<vox_skill_discovery::Candidate, _> = serde_json::from_str(&row.raw_json);
    match parsed {
        Ok(c) if c.members.is_empty() => {
            GateResult::fail("candidate has no member provenance refs (empty trajectory)")
        }
        Ok(_) => GateResult::pass("raw_json deserializes to a non-empty Candidate"),
        Err(e) => GateResult::fail(format!("raw_json is not a valid Candidate: {e}")),
    }
}

/// Gate 2: abstract the raw trajectory into a draft skill definition.
/// Returns `None` if `raw_json` doesn't parse (caller should have already
/// rejected via `gate_execution`, but this stays defensive).
pub fn abstract_candidate(row: &SkillCandidateRow) -> Option<CandidateSkillDraft> {
    let candidate: vox_skill_discovery::Candidate = serde_json::from_str(&row.raw_json).ok()?;
    let description = candidate
        .draft_frontmatter
        .as_ref()
        .map(|d| d.description.clone())
        .unwrap_or_else(|| candidate.suggested_action.clone());
    Some(CandidateSkillDraft {
        name: row.candidate_name.clone(),
        description,
        source_body: candidate.members.join("\n"),
    })
}

/// Gate 3: reject near-duplicates of already-installed skills. `existing`
/// is `(skill_id, description)` pairs — deliberately the minimal shape
/// needed rather than requiring a full `SkillManifest`/`RegisteredSkill` so
/// this stays unit-testable without constructing the whole skill registry.
///
/// `max_hamming_distance` of 0 means "only exact simhash matches"; higher
/// values widen the near-duplicate net. Simhash64 has 64 bits, so a
/// reasonable starting threshold is small (e.g. 3-6); left as a caller
/// parameter rather than hardcoded since it trades recall for precision.
pub fn gate_dedupe(
    draft: &CandidateSkillDraft,
    existing: &[(String, String)],
    max_hamming_distance: u32,
) -> GateResult {
    // Compared against `existing`'s description alone (no id), so the
    // candidate side uses description alone too — otherwise the candidate's
    // own name token would dilute the shingle overlap and mask a real
    // near-duplicate description.
    let candidate_hash = simhash64(&shingle(&draft.description, 3));
    for (existing_id, existing_desc) in existing {
        let existing_hash = simhash64(&shingle(existing_desc, 3));
        let dist = hamming(candidate_hash, existing_hash);
        if dist <= max_hamming_distance {
            return GateResult::fail(format!(
                "near-duplicate of existing skill '{existing_id}' (hamming distance {dist})"
            ));
        }
    }
    GateResult::pass("no near-duplicate found among existing skills")
}

/// Gate 5: generality — require at least `min_trajectories` observed rows
/// (any status) sharing `candidate_name`, i.e. the pattern was independently
/// mined more than once rather than promoted off a single occurrence.
pub fn gate_generality(observed_count: i64, min_trajectories: i64) -> GateResult {
    if observed_count >= min_trajectories {
        GateResult::pass(format!(
            "{observed_count} observed trajectories >= required {min_trajectories}"
        ))
    } else {
        GateResult::fail(format!(
            "only {observed_count} observed trajectories, need >= {min_trajectories}"
        ))
    }
}

/// Gate 6: no known counter-examples. See module docs — there is currently
/// no data source tracking failures tied to a `skill_candidates` row, so
/// this is an honest, documented pass-through (vacuously true) rather than a
/// fabricated signal. Kept as a real function (not inlined `true`) so a
/// future failure-tracking table has one call site to wire into.
pub fn gate_no_known_counterexamples(_row: &SkillCandidateRow) -> GateResult {
    GateResult::pass(
        "no failure-tracking data source exists yet for skill_candidates (documented gap, Task 3.3)",
    )
}

/// Outcome of gate 4's independent verification. Distinct from [`GateResult`]
/// on purpose: a plain pass/fail bool would conflate "the judge looked at
/// this and rejected it" (a real, permanent verdict — the candidate should
/// move to `rejected`) with "the verification call itself failed" (a
/// transient infra problem — timeout, unreachable model, privacy filter
/// denial — that says nothing about the candidate's merit and should leave
/// it `pending` for retry, not kill it). Collapsing these into one
/// `!passed` would make a future dispatcher permanently reject good
/// candidates whenever the judge model was briefly down.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationOutcome {
    /// The judge model reviewed the candidate and approved it.
    Approved { reason: String },
    /// The judge model reviewed the candidate and rejected it. Permanent —
    /// a caller should treat this like any other failed gate.
    Rejected { reason: String },
    /// The verification call itself did not complete (transport error,
    /// timeout, privacy filter denial, malformed response, etc). No verdict
    /// was reached; a caller should leave the candidate `pending` and retry
    /// rather than reject it.
    VerificationError { reason: String },
}

impl VerificationOutcome {
    /// True only for [`Self::Approved`] — mirrors `GateResult::passed` for
    /// callers that just want "did this gate clear".
    #[must_use]
    pub fn passed(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }

    /// True for [`Self::VerificationError`] — signals "retry me", as opposed
    /// to a permanent judge rejection.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::VerificationError { .. })
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            Self::Approved { reason } | Self::Rejected { reason } | Self::VerificationError { reason } => reason,
        }
    }
}

/// Gate 4: independent verification by a judge. Generic over an async
/// closure so this is unit-testable without a network call; see
/// [`llm_chat_judge`] for the production wiring. `judge` returns `Ok(true)`
/// for approval, `Ok(false)` for an explicit rejection verdict, and `Err` for
/// a failed verification *call* (no verdict reached) — see
/// [`VerificationOutcome`] for why these three are kept distinct.
pub async fn gate_independent_verification<F, Fut>(
    draft: &CandidateSkillDraft,
    judge: F,
) -> VerificationOutcome
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    let prompt = build_verification_prompt(draft);
    match judge(prompt).await {
        Ok(true) => VerificationOutcome::Approved {
            reason: "judge model approved the candidate as a genuine, generalizable, safe skill"
                .to_string(),
        },
        Ok(false) => VerificationOutcome::Rejected {
            reason: "judge model rejected the candidate".to_string(),
        },
        Err(e) => VerificationOutcome::VerificationError {
            reason: format!("verification call failed: {e}"),
        },
    }
}

/// Prompt sent to the judge model for gate 4. Kept as a standalone function
/// so tests can assert on prompt content without invoking an LLM.
#[must_use]
pub fn build_verification_prompt(draft: &CandidateSkillDraft) -> String {
    format!(
        "You are reviewing a candidate skill mined from real agent tool-call \
         history before it is promoted to production use. Judge whether it is \
         a genuine, generalizable, and safe skill (not a one-off hack, not \
         something that encodes secrets/credentials, not overly narrow to one \
         session).\n\n\
         Name: {}\n\
         Description: {}\n\
         Mined trajectory members:\n{}\n\n\
         Answer with exactly one word: PASS or FAIL.",
        draft.name, draft.description, draft.source_body
    )
}

/// Production judge for gate 4: calls `vox_actor_runtime::llm::llm_chat`
/// with `config`, gated by the same `VOX_INFERENCE_PRIVACY=local_only` hard
/// filter as inference routing elsewhere (Task 2.2) — `is_local_provider`
/// should be `vox_orchestrator::route_policy::is_local_http_provider(&model.provider_type)`
/// for the model backing `config`. Callers MUST perform that check before
/// calling this (kept as a plain bool parameter rather than importing
/// `ModelSpec` here to avoid this module depending on model-selection
/// internals it doesn't otherwise need).
pub async fn llm_chat_judge(
    prompt: String,
    config: vox_actor_runtime::llm::LlmConfig,
    is_local_provider: bool,
    privacy_local_only: bool,
) -> Result<bool, String> {
    if privacy_local_only && !is_local_provider {
        return Err(
            "VOX_INFERENCE_PRIVACY=local_only forbids a cloud judge model for gate 4".to_string(),
        );
    }
    let messages = vec![vox_actor_runtime::llm::LlmChatMessage {
        role: "user".to_string(),
        content: prompt,
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];
    let options = vox_actor_runtime::ActivityOptions::default();
    let activity_result = vox_actor_runtime::llm::llm_chat(&options, messages, config).await;
    let outcome = match activity_result {
        vox_actor_runtime::ActivityResult::Ok(Ok(response)) => response,
        vox_actor_runtime::ActivityResult::Ok(Err(provider_err)) => {
            return Err(format!("llm_chat provider error: {provider_err}"));
        }
        vox_actor_runtime::ActivityResult::Failed(e) => {
            return Err(format!("llm_chat activity error: {e}"));
        }
        vox_actor_runtime::ActivityResult::Cancelled => {
            return Err("llm_chat activity cancelled".to_string());
        }
    };
    let normalized = outcome.content.trim().to_ascii_uppercase();
    Ok(normalized.starts_with("PASS"))
}

/// Gate 8: provenance binding + re-verify-on-change. Hashes `raw_json` with
/// blake3 (matching `vox-similarity`'s hash choice) and compares against
/// `bound_hash` (the `source_hash` recorded at last promotion/confirmation).
/// `None` bound hash means "never bound" — treated as a mismatch so the
/// first promotion always binds explicitly rather than trusting an absent
/// value.
#[must_use]
pub fn hash_trajectory(raw_json: &str) -> String {
    blake3::hash(raw_json.as_bytes()).to_hex().to_string()
}

pub fn gate_provenance(row: &SkillCandidateRow, bound_hash: Option<&str>) -> GateResult {
    let current = hash_trajectory(&row.raw_json);
    match bound_hash {
        Some(bound) if bound == current => {
            GateResult::pass("trajectory hash matches previously bound provenance")
        }
        Some(bound) => GateResult::fail(format!(
            "trajectory hash {current} does not match bound provenance {bound}; forcing re-verification"
        )),
        None => GateResult::fail("no provenance hash bound yet; treat as first-time binding"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::SkillCandidateRow;

    fn row(raw_json: &str) -> SkillCandidateRow {
        SkillCandidateRow {
            id: 1,
            candidate_name: "grep-then-edit".to_string(),
            source: "op_miner".to_string(),
            raw_json: raw_json.to_string(),
            status: "pending".to_string(),
            lifecycle_state: "provisional".to_string(),
            source_hash: None,
            created_at_ms: 0,
        }
    }

    fn valid_candidate_json() -> String {
        r#"{
            "kind": "RepeatedOperations",
            "members": ["session-a:grep->edit", "session-b:grep->edit"],
            "score": 0.9,
            "suggested_action": "Extract a grep-then-edit skill",
            "draft_frontmatter": {
                "name": "grep-then-edit",
                "description": "Search then edit the matched file",
                "category": "editing",
                "tags": []
            }
        }"#
        .to_string()
    }

    // --- gate 1: execution / structural validity ---

    #[test]
    fn gate_execution_passes_well_formed_candidate() {
        let r = row(&valid_candidate_json());
        let result = gate_execution(&r);
        assert!(result.passed, "{}", result.reason);
    }

    #[test]
    fn gate_execution_fails_invalid_json() {
        let r = row("not json");
        let result = gate_execution(&r);
        assert!(!result.passed);
    }

    #[test]
    fn gate_execution_fails_empty_members() {
        let r = row(
            r#"{"kind":"RepeatedOperations","members":[],"score":0.9,"suggested_action":"x","draft_frontmatter":null}"#,
        );
        let result = gate_execution(&r);
        assert!(!result.passed);
        assert!(result.reason.contains("empty"));
    }

    // --- gate 2: abstraction ---

    #[test]
    fn abstract_candidate_uses_draft_frontmatter_description() {
        let r = row(&valid_candidate_json());
        let draft = abstract_candidate(&r).expect("abstracts");
        assert_eq!(draft.name, "grep-then-edit");
        assert_eq!(draft.description, "Search then edit the matched file");
        assert!(draft.source_body.contains("session-a:grep->edit"));
    }

    #[test]
    fn abstract_candidate_falls_back_to_suggested_action_without_frontmatter() {
        let r = row(
            r#"{"kind":"RepeatedOperations","members":["a"],"score":0.5,"suggested_action":"Do the thing","draft_frontmatter":null}"#,
        );
        let draft = abstract_candidate(&r).expect("abstracts");
        assert_eq!(draft.description, "Do the thing");
    }

    #[test]
    fn abstract_candidate_none_for_invalid_json() {
        let r = row("garbage");
        assert!(abstract_candidate(&r).is_none());
    }

    // --- gate 3: dedupe ---

    #[test]
    fn gate_dedupe_rejects_near_identical_description() {
        let draft = CandidateSkillDraft {
            name: "grep-then-edit".to_string(),
            description: "Search the codebase then edit the matched file".to_string(),
            source_body: String::new(),
        };
        let existing = vec![(
            "existing-skill".to_string(),
            "Search the codebase then edit the matched file".to_string(),
        )];
        let result = gate_dedupe(&draft, &existing, 5);
        assert!(!result.passed);
        assert!(result.reason.contains("existing-skill"));
    }

    #[test]
    fn gate_dedupe_passes_unrelated_description() {
        let draft = CandidateSkillDraft {
            name: "grep-then-edit".to_string(),
            description: "Search the codebase then edit the matched file".to_string(),
            source_body: String::new(),
        };
        let existing = vec![(
            "unrelated-skill".to_string(),
            "Compress and upload nightly backups to cold storage".to_string(),
        )];
        let result = gate_dedupe(&draft, &existing, 3);
        assert!(result.passed, "{}", result.reason);
    }

    #[test]
    fn gate_dedupe_passes_with_no_existing_skills() {
        let draft = CandidateSkillDraft {
            name: "n".to_string(),
            description: "d".to_string(),
            source_body: String::new(),
        };
        assert!(gate_dedupe(&draft, &[], 0).passed);
    }

    // --- gate 5: generality ---

    #[test]
    fn gate_generality_passes_at_threshold() {
        assert!(gate_generality(3, 3).passed);
        assert!(gate_generality(4, 3).passed);
    }

    #[test]
    fn gate_generality_fails_below_threshold() {
        assert!(!gate_generality(1, 3).passed);
    }

    // --- gate 6: no known counter-examples (documented gap) ---

    #[test]
    fn gate_no_known_counterexamples_is_documented_passthrough() {
        let r = row(&valid_candidate_json());
        let result = gate_no_known_counterexamples(&r);
        assert!(result.passed);
        assert!(result.reason.contains("documented gap"));
    }

    // --- gate 4: independent verification (fake judge, no network) ---

    #[tokio::test]
    async fn gate_independent_verification_approves_when_judge_approves() {
        let draft = CandidateSkillDraft {
            name: "n".to_string(),
            description: "d".to_string(),
            source_body: "s".to_string(),
        };
        let result = gate_independent_verification(&draft, |prompt| async move {
            assert!(prompt.contains("PASS or FAIL"));
            Ok(true)
        })
        .await;
        assert!(result.passed());
        assert!(!result.is_transient());
        assert!(matches!(result, VerificationOutcome::Approved { .. }));
    }

    /// A judge that reviewed the candidate and explicitly rejected it: a
    /// permanent verdict, distinct from a failed verification call.
    #[tokio::test]
    async fn gate_independent_verification_rejects_when_judge_disapproves() {
        let draft = CandidateSkillDraft {
            name: "n".to_string(),
            description: "d".to_string(),
            source_body: "s".to_string(),
        };
        let result =
            gate_independent_verification(&draft, |_| async move { Ok(false) }).await;
        assert!(!result.passed());
        assert!(!result.is_transient(), "a judge rejection is permanent, not transient");
        assert!(matches!(result, VerificationOutcome::Rejected { .. }));
    }

    /// The verification *call itself* fails (timeout/unreachable/etc) — no
    /// verdict was reached, so this must be distinguishable from a judge
    /// rejection: a future dispatcher should retry, not permanently reject.
    #[tokio::test]
    async fn gate_independent_verification_is_transient_when_call_errors() {
        let draft = CandidateSkillDraft {
            name: "n".to_string(),
            description: "d".to_string(),
            source_body: "s".to_string(),
        };
        let result = gate_independent_verification(&draft, |_| async move {
            Err("timeout".to_string())
        })
        .await;
        assert!(!result.passed());
        assert!(result.is_transient(), "an infra failure must be marked transient");
        assert!(matches!(result, VerificationOutcome::VerificationError { .. }));
        assert!(result.reason().contains("timeout"));
    }

    #[test]
    fn verification_outcome_variants_are_distinguishable_not_collapsed() {
        // Regression guard for the code-review finding: Approved/Rejected/
        // VerificationError must never compare equal to each other even
        // when their `passed()` bit matches, and `is_transient()` must only
        // be true for VerificationError.
        let rejected = VerificationOutcome::Rejected { reason: "r".to_string() };
        let error = VerificationOutcome::VerificationError { reason: "r".to_string() };
        assert_ne!(rejected, error);
        assert!(!rejected.passed() && !rejected.is_transient());
        assert!(!error.passed() && error.is_transient());
    }

    #[tokio::test]
    async fn llm_chat_judge_rejects_cloud_model_under_local_only_privacy() {
        let config = vox_actor_runtime::llm::LlmConfig::openrouter("some/model");
        let result = llm_chat_judge("prompt".to_string(), config, false, true).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("local_only"));
    }

    // --- gate 7: lifecycle ---

    #[test]
    fn lifecycle_roundtrips_through_as_str_and_parse() {
        for l in [Lifecycle::Provisional, Lifecycle::Confirmed, Lifecycle::Deprecated] {
            assert_eq!(Lifecycle::parse(l.as_str()), l);
        }
    }

    #[test]
    fn lifecycle_unknown_string_defaults_to_provisional() {
        assert_eq!(Lifecycle::parse("garbage"), Lifecycle::Provisional);
    }

    #[test]
    fn only_confirmed_is_usable() {
        assert!(!Lifecycle::Provisional.is_usable());
        assert!(Lifecycle::Confirmed.is_usable());
        assert!(!Lifecycle::Deprecated.is_usable());
    }

    // --- gate 8: provenance ---

    #[test]
    fn gate_provenance_passes_on_matching_hash() {
        let r = row(&valid_candidate_json());
        let h = hash_trajectory(&r.raw_json);
        assert!(gate_provenance(&r, Some(&h)).passed);
    }

    #[test]
    fn gate_provenance_fails_on_mismatched_hash() {
        let r = row(&valid_candidate_json());
        let result = gate_provenance(&r, Some("stale-hash"));
        assert!(!result.passed);
        assert!(result.reason.contains("re-verification"));
    }

    #[test]
    fn gate_provenance_fails_when_never_bound() {
        let r = row(&valid_candidate_json());
        let result = gate_provenance(&r, None);
        assert!(!result.passed);
    }

    #[test]
    fn hash_trajectory_is_deterministic_and_sensitive_to_content() {
        let a = hash_trajectory("{}");
        let b = hash_trajectory("{}");
        let c = hash_trajectory("{\"x\":1}");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
