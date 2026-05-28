//! Contract-first natural-language harness representation for orchestration and handoff surfaces.

use std::collections::HashSet;

use crate::generated::agent_harness::{
    Adapter as HarnessAdapter, AgentHarnessSpec, CompletionGate as HarnessGate,
    Contracts as HarnessContracts, FailureTaxonomy as HarnessFailureMode,
    RequiredOutput as HarnessArtifactSpec, Role as HarnessRole, Stage as HarnessStage,
    State as HarnessState, Subject as HarnessSubject,
};

/// Anti-bleed expectations when a harness is supplied by MCP, handoff, or remote execution.
#[derive(Debug, Clone, Copy)]
pub struct HarnessIngestExpectations<'a> {
    pub repository_id: &'a str,
    pub session_id: Option<&'a str>,
    pub thread_id: Option<&'a str>,
}

impl AgentHarnessSpec {
    /// Build a conservative default harness aligned with Vox's current contract-first execution loop.
    #[must_use]
    pub fn minimal_contract_first(
        repository_id: &str,
        description: impl Into<String>,
        session_id: Option<&str>,
        thread_id: Option<&str>,
        output_path_hints: &[String],
    ) -> Self {
        let mut required_outputs = Vec::new();
        if output_path_hints.is_empty() {
            required_outputs.push(HarnessArtifactSpec {
                artifact_id: "response_artifact".to_string(),
                kind: "report".to_string(),
                path_hint: Some("artifacts/RESPONSE.md".to_string()),
                required: true,
            });
        } else {
            for (idx, path) in output_path_hints.iter().enumerate() {
                required_outputs.push(HarnessArtifactSpec {
                    artifact_id: format!("artifact_{}", idx + 1),
                    kind: "deliverable".to_string(),
                    path_hint: Some(path.clone()),
                    required: true,
                });
            }
        }
        Self {
            schema_version: 1,
            harness_id: "vox.contract_first.default.v1".to_string(),
            description: Some(description.into()),
            task_family: Some("vox_task_execution".to_string()),
            runtime_charter: "vox_orchestrator_runtime_charter_v1".to_string(),
            subject: HarnessSubject {
                repository_id: repository_id.to_string(),
                session_id: session_id
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned),
                thread_id: thread_id
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned),
            },
            roles: vec![
                HarnessRole {
                    role_id: "orchestrator".to_string(),
                    prompt_purpose: "Own routing, reporting, and completion gating.".to_string(),
                    responsibilities: vec![
                        "Choose the next runnable stage.".to_string(),
                        "Preserve durable artifacts and lineage.".to_string(),
                    ],
                    permission_mode: Some("orchestrate_only".to_string()),
                },
                HarnessRole {
                    role_id: "worker".to_string(),
                    prompt_purpose: "Perform substantive task work within declared scope."
                        .to_string(),
                    responsibilities: vec![
                        "Produce required artifacts.".to_string(),
                        "Return evidence needed for completion.".to_string(),
                    ],
                    permission_mode: Some("task_workspace".to_string()),
                },
            ],
            stages: vec![
                HarnessStage {
                    stage_id: "execute".to_string(),
                    role_id: "worker".to_string(),
                    summary: "Perform the task and materialize designated outputs.".to_string(),
                    input_artifacts: Some(vec![]),
                    output_artifacts: Some(
                        required_outputs
                            .iter()
                            .map(|x| x.artifact_id.clone())
                            .collect(),
                    ),
                    completion_gate_ids: Some(vec!["artifact_delivery".to_string()]),
                    failure_mode_codes: Some(vec!["tool_error".to_string(), "timeout".to_string()]),
                },
                HarnessStage {
                    stage_id: "verify_completion".to_string(),
                    role_id: "orchestrator".to_string(),
                    summary: "Check that required outputs and evidence exist before closure."
                        .to_string(),
                    input_artifacts: Some(
                        required_outputs
                            .iter()
                            .map(|x| x.artifact_id.clone())
                            .collect(),
                    ),
                    output_artifacts: Some(vec![]),
                    completion_gate_ids: Some(vec!["artifact_delivery".to_string()]),
                    failure_mode_codes: Some(vec!["missing_artifact".to_string()]),
                },
            ],
            contracts: HarnessContracts {
                required_outputs,
                completion_gates: vec![HarnessGate {
                    gate_id: "artifact_delivery".to_string(),
                    description:
                        "All required outputs must be path-addressable before the task is complete."
                            .to_string(),
                    evidence_required: Some(true),
                    independent_verification: Some(false),
                }],
                max_attempts: Some(3),
                requires_artifact_backed_completion: true,
            },
            adapters: Some(vec![
                HarnessAdapter {
                    adapter_id: "workspace_io".to_string(),
                    kind: "file_system".to_string(),
                    description:
                        "Read, write, and reopen artifacts by path for recovery and audit."
                            .to_string(),
                    target_ref: None,
                },
                HarnessAdapter {
                    adapter_id: "verification_hooks".to_string(),
                    kind: "verification".to_string(),
                    description:
                        "Run the lightest sufficient validation before claiming completion."
                            .to_string(),
                    target_ref: None,
                },
            ]),
            state: HarnessState {
                state_root: "runtime/".to_string(),
                child_workspace_root: Some("children/".to_string()),
                task_history_path: None,
                artifact_manifest_path: Some("artifacts/manifest.json".to_string()),
                path_addressable: true,
                compaction_stable: true,
            },
            failure_taxonomy: vec![
                HarnessFailureMode {
                    code: "missing_artifact".to_string(),
                    description: "A required output path was not materialized.".to_string(),
                    retryable: Some(true),
                    recovery_stage_id: Some("execute".to_string()),
                },
                HarnessFailureMode {
                    code: "tool_error".to_string(),
                    description: "A deterministic adapter failed unexpectedly.".to_string(),
                    retryable: Some(true),
                    recovery_stage_id: Some("execute".to_string()),
                },
                HarnessFailureMode {
                    code: "timeout".to_string(),
                    description: "The task exceeded the run or stage budget.".to_string(),
                    retryable: Some(true),
                    recovery_stage_id: Some("execute".to_string()),
                },
            ],
            metadata: None,
        }
    }
}

fn trim_non_empty(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// Fill missing repository/session/thread continuity fields from the transport boundary.
pub fn apply_harness_subject_defaults(
    harness: &mut AgentHarnessSpec,
    expectations: HarnessIngestExpectations<'_>,
) {
    if harness.subject.repository_id.trim().is_empty() {
        harness.subject.repository_id = expectations.repository_id.to_string();
    }
    if harness.subject.session_id.is_none() {
        harness.subject.session_id = trim_non_empty(expectations.session_id);
    }
    if harness.subject.thread_id.is_none() {
        harness.subject.thread_id = trim_non_empty(expectations.thread_id);
    }
}

/// Structural and continuity validation for a supplied harness artifact.
pub fn validate_agent_harness_ingest(
    harness: &AgentHarnessSpec,
    expectations: HarnessIngestExpectations<'_>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if harness.schema_version != 1 {
        errors.push(format!(
            "unsupported harness schema_version {} (only 1 supported)",
            harness.schema_version
        ));
    }
    if harness.harness_id.trim().is_empty() {
        errors.push("harness_id is empty".to_string());
    }
    if harness.runtime_charter.trim().is_empty() {
        errors.push("runtime_charter is empty".to_string());
    }
    if harness.subject.repository_id.trim().is_empty() {
        errors.push("subject.repository_id is empty".to_string());
    } else if harness.subject.repository_id != expectations.repository_id {
        errors.push(format!(
            "subject.repository_id {:?} does not match expected {:?}",
            harness.subject.repository_id, expectations.repository_id
        ));
    }
    if let Some(expected_session) = trim_non_empty(expectations.session_id)
        && let Some(actual_session) = trim_non_empty(harness.subject.session_id.as_deref())
        && actual_session != expected_session
    {
        errors.push(format!(
            "subject.session_id {:?} does not match expected {:?}",
            actual_session, expected_session
        ));
    }
    if let Some(expected_thread) = trim_non_empty(expectations.thread_id)
        && let Some(actual_thread) = trim_non_empty(harness.subject.thread_id.as_deref())
        && actual_thread != expected_thread
    {
        errors.push(format!(
            "subject.thread_id {:?} does not match expected {:?}",
            actual_thread, expected_thread
        ));
    }
    if harness.roles.is_empty() {
        errors.push("roles must not be empty".to_string());
    }
    if harness.stages.is_empty() {
        errors.push("stages must not be empty".to_string());
    }
    if !harness.contracts.requires_artifact_backed_completion
        && harness.contracts.required_outputs.is_empty()
        && harness.contracts.completion_gates.is_empty()
    {
        errors.push(
            "contracts must declare required_outputs or completion_gates when artifact-backed completion is disabled"
                .to_string(),
        );
    }
    if harness.state.state_root.trim().is_empty() {
        errors.push("state.state_root is empty".to_string());
    }
    if !harness.state.path_addressable {
        errors.push("state.path_addressable must be true".to_string());
    }
    if !harness.state.compaction_stable {
        errors.push("state.compaction_stable must be true".to_string());
    }
    if harness.failure_taxonomy.is_empty() {
        errors.push("failure_taxonomy must not be empty".to_string());
    }

    let mut role_ids = HashSet::new();
    for role in &harness.roles {
        if role.role_id.trim().is_empty() {
            errors.push("role_id must not be empty".to_string());
        } else if !role_ids.insert(role.role_id.clone()) {
            errors.push(format!("duplicate role_id {:?}", role.role_id));
        }
        if role.prompt_purpose.trim().is_empty() {
            errors.push(format!("role {:?} prompt_purpose is empty", role.role_id));
        }
        if role.responsibilities.is_empty() {
            errors.push(format!(
                "role {:?} must declare responsibilities",
                role.role_id
            ));
        }
    }

    let mut stage_ids = HashSet::new();
    for stage in &harness.stages {
        if stage.stage_id.trim().is_empty() {
            errors.push("stage_id must not be empty".to_string());
        } else if !stage_ids.insert(stage.stage_id.clone()) {
            errors.push(format!("duplicate stage_id {:?}", stage.stage_id));
        }
        if !role_ids.contains(&stage.role_id) {
            errors.push(format!(
                "stage {:?} references unknown role_id {:?}",
                stage.stage_id, stage.role_id
            ));
        }
        if stage.summary.trim().is_empty() {
            errors.push(format!("stage {:?} summary is empty", stage.stage_id));
        }
    }

    let gate_ids: HashSet<String> = harness
        .contracts
        .completion_gates
        .iter()
        .map(|g| g.gate_id.clone())
        .collect();
    for stage in &harness.stages {
        for gate_id in stage.completion_gate_ids.as_deref().unwrap_or_default() {
            if !gate_ids.contains(gate_id) {
                errors.push(format!(
                    "stage {:?} references unknown completion gate {:?}",
                    stage.stage_id, gate_id
                ));
            }
        }
    }

    let failure_codes: HashSet<String> = harness
        .failure_taxonomy
        .iter()
        .map(|f| f.code.clone())
        .collect();
    for stage in &harness.stages {
        for code in stage.failure_mode_codes.as_deref().unwrap_or_default() {
            if !failure_codes.contains(code) {
                errors.push(format!(
                    "stage {:?} references unknown failure_mode_code {:?}",
                    stage.stage_id, code
                ));
            }
        }
    }
    for failure in &harness.failure_taxonomy {
        if failure.code.trim().is_empty() {
            errors.push("failure_taxonomy.code must not be empty".to_string());
        }
        if failure.description.trim().is_empty() {
            errors.push(format!(
                "failure_taxonomy {:?} description is empty",
                failure.code
            ));
        }
        if let Some(ref stage_id) = failure.recovery_stage_id
            && !stage_ids.contains(stage_id)
        {
            errors.push(format!(
                "failure_taxonomy {:?} references unknown recovery_stage_id {:?}",
                failure.code, stage_id
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_contract_first_is_valid() {
        let harness = AgentHarnessSpec::minimal_contract_first(
            "repo-harness",
            "Implement the contract-first runtime path",
            Some("sid-harness"),
            Some("thread-7"),
            &["artifacts/out.json".to_string()],
        );
        let res = validate_agent_harness_ingest(
            &harness,
            HarnessIngestExpectations {
                repository_id: "repo-harness",
                session_id: Some("sid-harness"),
                thread_id: Some("thread-7"),
            },
        );
        assert!(res.is_ok(), "{res:?}");
    }

    #[test]
    fn defaults_fill_missing_subject_continuity() {
        let mut harness =
            AgentHarnessSpec::minimal_contract_first("repo-fill", "Fill defaults", None, None, &[]);
        harness.subject.repository_id.clear();
        apply_harness_subject_defaults(
            &mut harness,
            HarnessIngestExpectations {
                repository_id: "repo-fill",
                session_id: Some("sid-fill"),
                thread_id: Some("thread-fill"),
            },
        );
        assert_eq!(harness.subject.repository_id, "repo-fill");
        assert_eq!(harness.subject.session_id.as_deref(), Some("sid-fill"));
        assert_eq!(harness.subject.thread_id.as_deref(), Some("thread-fill"));
    }
}
