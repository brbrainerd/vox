//! Centralized validation and merge policy for [`crate::ContextEnvelope`] at orchestration ingress.
//!
//! Driven by [`crate::OrchestratorConfig::context_lifecycle_shadow`] and
//! [`crate::OrchestratorConfig::context_lifecycle_enforce`]. When both are `false`, policy is a
//! no-op and legacy serde-only parsing remains the contract boundary.
//!
//! With `context_lifecycle_shadow` enabled, successful validation logs `event=context.capture` and
//! session merge resolutions log `event=context.select` (tracing target
//! `vox_orchestrator::context_lifecycle`).
//!
//! **Contract:** JSON telemetry shapes for those events are validated by
//! `contracts/orchestration/context-lifecycle-telemetry.schema.json` (fixtures in
//! `contracts/orchestration/context-lifecycle-telemetry.fixtures.json`; see crate test
//! `context_lifecycle_telemetry_fixtures_validate_against_schema`).

use std::collections::{HashMap, HashSet};

use crate::OrchestratorConfig;
use crate::context_envelope::{
    ContextEnvelope, ContextFact, ContextMergeStrategy, ContextProvenance,
};

fn harness_id_from_context(envelope: &ContextEnvelope) -> Option<&str> {
    let payload = envelope.content.structured_payload.as_ref()?;
    let id = payload
        .get("harness_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some(id)
}

fn trace_context_select_shadow(
    enabled: bool,
    strategy: &ContextMergeStrategy,
    outcome: &'static str,
    incoming: &ContextEnvelope,
    prior_id: Option<&str>,
) {
    if !enabled {
        return;
    }
    tracing::info!(
        target: "vox_orchestrator::context_lifecycle",
        event = "context.select",
        ?strategy,
        outcome,
        incoming_envelope_id = %incoming.envelope_id,
        prior_envelope_id = prior_id,
        harness_id = harness_id_from_context(incoming),
    );
}

/// Origin plane for telemetry and policy tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextIngestSource {
    McpSubmitTask,
    McpHandoffTool,
    SessionStoreWrite,
    SessionAttach,
    InternalHandoffAccept,
}

/// Repository / session bounds for anti-bleed checks.
#[derive(Debug, Clone, Copy)]
pub struct ContextIngestExpectations<'a> {
    pub repository_id: &'a str,
    pub session_id: Option<&'a str>,
}

/// Structural and policy validation without config branching.
pub(crate) fn validate_context_envelope_ingest(
    envelope: &ContextEnvelope,
    expectations: ContextIngestExpectations<'_>,
    now_ms: u64,
) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    if envelope.schema_version != 1 {
        errors.push(format!(
            "unsupported context schema_version {} (only 1 supported)",
            envelope.schema_version
        ));
    }

    if envelope.envelope_id.trim().is_empty() {
        errors.push("context envelope_id is empty".to_string());
    }

    if envelope.provenance.source_system.trim().is_empty() {
        errors.push("context provenance.source_system is empty".to_string());
    }

    if envelope.subject.repository_id.trim().is_empty() {
        errors.push("context subject.repository_id is empty".to_string());
    } else if envelope.subject.repository_id != expectations.repository_id {
        errors.push(format!(
            "context subject.repository_id {:?} does not match expected {:?}",
            envelope.subject.repository_id, expectations.repository_id
        ));
    }

    if envelope.created_at_unix_ms == 0 {
        errors.push("context created_at_unix_ms is zero".to_string());
    }

    if let Some(exp) = envelope.expires_at_unix_ms {
        if exp < now_ms {
            errors.push(format!(
                "context envelope expired (expires_at_unix_ms={exp} now_ms={now_ms})"
            ));
        }
    }

    if let Some(stale_after) = envelope.conflict_policy.stale_after_ms {
        let cutoff = envelope.created_at_unix_ms.saturating_add(stale_after);
        if cutoff < now_ms {
            errors.push(format!(
                "context envelope stale by conflict_policy.stale_after_ms (cutoff_ms={cutoff} now_ms={now_ms})"
            ));
        }
    }

    if envelope.trust.authority_rank > 10_000 {
        errors.push(format!(
            "context trust.authority_rank {} exceeds sanity bound 10000",
            envelope.trust.authority_rank
        ));
    }

    if let Some(sid) = expectations
        .session_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Some(env_sid) = envelope
            .subject
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if env_sid != sid {
                errors.push(format!(
                    "context subject.session_id {:?} does not match expected session {:?}",
                    env_sid, sid
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// When [`crate::context_envelope::ContextBudget::max_tokens_for_injection`] is set, truncate
/// [`ContextEnvelope::content`] `summary_text` to a conservative UTF-8 byte cap (~4 bytes per token).
/// Idempotent for small summaries. Updates `budget.token_estimate` to the cap when truncation runs.
pub fn clamp_context_envelope_injection_budget(envelope: &mut ContextEnvelope) {
    const BYTES_PER_TOKEN_EST: usize = 4;
    let Some(max_tok) = envelope.budget.max_tokens_for_injection else {
        return;
    };
    let max_bytes = (max_tok as usize).saturating_mul(BYTES_PER_TOKEN_EST);
    if max_bytes < 32 {
        return;
    }
    let summary = envelope.content.summary_text.as_str();
    if summary.len() <= max_bytes {
        return;
    }
    let cut = summary.floor_char_boundary(max_bytes.saturating_sub(64));
    envelope.content.summary_text = format!(
        "{}…\n[vox: summary truncated to max_tokens_for_injection={max_tok}]",
        &summary[..cut]
    );
    envelope.budget.token_estimate = Some(max_tok);
}

/// Apply configured shadow/enforce policy after validation.
pub fn apply_context_lifecycle_policy(
    cfg: &OrchestratorConfig,
    envelope: &ContextEnvelope,
    expectations: ContextIngestExpectations<'_>,
    source: ContextIngestSource,
) -> Result<(), String> {
    if !cfg.context_lifecycle_shadow && !cfg.context_lifecycle_enforce {
        return Ok(());
    }

    let now_ms = crate::types::now_unix_ms();
    let res = validate_context_envelope_ingest(envelope, expectations, now_ms);

    // Add OBO token validation to the error list if enforcement is on
    let mut errs = match res {
        Ok(()) => Vec::new(),
        Err(errs) => errs,
    };

    if cfg.context_lifecycle_enforce {
        let session_key = expectations
            .session_id
            .unwrap_or("anonymous_session")
            .as_bytes();
        if !envelope.verify(session_key) {
            errs.push("obo_token missing or invalid".to_string());
        }
    }

    if errs.is_empty() {
        if cfg.context_lifecycle_shadow {
            tracing::info!(
                target: "vox_orchestrator::context_lifecycle",
                event = "context.capture",
                ?source,
                repository_id = expectations.repository_id,
                session_id = expectations.session_id,
                envelope_id = %envelope.envelope_id,
                envelope_type = ?envelope.envelope_type,
                merge_strategy = ?envelope.conflict_policy.merge_strategy,
                harness_id = harness_id_from_context(envelope),
                trace_id = envelope.provenance.trace_id.as_deref(),
                correlation_id = envelope.provenance.correlation_id.as_deref(),
                "context envelope passed lifecycle validation",
            );
        }
        return Ok(());
    }

    let err_text = errs.join("; ");
    tracing::warn!(
        target: "vox_orchestrator::context_lifecycle",
        ?source,
        repository_id = expectations.repository_id,
        session_id = expectations.session_id,
        "{}",
        err_text
    );

    if cfg.context_lifecycle_enforce {
        // Return exactly what the task requests if obo fails independently
        if err_text.contains("obo_token missing or invalid") {
            return Err("obo_token missing or invalid".to_string());
        }
        return Err(err_text);
    }
    Ok(())
}

/// Merge `incoming` with an optional previously stored JSON envelope for session-scoped keys.
///
/// Returns the envelope that should be persisted. [`ContextMergeStrategy::ManualReview`] returns
/// an error when an existing envelope is present so callers can surface a human gate.
///
/// When `shadow_log_select` is true, emits a structured `context.select` tracing event (target
/// `vox_orchestrator::context_lifecycle`) for merge outcomes — wire
/// [`OrchestratorConfig::context_lifecycle_shadow`].
pub fn merge_context_envelope_for_session_store(
    existing_json: Option<&str>,
    incoming: &ContextEnvelope,
    shadow_log_select: bool,
) -> Result<ContextEnvelope, String> {
    let strategy = &incoming.conflict_policy.merge_strategy;
    let existing: Option<ContextEnvelope> = existing_json
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|raw| serde_json::from_str::<ContextEnvelope>(raw).map_err(|e| e.to_string()))
        .transpose()?;

    let Some(prev) = existing else {
        trace_context_select_shadow(shadow_log_select, strategy, "initial_store", incoming, None);
        return Ok(incoming.clone());
    };
    let prev_id = prev.envelope_id.as_str();

    match strategy {
        ContextMergeStrategy::AppendOnly => {
            trace_context_select_shadow(
                shadow_log_select,
                strategy,
                "kept_previous_append_only",
                incoming,
                Some(prev_id),
            );
            Ok(prev)
        }
        ContextMergeStrategy::LastWriteWins => {
            trace_context_select_shadow(
                shadow_log_select,
                strategy,
                "took_incoming_last_write",
                incoming,
                Some(prev_id),
            );
            Ok(incoming.clone())
        }
        ContextMergeStrategy::AuthorityPrecedence => {
            if incoming.trust.authority_rank >= prev.trust.authority_rank {
                trace_context_select_shadow(
                    shadow_log_select,
                    strategy,
                    "took_incoming_authority",
                    incoming,
                    Some(prev_id),
                );
                Ok(incoming.clone())
            } else {
                trace_context_select_shadow(
                    shadow_log_select,
                    strategy,
                    "kept_previous_authority",
                    incoming,
                    Some(prev_id),
                );
                Ok(prev)
            }
        }
        ContextMergeStrategy::ConfidenceWeighted => {
            let inc = incoming.trust.confidence.unwrap_or(0.0);
            let p = prev.trust.confidence.unwrap_or(0.0);
            if inc >= p {
                trace_context_select_shadow(
                    shadow_log_select,
                    strategy,
                    "took_incoming_confidence",
                    incoming,
                    Some(prev_id),
                );
                Ok(incoming.clone())
            } else {
                trace_context_select_shadow(
                    shadow_log_select,
                    strategy,
                    "kept_previous_confidence",
                    incoming,
                    Some(prev_id),
                );
                Ok(prev)
            }
        }
        ContextMergeStrategy::CrdtMerge => {
            trace_context_select_shadow(
                shadow_log_select,
                strategy,
                "crdt_merged",
                incoming,
                Some(prev_id),
            );
            Ok(merge_envelopes_crdt(&prev, incoming))
        }
        ContextMergeStrategy::ManualReview => Err(
            "context merge_strategy ManualReview requires explicit human review when a session envelope already exists"
                .to_string(),
        ),
    }
}

fn merge_envelopes_crdt(prev: &ContextEnvelope, incoming: &ContextEnvelope) -> ContextEnvelope {
    let mut out = incoming.clone();
    let now_ms = crate::types::now_unix_ms();
    out.envelope_id = format!("ctx-crdt-merge-{now_ms}");
    out.created_at_unix_ms = now_ms;
    let merged_summary = match (
        prev.content.summary_text.trim().is_empty(),
        incoming.content.summary_text.trim().is_empty(),
    ) {
        (true, true) => String::new(),
        (false, true) => prev.content.summary_text.clone(),
        (true, false) => incoming.content.summary_text.clone(),
        (false, false) => format!(
            "{}\n---\n{}",
            prev.content.summary_text, incoming.content.summary_text
        ),
    };
    out.content.summary_text = merged_summary;
    out.content.facts = merge_facts_crdt(&prev.content.facts, &incoming.content.facts);
    out.content.tags = merge_tags(&prev.content.tags, &incoming.content.tags);
    if out.content.structured_payload.is_none() {
        out.content.structured_payload = prev.content.structured_payload.clone();
    }
    if out.agentos.is_none() {
        out.agentos = prev.agentos.clone();
    }
    out.provenance = merge_provenance_crdt(&prev.provenance, &incoming.provenance);
    out
}

fn merge_facts_crdt(prev: &[ContextFact], incoming: &[ContextFact]) -> Vec<ContextFact> {
    let mut map: HashMap<String, ContextFact> = HashMap::new();
    for f in prev {
        map.insert(f.fact_id.clone(), f.clone());
    }
    for f in incoming {
        map.insert(f.fact_id.clone(), f.clone());
    }
    let mut v: Vec<ContextFact> = map.into_values().collect();
    v.sort_by(|a, b| a.fact_id.cmp(&b.fact_id));
    v
}

fn merge_tags(prev: &[String], incoming: &[String]) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    let mut ordered: Vec<String> = Vec::new();
    for t in prev.iter().chain(incoming.iter()) {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        if set.insert(t.to_string()) {
            ordered.push(t.to_string());
        }
    }
    ordered
}

fn merge_provenance_crdt(
    prev: &ContextProvenance,
    incoming: &ContextProvenance,
) -> ContextProvenance {
    let mut out = incoming.clone();
    if out.trace_id.is_none() {
        out.trace_id = prev.trace_id.clone();
    }
    if out.correlation_id.is_none() {
        out.correlation_id = prev.correlation_id.clone();
    }
    out.capture_mode = crate::context_envelope::ContextCaptureMode::Derived;
    let mut seen: HashSet<String> = out.observed_via.iter().cloned().collect();
    for o in &prev.observed_via {
        if seen.insert(o.clone()) {
            out.observed_via.push(o.clone());
        }
    }
    out.observed_via
        .push("context_lifecycle:crdt_merge".to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_envelope::{
        ContextBudget, ContextConflictPolicy, ContextContent, ContextEnvelopeType,
        ContextInjectionMode, ContextPriority, ContextSubject, ContextTrust, ContextTrustTier,
    };

    fn minimal_envelope(repo: &str, session: Option<&str>) -> ContextEnvelope {
        let now = 1_700_000_000_000_u64;
        ContextEnvelope {
            schema_version: 1,
            envelope_type: ContextEnvelopeType::RetrievalEvidence,
            envelope_id: "e1".to_string(),
            created_at_unix_ms: now,
            expires_at_unix_ms: Some(now + 3_600_000),
            ttl_seconds: Some(3600),
            provenance: ContextProvenance {
                source_plane: crate::context_envelope::ContextSourcePlane::Mcp,
                source_system: "test".to_string(),
                source_tool: None,
                source_path: None,
                producer_agent_id: None,
                producer_node_id: None,
                producer_session_id: None,
                producer_thread_id: None,
                capture_mode: crate::context_envelope::ContextCaptureMode::Inline,
                policy_version: None,
                observed_via: Vec::new(),
                trace_id: None,
                correlation_id: None,
            },
            trust: ContextTrust {
                trust_tier: ContextTrustTier::Trusted,
                authority_rank: 50,
                freshness_tier: crate::context_envelope::ContextFreshnessTier::Recent,
                confidence: Some(0.8),
                contradiction_ratio: None,
                requires_citation: None,
                may_override_lower_authority: None,
            },
            lineage: None,
            subject: ContextSubject {
                repository_id: repo.to_string(),
                workspace_id: None,
                session_id: session.map(ToOwned::to_owned),
                thread_id: None,
                task_id: None,
                goal_id: None,
                agent_id: None,
                receiver_agent_id: None,
                node_id: None,
                populi_scope_id: None,
                surface: None,
            },
            content: ContextContent {
                summary_text: "s".to_string(),
                facts: Vec::new(),
                repo_paths: Vec::new(),
                artifact_refs: Vec::new(),
                citations: Vec::new(),
                tags: Vec::new(),
                structured_payload: None,
                truncated_warnings: Vec::new(),
            },
            conflict_policy: ContextConflictPolicy {
                merge_strategy: ContextMergeStrategy::LastWriteWins,
                stale_after_ms: None,
                dedupe_key: None,
                overwrite_requires_evidence: None,
                conflict_class: None,
            },
            budget: ContextBudget {
                priority: ContextPriority::Normal,
                injection_mode: ContextInjectionMode::Inline,
                token_estimate: None,
                max_tokens_for_injection: None,
                retrieval_cost_class: None,
                must_refresh_before_use: None,
            },
            safety: None,
            obo_token: None,
            operating_mode: None,
            agentos: None,
        }
    }

    #[test]
    fn validate_rejects_repo_mismatch() {
        let env = minimal_envelope("a", Some("s1"));
        let errs = validate_context_envelope_ingest(
            &env,
            ContextIngestExpectations {
                repository_id: "b",
                session_id: Some("s1"),
            },
            env.created_at_unix_ms + 100,
        )
        .unwrap_err();
        assert!(errs.iter().any(|e| e.contains("repository_id")));
    }

    #[test]
    fn validate_rejects_session_mismatch_when_expected() {
        let env = minimal_envelope("repo", Some("s2"));
        let errs = validate_context_envelope_ingest(
            &env,
            ContextIngestExpectations {
                repository_id: "repo",
                session_id: Some("s1"),
            },
            env.created_at_unix_ms + 100,
        )
        .unwrap_err();
        assert!(errs.iter().any(|e| e.contains("session_id")));
    }

    #[test]
    fn merge_authority_prefers_higher_incoming() {
        let mut low = minimal_envelope("repo", None);
        low.trust.authority_rank = 10;
        low.conflict_policy.merge_strategy = ContextMergeStrategy::AuthorityPrecedence;
        let mut high = low.clone();
        high.envelope_id = "e2".to_string();
        high.trust.authority_rank = 90;
        let merged = merge_context_envelope_for_session_store(
            Some(&serde_json::to_string(&low).unwrap()),
            &high,
            false,
        )
        .unwrap();
        assert_eq!(merged.trust.authority_rank, 90);
    }

    #[test]
    fn merge_manual_review_errors_when_existing() {
        let mut prev = minimal_envelope("repo", None);
        prev.conflict_policy.merge_strategy = ContextMergeStrategy::ManualReview;
        let mut inc = prev.clone();
        inc.envelope_id = "e2".to_string();
        inc.conflict_policy.merge_strategy = ContextMergeStrategy::ManualReview;
        let err = merge_context_envelope_for_session_store(
            Some(&serde_json::to_string(&prev).unwrap()),
            &inc,
            false,
        )
        .unwrap_err();
        assert!(err.contains("ManualReview"));
    }

    #[test]
    fn clamp_truncates_summary_when_max_tokens_for_injection_set() {
        let mut env = minimal_envelope("repo", None);
        env.content.summary_text = "x".repeat(500);
        env.budget.max_tokens_for_injection = Some(20);
        clamp_context_envelope_injection_budget(&mut env);
        assert!(env.content.summary_text.len() < 500);
        assert!(env.content.summary_text.contains("truncated"));
        assert_eq!(env.budget.token_estimate, Some(20));
    }
}
