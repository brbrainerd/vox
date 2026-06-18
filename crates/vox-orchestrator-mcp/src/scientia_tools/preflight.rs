use crate::params::ToolResult;
use crate::server_state::ServerState;
use schemars::JsonSchema;
use serde::Deserialize;
use vox_publisher::publication_preflight::PreflightProfile;

use super::common::{
    REM_PUBLICATION_ID, REM_SCIENTIA_DB, REM_SCIENTIA_METADATA, REM_WORTHINESS_CONTRACT,
    no_voxdb_tool_string, publication_manifest_from_row,
};
use super::lifecycle::PreflightProfileParam;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationPreflightParams {
    pub publication_id: String,
    #[serde(default)]
    pub profile: Option<PreflightProfileParam>,
    /// When true, attach [`vox_publisher::publication_worthiness::WorthinessEvaluation`] (`contracts/scientia/publication-worthiness.default.yaml` from repo root).
    #[serde(default)]
    pub with_worthiness: bool,
}

pub async fn vox_scientia_publication_preflight(
    state: &ServerState,
    params: VoxScientiaPublicationPreflightParams,
) -> String {
    let Some(db) = &state.db else {
        return no_voxdb_tool_string();
    };
    let row = match db.get_publication_manifest(&params.publication_id).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let Some(row) = row else {
        return ToolResult::<String>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json();
    };
    let mut manifest = publication_manifest_from_row(&row);
    let profile: PreflightProfile = params.profile.unwrap_or_default().into();
    let item = match vox_publisher::switching::unified_news_item_from_manifest_parts(
        row.publication_id.as_str(),
        row.title.as_str(),
        row.author.as_str(),
        row.body_markdown.as_str(),
        row.metadata_json.as_deref(),
    ) {
        Ok(i) => i,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("parse metadata_json for gate: {e}"),
                REM_SCIENTIA_METADATA,
            )
            .to_json();
        }
    };
    let dual = match db
        .has_dual_publication_approval_for_digest(
            &params.publication_id,
            row.content_sha3_256.as_str(),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let gate = vox_publisher::gate::evaluate_publish_gate(
        vox_publisher::gate::publish_gate_inputs_for_mcp(
            false,
            state.orchestrator_config.news.dry_run,
            state.orchestrator_config.news.publish_armed,
            true,
            dual,
            &item,
        ),
    );
    let attention =
        vox_publisher::publication_preflight::PreflightAttentionInputs { gate: Some(gate) };
    let report = if params.with_worthiness {
        let rid = manifest
            .metadata_json
            .as_deref()
            .and_then(|raw| {
                let v: serde_json::Value = serde_json::from_str(raw).ok()?;
                v.get("repository_id")
                    .and_then(|x| x.as_str())
                    .map(std::string::ToString::to_string)
            })
            .unwrap_or_else(|| state.repository.repository_id.clone());
        match db
            .merge_scientia_live_socrates_into_metadata_json(
                manifest.metadata_json.as_deref(),
                rid.as_str(),
            )
            .await
        {
            Ok(s) => manifest.metadata_json = Some(s),
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("socrates telemetry merge: {e}"),
                    REM_SCIENTIA_DB,
                )
                .to_json();
            }
        }
        match vox_publisher::scientia_evidence::enrich_metadata_json_with_repo_files(
            manifest.metadata_json.as_deref(),
            &state.repository.root,
        ) {
            Ok(Some(updated)) => manifest.metadata_json = Some(updated),
            Ok(None) => {}
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("scientia_evidence file hydration: {e}"),
                    REM_SCIENTIA_METADATA,
                )
                .to_json();
            }
        }
        let path = state
            .repository
            .root
            .join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH);
        let yaml = match vox_bounded_fs::read_utf8_path_capped(&path) {
            Ok(s) => s,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("read worthiness contract {}: {e}", path.display()),
                    REM_WORTHINESS_CONTRACT,
                )
                .to_json();
            }
        };
        let contract = match vox_publisher::publication_worthiness::load_contract_from_str(&yaml) {
            Ok(c) => c,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("parse worthiness contract: {e}"),
                    REM_WORTHINESS_CONTRACT,
                )
                .to_json();
            }
        };
        if let Err(e) =
            vox_publisher::publication_worthiness::validate_contract_invariants(&contract)
        {
            return ToolResult::<String>::err_with_remediation(
                format!("worthiness contract invariants: {e}"),
                REM_WORTHINESS_CONTRACT,
            )
            .to_json();
        }
        let scientia_h =
            vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(
                &state.repository.root,
            );
        vox_publisher::publication_preflight::run_preflight_with_worthiness_attention_heuristics(
            &manifest,
            profile,
            &contract,
            Some(attention),
            &scientia_h,
            None,
        )
    } else {
        vox_publisher::publication_preflight::run_preflight_with_attention(
            &manifest,
            profile,
            Some(attention),
        )
    };
    let operator_status_surface_v1 =
        vox_publisher::publication_preflight::operator_status_surface_v1(
            row.publication_id.as_str(),
            profile,
            &report,
        );
    ToolResult::ok(serde_json::json!({
        "preflight_report": report,
        "operator_status_surface_v1": operator_status_surface_v1
    }))
    .to_json()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaWorthinessEvaluateParams {
    /// Repo-relative contract YAML (defaults to `contracts/scientia/publication-worthiness.default.yaml`).
    #[serde(default)]
    pub contract_yaml_relative: Option<String>,
    /// When true and [`ServerState::db`] is set, attach [`vox_db::VoxDb::summarize_trust_rollups`] slices for the workspace repository.
    #[serde(default)]
    pub with_live_trust: Option<bool>,
    /// [`vox_publisher::publication_worthiness::WorthinessInputs`] as a JSON object.
    pub metrics: serde_json::Value,
}

/// Local-only worthiness gate: load contract from the discovered repository root; no DB writes.
pub async fn vox_scientia_worthiness_evaluate(
    state: &ServerState,
    params: VoxScientiaWorthinessEvaluateParams,
) -> String {
    let root = &state.repository.root;
    let contract_path = match params.contract_yaml_relative {
        Some(rel) if !rel.trim().is_empty() => root.join(rel.trim()),
        _ => root.join(vox_publisher::publication_worthiness::DEFAULT_CONTRACT_REL_PATH),
    };
    let yaml = match vox_bounded_fs::read_utf8_path_capped(&contract_path) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("read contract {}: {e}", contract_path.display()),
                REM_WORTHINESS_CONTRACT,
            )
            .to_json();
        }
    };
    let contract = match vox_publisher::publication_worthiness::load_contract_from_str(&yaml) {
        Ok(c) => c,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("parse contract YAML: {e}"),
                REM_WORTHINESS_CONTRACT,
            )
            .to_json();
        }
    };
    if let Err(e) = vox_publisher::publication_worthiness::validate_contract_invariants(&contract) {
        return ToolResult::<String>::err_with_remediation(
            format!("contract invariants: {e}"),
            REM_WORTHINESS_CONTRACT,
        )
        .to_json();
    }
    let inputs: vox_publisher::publication_worthiness::WorthinessInputs =
        match serde_json::from_value(params.metrics) {
            Ok(i) => i,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("metrics: {e}"),
                    "Pass `metrics` as a JSON object matching `WorthinessInputs` (see publication_worthiness docs).",
                )
                .to_json();
            }
        };
    let out = vox_publisher::publication_worthiness::evaluate_worthiness(&contract, &inputs);
    if params.with_live_trust != Some(true) {
        return ToolResult::ok(out).to_json();
    }
    let Some(db) = &state.db else {
        let mut v = match serde_json::to_value(&out) {
            Ok(v) => v,
            Err(e) => {
                return ToolResult::<serde_json::Value>::err_with_remediation(
                    format!("serialize evaluation: {e}"),
                    REM_WORTHINESS_CONTRACT,
                )
                .to_json();
            }
        };
        if let serde_json::Value::Object(ref mut m) = v {
            m.insert(
                "live_trust_note".to_string(),
                serde_json::Value::String(
                    "with_live_trust requested but VoxDb is not connected.".into(),
                ),
            );
        }
        return ToolResult::ok(v).to_json();
    };
    let repo = state.repository.repository_id.as_str();
    let mut v = match serde_json::to_value(&out) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("serialize evaluation: {e}"),
                REM_WORTHINESS_CONTRACT,
            )
            .to_json();
        }
    };
    if let serde_json::Value::Object(ref mut m) = v {
        let mut live = serde_json::json!({});
        if let Ok(rows) = db
            .summarize_trust_rollups(None, None, None, Some(repo), "dimension", 32)
            .await
        {
            live["by_dimension"] =
                serde_json::to_value(&rows).unwrap_or_else(|_| serde_json::json!([]));
        }
        if let Ok(rows) = db
            .summarize_trust_rollups(None, None, None, Some(repo), "dimension_domain", 32)
            .await
        {
            live["by_dimension_domain"] =
                serde_json::to_value(&rows).unwrap_or_else(|_| serde_json::json!([]));
        }
        if let Ok(rows) = db
            .summarize_trust_rollups(None, None, None, Some(repo), "entity_dimension", 32)
            .await
        {
            live["by_entity_dimension"] =
                serde_json::to_value(&rows).unwrap_or_else(|_| serde_json::json!([]));
        }
        m.insert("live_trust_rollups".to_string(), live);
    }
    ToolResult::ok(v).to_json()
}

/// Preferred Rust alias (same JSON shape as [`VoxScientiaPublicationPreflightParams`]).
pub type ScientiaPublicationPreflightParams = VoxScientiaPublicationPreflightParams;
/// Preferred Rust alias (same JSON shape as [`VoxScientiaWorthinessEvaluateParams`]).
pub type ScientiaWorthinessEvaluateParams = VoxScientiaWorthinessEvaluateParams;
