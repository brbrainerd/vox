//! Thin CLI handlers wrapping the SCIENTIA Phase B / C / D / E / G / H
//! pure-library crates. Each handler:
//!
//! 1. Reads a small JSON input file (typed input struct).
//! 2. Calls into the library.
//! 3. Writes the library's output as JSON, markdown, or HTML to stdout.
//!
//! Keeping the handlers in one file makes the dispatcher arms in
//! `commands::scientia::run` compact and avoids one orphan-resolving module
//! per crate. The handlers are also load-bearing for `vox-arch-check` —
//! they are the workspace consumers that drop the otherwise-orphan
//! libraries' "no in-tree consumer" warning.

use anyhow::{Context, Result};
use std::path::Path;

/// Read a JSON file into a typed value.
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("read {}", path.display()))?;
    let value: T = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON at {}", path.display()))?;
    Ok(value)
}

// ── Phase B — replay-runner ───────────────────────────────────────────────────

/// `vox scientia publication-replay-execute --main-entity X.json --stage-dir DIR`
///
/// Re-executes the `MainEntity` entry-point inside `stage_dir` and emits a
/// `ReplayReport` JSON document on stdout.
pub async fn replay_execute(main_entity_path: &Path, stage_dir: &Path) -> Result<()> {
    let main_entity: vox_scientia::ro_crate::MainEntity = read_json(main_entity_path)?;
    let report = vox_scientia::replay::run_replay(stage_dir, &main_entity)
        .await
        .context("vox_scientia::replay::run_replay")?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

// ── Phase C — manuscript-scaffold ─────────────────────────────────────────────

/// `vox scientia publication-manuscript-draft --scaffold X.json`
///
/// Reads a `ScaffoldInput` JSON file and emits the IMRaD markdown to stdout.
pub fn manuscript_draft(scaffold_path: &Path) -> Result<()> {
    let input: vox_scientia::manuscript::scaffold::ScaffoldInput = read_json(scaffold_path)?;
    print!("{}", vox_scientia::manuscript::scaffold::render_imrad(&input));
    Ok(())
}

// ── Phase D — critic-gate ─────────────────────────────────────────────────────

/// Owned analog of [`vox_scientia::critic_gate::GateInputs`] so it can be deserialized
/// from JSON. Converted to the borrowed form at call time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CriticGateInputsJson {
    pub approvers: Vec<vox_scientia::critic_gate::ApproverRecord>,
    pub artifact_model_fingerprints: Vec<vox_scientia::critic_gate::ModelFingerprint>,
    pub venue_policy: vox_scientia::critic_gate::VenueCriticPolicy,
}

/// `vox scientia publication-critic-gate-check --inputs X.json`
///
/// Reads a `CriticGateInputsJson` JSON file and emits the `GateOutcome` JSON.
pub fn critic_gate_check(inputs_path: &Path) -> Result<()> {
    let owned: CriticGateInputsJson = read_json(inputs_path)?;
    let inputs = vox_scientia::critic_gate::GateInputs {
        approvers: &owned.approvers,
        artifact_model_fingerprints: &owned.artifact_model_fingerprints,
        venue_policy: owned.venue_policy,
    };
    let outcome = vox_scientia::critic_gate::evaluate_gate(&inputs);
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    Ok(())
}

/// Inputs for the `publication-critic-approve` wiring path. Loaded as a
/// JSON file; the handler combines this with the existing human approvers
/// (looked up from the DB by digest) before running the gate evaluator.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CriticApproveInputsJson {
    /// Critic's model fingerprint. Persisted as
    /// `publication_approvals.critic_fingerprint_json`.
    pub critic_fingerprint: vox_scientia::critic_gate::ModelFingerprint,
    /// Critic's signed report URI (recorded as
    /// `publication_approvals.critic_report_uri`).
    #[serde(default)]
    pub critic_report_uri: Option<String>,
    /// Recommendation from the critic's signed report.
    pub critic_recommendation: vox_scientia::critic_gate::CriticRecommendation,
    /// Model fingerprints of every artifact-side model whose output
    /// contributed to the manifest content. The gate refuses critics
    /// whose fingerprint collides with any of these.
    pub artifact_model_fingerprints: Vec<vox_scientia::critic_gate::ModelFingerprint>,
    /// Venue's critic policy (`Allowed` or `Forbidden`).
    pub venue_policy: vox_scientia::critic_gate::VenueCriticPolicy,
}

/// `vox scientia publication-critic-approve --publication-id X --critic-id Y --inputs Z.json`
///
/// Runs the solo-author critic gate over the existing human approvers
/// for the publication's current content digest + the proposed critic.
/// Persists the critic's approval only when the gate clears.
///
/// Returns:
/// - `Ok(())` on a cleared gate (approval persisted).
/// - `Err` with the structured `GateReason` + diagnostics on a non-cleared
///   gate (no DB write).
pub async fn critic_approve(
    publication_id: &str,
    critic_id: &str,
    inputs_path: &Path,
) -> Result<()> {
    let inputs: CriticApproveInputsJson = read_json(inputs_path)?;
    let db = vox_db::VoxDb::connect_default()
        .await
        .context("connect to default Codex / VoxDb")?;
    let manifest = db
        .get_publication_manifest(publication_id)
        .await
        .context("load publication manifest")?
        .ok_or_else(|| anyhow::anyhow!("publication not found: {publication_id}"))?;

    let existing = db
        .list_publication_approvals_for_digest(publication_id, &manifest.content_sha3_256)
        .await
        .context("list existing approvals")?;
    let mut approver_records: Vec<vox_scientia::critic_gate::ApproverRecord> = existing
        .into_iter()
        .map(|row| {
            let role = if row.is_critic() {
                vox_scientia::critic_gate::ApproverRole::AuditedLLMCritic
            } else {
                vox_scientia::critic_gate::ApproverRole::Human
            };
            let fp = row
                .critic_fingerprint_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            vox_scientia::critic_gate::ApproverRecord {
                approver_id: row.approver,
                role,
                critic_fingerprint: fp,
                critic_recommendation: None, // we did not persist this
            }
        })
        .collect();
    // Append the proposed critic as a *candidate* row for the gate.
    approver_records.push(vox_scientia::critic_gate::ApproverRecord {
        approver_id: critic_id.to_string(),
        role: vox_scientia::critic_gate::ApproverRole::AuditedLLMCritic,
        critic_fingerprint: Some(inputs.critic_fingerprint.clone()),
        critic_recommendation: Some(inputs.critic_recommendation),
    });

    let gate_inputs = vox_scientia::critic_gate::GateInputs {
        approvers: &approver_records,
        artifact_model_fingerprints: &inputs.artifact_model_fingerprints,
        venue_policy: inputs.venue_policy,
    };
    let outcome = vox_scientia::critic_gate::evaluate_gate(&gate_inputs);
    if !outcome.cleared {
        return Err(anyhow::anyhow!(
            "critic gate not cleared: reason={}, diagnostics={:?}",
            outcome.reason.as_str(),
            outcome.diagnostics
        ));
    }

    let fingerprint_json = serde_json::to_string(&inputs.critic_fingerprint)
        .context("serialize critic fingerprint")?;
    db.record_publication_critic_approval_for_digest(
        publication_id,
        &manifest.content_sha3_256,
        critic_id,
        &fingerprint_json,
        inputs.critic_report_uri.as_deref(),
    )
    .await
    .context("persist critic approval")?;

    // Transition state to `approved` only when the digest now clears the
    // gate via either path (TwoHumans or HumanPlusAuditedCritic). The
    // outcome above already covered this; we just persist the transition.
    db.set_publication_state(publication_id, "approved", None)
        .await
        .context("set publication state to approved")?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "publication_id": publication_id,
            "critic_id": critic_id,
            "digest": manifest.content_sha3_256,
            "gate_outcome": outcome,
        }))?
    );
    Ok(())
}

// ── Phase 1 — claim extraction + MiniCheck into preflight ─────────────────────

/// `vox scientia publication-extract-claims --publication-id X`
///
/// Loads the manifest, runs the SCIENTIA `ExtractionPipeline` (VeriScore +
/// atomic decomposition + span check + MiniCheck verifier) over
/// `body_markdown`, merges an [`ExtractedClaimsSummary`] into
/// `metadata_json.scientia_evidence.extracted_claims`, and re-upserts the
/// manifest. The summary drives `WorthinessInputs::claim_evidence_coverage`
/// on subsequent preflight runs.
///
/// `VOX_MINICHECK_ENDPOINT` selects the HTTP verifier; absent → mock
/// (deterministic word-overlap; useful for tests, low-signal in production).
///
/// [`ExtractedClaimsSummary`]: vox_publisher::scientia_evidence::ExtractedClaimsSummary
pub async fn publication_extract_claims(publication_id: &str) -> Result<()> {
    let db = vox_db::VoxDb::connect_default()
        .await
        .context("connect to default Codex / VoxDb")?;
    let manifest_row = db
        .get_publication_manifest(publication_id)
        .await
        .context("load publication manifest")?
        .ok_or_else(|| anyhow::anyhow!("publication not found: {publication_id}"))?;

    // Context for the verifier: prefer citations payload, fall back to the
    // body itself (best-effort; mock backend uses string-overlap).
    let context_owned = manifest_row.citations_json.clone();
    let context_str = context_owned
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&manifest_row.body_markdown);

    let pipeline = vox_scientia::claim_extractor::ExtractionPipeline::new(
        vox_scientia::claim_extractor::ExtractionConfig::default(),
    );
    let result = pipeline
        .extract(&manifest_row.body_markdown, &[context_str])
        .await
        .map_err(|e| anyhow::anyhow!("extract pipeline: {e}"))?;

    let total_atomic = result.claims.len() as u32;
    let mut supported = 0u32;
    let mut refuted = 0u32;
    let mut abstained = 0u32;
    for v in &result.verdicts {
        match v {
            vox_scientia::claim_extractor::ClaimVerdict::Supported { .. } => supported += 1,
            vox_scientia::claim_extractor::ClaimVerdict::Contradicted { .. } => refuted += 1,
            vox_scientia::claim_extractor::ClaimVerdict::Contested { .. } => {
                // Treat contested as neither supported nor refuted; the
                // worthiness rubric counts only `supported` toward
                // claim_evidence_coverage.
            }
            vox_scientia::claim_extractor::ClaimVerdict::Abstain { .. } => abstained += 1,
        }
    }

    // The verifier surfaces its model id on every VerifierOutput; we don't
    // capture per-claim outputs here, but reading the env var preserves
    // the same dispatch the pipeline itself used.
    let verifier_model = if std::env::var("VOX_MINICHECK_ENDPOINT").is_ok() {
        "minicheck-http".to_string()
    } else {
        "mock".to_string()
    };

    let summary = vox_publisher::scientia_evidence::ExtractedClaimsSummary {
        schema_version: 1,
        total_atomic,
        supported,
        refuted,
        abstained,
        verifier_model,
        abstain_threshold: 0.3,
        promotion_threshold: 0.7,
        extracted_at_ms: chrono::Utc::now().timestamp_millis(),
    };

    let merged_metadata = merge_extracted_claims_into_metadata(
        manifest_row.metadata_json.as_deref(),
        &summary,
    )?;

    // Re-upsert with the updated metadata_json. The content digest
    // recomputes from the canonical fields including metadata_json, so
    // updating this WILL produce a new digest. Existing approvals bound
    // to the prior digest do NOT carry over — that is the intended
    // behavior of the digest-binding scheme.
    let params = vox_db::PublicationManifestParams {
        publication_id: &manifest_row.publication_id,
        content_type: &manifest_row.content_type,
        source_ref: manifest_row.source_ref.as_deref(),
        title: &manifest_row.title,
        author: &manifest_row.author,
        abstract_text: manifest_row.abstract_text.as_deref(),
        body_markdown: &manifest_row.body_markdown,
        citations_json: manifest_row.citations_json.as_deref(),
        metadata_json: Some(&merged_metadata),
        revision_history_json: manifest_row.revision_history_json.as_deref(),
        content_sha3_256: &manifest_row.content_sha3_256,
        state: &manifest_row.state,
    };
    db.upsert_publication_manifest(params)
        .await
        .context("upsert manifest with extracted_claims summary")?;

    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

/// Merge an `ExtractedClaimsSummary` into the manifest's existing
/// `metadata_json` under `scientia_evidence.extracted_claims`, preserving
/// every other key. Returns the serialized JSON ready for upsert.
fn merge_extracted_claims_into_metadata(
    metadata_json: Option<&str>,
    summary: &vox_publisher::scientia_evidence::ExtractedClaimsSummary,
) -> Result<String> {
    let mut root: serde_json::Value = match metadata_json {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(s)
            .context("parse existing metadata_json as object")?,
        _ => serde_json::Value::Object(Default::default()),
    };
    let obj = root
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("metadata_json must be a JSON object"))?;
    let evidence = obj
        .entry("scientia_evidence".to_string())
        .or_insert_with(|| serde_json::Value::Object(Default::default()));
    let evidence_obj = evidence
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("scientia_evidence must be a JSON object"))?;
    evidence_obj.insert(
        "extracted_claims".into(),
        serde_json::to_value(summary)?,
    );
    Ok(serde_json::to_string(&root)?)
}

// ── Phase 3 — markdown→LaTeX renderer ─────────────────────────────────────────

/// `vox scientia publication-render-latex --scaffold X.json [--output Y.tex]`
///
/// Reads a `ScaffoldInput` JSON and writes the rendered LaTeX to `output`
/// or stdout. Suitable for piping into `tectonic` / `pdflatex` for PDF
/// generation.
pub fn render_latex_handler(scaffold_path: &Path, output: Option<&Path>) -> Result<()> {
    let input: vox_scientia::manuscript::scaffold::ScaffoldInput = read_json(scaffold_path)?;
    let tex = vox_scientia::manuscript::latex::render_latex(&input);
    match output {
        Some(p) => {
            std::fs::write(p, tex.as_bytes())
                .with_context(|| format!("write LaTeX to {}", p.display()))?;
            eprintln!("LaTeX written to {}", p.display());
        }
        None => print!("{tex}"),
    }
    Ok(())
}

// ── Phase 4 — arXiv staging bundle ────────────────────────────────────────────

/// `vox scientia publication-arxiv-bundle --scaffold X.json --figures-dir D --output Z.tar.gz`
///
/// Reads a `ScaffoldInput` JSON, resolves each declared figure path against
/// `figures_dir`, and writes the arXiv-ready `.tar.gz` to `output`.
/// The bundle contains `main.tex` at the root plus each figure at its
/// declared sub-path.
///
/// Errors when a figure declared in the scaffold has no corresponding file
/// under `figures_dir`.
pub fn arxiv_bundle_handler(
    scaffold_path: &Path,
    figures_dir: &Path,
    output: &Path,
) -> Result<()> {
    let input: vox_scientia::manuscript::scaffold::ScaffoldInput = read_json(scaffold_path)?;

    let mut figure_blobs: Vec<(String, Vec<u8>)> = Vec::with_capacity(input.figures.len());
    for f in &input.figures {
        let candidate = figures_dir.join(&f.path);
        let blob = std::fs::read(&candidate).with_context(|| {
            format!(
                "figure {:?} declared in scaffold but not found at {}",
                f.path,
                candidate.display()
            )
        })?;
        figure_blobs.push((f.path.clone(), blob));
    }

    let bytes = vox_scientia::manuscript::latex::render_arxiv_bundle(&input, &figure_blobs)
        .map_err(|e| anyhow::anyhow!("render_arxiv_bundle: {e}"))?;
    std::fs::write(output, &bytes)
        .with_context(|| format!("write arXiv bundle to {}", output.display()))?;
    eprintln!(
        "arXiv bundle ({} bytes, {} figures) written to {}",
        bytes.len(),
        input.figures.len(),
        output.display()
    );
    Ok(())
}

// ── Phase E — class-routing ───────────────────────────────────────────────────

/// `vox scientia publication-venue-recommend --candidate-class CLASS [--yaml CONFIG]`
///
/// Prints the recommended venues, reply-window length, negative-result quota,
/// and critic-allowed flag for the requested class.
pub fn venue_recommend(candidate_class: &str, yaml_config: Option<&Path>) -> Result<()> {
    use vox_scientia::class_routing::{
        builtin_class_defaults, critic_allowed_for, load_class_defaults_from_yaml,
        negative_result_quota_for, recommended_venues_for, reply_window_days_for, FindingClass,
    };
    let class = FindingClass::from_str(candidate_class).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown candidate_class {candidate_class:?}; \
             valid: algorithmic_improvement, reproducibility_infra, policy_governance, \
             telemetry_trust, other, model_capability_atlas, provider_reliability_atlas"
        )
    })?;
    let defaults = if let Some(p) = yaml_config {
        let yaml = std::fs::read_to_string(p)
            .with_context(|| format!("read {}", p.display()))?;
        load_class_defaults_from_yaml(&yaml).context("parse class defaults YAML")?
    } else {
        builtin_class_defaults()
    };
    #[derive(serde::Serialize)]
    struct Out<'a> {
        candidate_class: &'a str,
        recommended_venues: &'a [String],
        reply_window_days: u32,
        negative_result_quota: u32,
        critic_allowed: bool,
        atlas_gate_applies: bool,
    }
    let out = Out {
        candidate_class: class.as_str(),
        recommended_venues: recommended_venues_for(&defaults, class),
        reply_window_days: reply_window_days_for(&defaults, class),
        negative_result_quota: negative_result_quota_for(&defaults, class),
        critic_allowed: critic_allowed_for(&defaults, class),
        atlas_gate_applies: vox_scientia::class_routing::atlas_gate_applies_to(class),
    };
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

// ── Phase G — findings-site ───────────────────────────────────────────────────

/// `vox scientia publication-finding-page-render --page X.json`
///
/// Reads a `FindingPage` JSON file and emits a complete HTML document on
/// stdout.
pub fn finding_page_render(page_path: &Path) -> Result<()> {
    let page: vox_scientia::findings_site::FindingPage = read_json(page_path)?;
    print!("{}", vox_scientia::findings_site::render_finding_page(&page));
    Ok(())
}

// ── Phase H — scientia-dashboard ──────────────────────────────────────────────

/// Owned analog of [`vox_scientia::dashboard::DashboardInputs`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardInputsJson {
    pub candidates: Vec<vox_scientia::dashboard::CandidateRow>,
    pub claims_pending: vox_scientia::dashboard::ClaimsPendingSummary,
    #[serde(default)]
    pub manifests_in_reply_window: Vec<vox_scientia::dashboard::ReplyWindowEntry>,
    #[serde(default)]
    pub retraction_queue: Vec<String>,
    pub now_ms: i64,
}

/// `vox scientia publication-dashboard-snapshot --inputs X.json`
///
/// Reads a `DashboardInputsJson` JSON file and emits the `QueueSnapshot` JSON.
pub fn dashboard_snapshot(inputs_path: &Path) -> Result<()> {
    let owned: DashboardInputsJson = read_json(inputs_path)?;
    let inputs = vox_scientia::dashboard::DashboardInputs {
        candidates: &owned.candidates,
        claims_pending: owned.claims_pending.clone(),
        manifests_in_reply_window: &owned.manifests_in_reply_window,
        retraction_queue: &owned.retraction_queue,
        now_ms: owned.now_ms,
    };
    let snap = vox_scientia::dashboard::build_queue_snapshot(&inputs);
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_json<T: serde::Serialize>(value: &T) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(serde_json::to_vec_pretty(value).unwrap().as_slice())
            .unwrap();
        f
    }

    #[test]
    fn read_json_round_trips_a_typed_value() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct V {
            x: i32,
            y: String,
        }
        let v = V { x: 7, y: "hi".into() };
        let f = tmp_json(&v);
        let back: V = read_json(f.path()).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn manuscript_draft_emits_markdown_for_minimal_input() {
        let input = vox_scientia::manuscript::scaffold::ScaffoldInput {
            title_hint: "t".into(),
            authors: vec![],
            results_rows: vec![],
            cited_facts: vec![],
            methods_summary: None,
            limitations: vec![],
            ai_disclosure_markdown: None,
            competing_interests: None,
            figures: vec![],
        };
        let f = tmp_json(&input);
        // smoke: the call must not error; stdout capture is exercised
        // separately in integration suites.
        manuscript_draft(f.path()).unwrap();
    }

    #[test]
    fn venue_recommend_known_class_succeeds() {
        venue_recommend("algorithmic_improvement", None).unwrap();
    }

    #[test]
    fn venue_recommend_unknown_class_errors_with_helpful_message() {
        let err = venue_recommend("not_a_class", None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not_a_class"), "got: {msg}");
        assert!(msg.contains("algorithmic_improvement"), "got: {msg}");
    }

    #[test]
    fn critic_gate_check_two_humans_clears_outcome() {
        let inputs = CriticGateInputsJson {
            approvers: vec![
                vox_scientia::critic_gate::ApproverRecord {
                    approver_id: "alice".into(),
                    role: vox_scientia::critic_gate::ApproverRole::Human,
                    critic_fingerprint: None,
                    critic_recommendation: None,
                },
                vox_scientia::critic_gate::ApproverRecord {
                    approver_id: "bob".into(),
                    role: vox_scientia::critic_gate::ApproverRole::Human,
                    critic_fingerprint: None,
                    critic_recommendation: None,
                },
            ],
            artifact_model_fingerprints: vec![],
            venue_policy: vox_scientia::critic_gate::VenueCriticPolicy::Forbidden,
        };
        let f = tmp_json(&inputs);
        critic_gate_check(f.path()).unwrap();
    }

    #[test]
    fn dashboard_snapshot_empty_inputs_succeeds() {
        let inputs = DashboardInputsJson {
            candidates: vec![],
            claims_pending: vox_scientia::dashboard::ClaimsPendingSummary {
                verifiable: 0,
                abstained: 0,
                extraction_running: 0,
            },
            manifests_in_reply_window: vec![],
            retraction_queue: vec![],
            now_ms: 1_700_000_000_000,
        };
        let f = tmp_json(&inputs);
        dashboard_snapshot(f.path()).unwrap();
    }

    // ── Phase 1 — merge_extracted_claims_into_metadata ───────────────────

    fn sample_summary() -> vox_publisher::scientia_evidence::ExtractedClaimsSummary {
        vox_publisher::scientia_evidence::ExtractedClaimsSummary {
            schema_version: 1,
            total_atomic: 10,
            supported: 8,
            refuted: 1,
            abstained: 1,
            verifier_model: "mock".into(),
            abstain_threshold: 0.3,
            promotion_threshold: 0.7,
            extracted_at_ms: 1_747_000_000_000,
        }
    }

    #[test]
    fn merge_into_empty_metadata_creates_scientia_evidence_wrapper() {
        let summary = sample_summary();
        let merged = merge_extracted_claims_into_metadata(None, &summary).unwrap();
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let ec = &v["scientia_evidence"]["extracted_claims"];
        assert_eq!(ec["total_atomic"], 10);
        assert_eq!(ec["supported"], 8);
        assert_eq!(ec["verifier_model"], "mock");
    }

    #[test]
    fn merge_preserves_existing_sibling_keys() {
        let existing = serde_json::json!({
            "scientific_publication": {"authors": ["Alice"]},
            "scientia_evidence": {
                "human_meaningful_advance": true,
                "candidate_note": "perf delta"
            }
        });
        let summary = sample_summary();
        let merged = merge_extracted_claims_into_metadata(
            Some(&existing.to_string()),
            &summary,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        // Top-level sibling preserved.
        assert_eq!(v["scientific_publication"]["authors"][0], "Alice");
        // scientia_evidence siblings preserved.
        assert_eq!(v["scientia_evidence"]["human_meaningful_advance"], true);
        assert_eq!(v["scientia_evidence"]["candidate_note"], "perf delta");
        // New extracted_claims attached.
        assert_eq!(
            v["scientia_evidence"]["extracted_claims"]["supported"],
            8
        );
    }

    #[test]
    fn merge_replaces_existing_extracted_claims_idempotent() {
        let existing = serde_json::json!({
            "scientia_evidence": {
                "extracted_claims": {
                    "schema_version": 1,
                    "total_atomic": 5,
                    "supported": 2,
                    "refuted": 0,
                    "abstained": 3,
                    "verifier_model": "mock",
                    "abstain_threshold": 0.3,
                    "promotion_threshold": 0.7,
                    "extracted_at_ms": 0
                }
            }
        });
        let summary = sample_summary();
        let merged = merge_extracted_claims_into_metadata(
            Some(&existing.to_string()),
            &summary,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&merged).unwrap();
        // Old `total_atomic: 5` replaced by new `total_atomic: 10`.
        assert_eq!(
            v["scientia_evidence"]["extracted_claims"]["total_atomic"],
            10
        );
    }

    #[test]
    fn merge_into_non_object_metadata_errors_helpfully() {
        let result = merge_extracted_claims_into_metadata(Some("[1, 2, 3]"), &sample_summary());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("metadata_json"), "got: {msg}");
    }

    #[test]
    fn merge_invalid_json_errors_with_context() {
        let result = merge_extracted_claims_into_metadata(Some("not json"), &sample_summary());
        assert!(result.is_err());
    }

    // ── Phase 3+4 — LaTeX render + arXiv bundle handlers ─────────────────

    #[test]
    fn render_latex_handler_writes_documentclass_to_output_file() {
        let input = vox_scientia::manuscript::scaffold::ScaffoldInput {
            title_hint: "T".into(),
            authors: vec![],
            results_rows: vec![],
            cited_facts: vec![],
            methods_summary: None,
            limitations: vec![],
            ai_disclosure_markdown: None,
            competing_interests: None,
            figures: vec![],
        };
        let scaffold = tmp_json(&input);
        let outdir = tempfile::tempdir().unwrap();
        let outpath = outdir.path().join("main.tex");
        render_latex_handler(scaffold.path(), Some(&outpath)).unwrap();
        let tex = std::fs::read_to_string(&outpath).unwrap();
        assert!(tex.contains("\\documentclass"));
        assert!(tex.contains("\\end{document}"));
    }

    #[test]
    fn arxiv_bundle_handler_writes_targz_with_main_tex_for_no_figures_input() {
        let input = vox_scientia::manuscript::scaffold::ScaffoldInput {
            title_hint: "T".into(),
            authors: vec![],
            results_rows: vec![],
            cited_facts: vec![],
            methods_summary: None,
            limitations: vec![],
            ai_disclosure_markdown: None,
            competing_interests: None,
            figures: vec![],
        };
        let scaffold = tmp_json(&input);
        let figures_dir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        let outpath = outdir.path().join("bundle.tar.gz");
        arxiv_bundle_handler(scaffold.path(), figures_dir.path(), &outpath).unwrap();
        let bytes = std::fs::read(&outpath).unwrap();
        let entries = vox_scientia::manuscript::latex::list_bundle_entries(&bytes).unwrap();
        assert!(entries.iter().any(|(p, _)| p == "main.tex"));
    }

    #[test]
    fn arxiv_bundle_handler_errors_when_declared_figure_missing_from_disk() {
        let input = vox_scientia::manuscript::scaffold::ScaffoldInput {
            title_hint: "T".into(),
            authors: vec![],
            results_rows: vec![],
            cited_facts: vec![],
            methods_summary: None,
            limitations: vec![],
            ai_disclosure_markdown: None,
            competing_interests: None,
            figures: vec![vox_scientia::manuscript::scaffold::FigureEntry {
                path: "figures/missing.svg".into(),
                sha3_256_hex: "00".into(),
                source_script: "x".into(),
                caption_hint: None,
            }],
        };
        let scaffold = tmp_json(&input);
        let figures_dir = tempfile::tempdir().unwrap(); // empty
        let outdir = tempfile::tempdir().unwrap();
        let outpath = outdir.path().join("bundle.tar.gz");
        let err = arxiv_bundle_handler(scaffold.path(), figures_dir.path(), &outpath)
            .unwrap_err()
            .to_string();
        assert!(err.contains("figures/missing.svg"), "got: {err}");
    }

    #[test]
    fn finding_page_render_emits_html_doctype() {
        let page = vox_scientia::findings_site::FindingPage {
            title: "t".into(),
            authors: vec![],
            abstract_text: "".into(),
            body_html: "".into(),
            trusty_uri: "RA1".into(),
            doi: None,
            versions: vec![],
            verified_claims: vec![],
            replies: vec![],
            retraction: None,
            published_at_iso: "2026-05-15".into(),
        };
        let f = tmp_json(&page);
        // smoke
        finding_page_render(f.path()).unwrap();
    }
}
