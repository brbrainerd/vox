use crate::params::ToolResult;
use crate::server_state::ServerState;
use schemars::JsonSchema;
use serde::Deserialize;

use super::common::{
    REM_PUBLICATION_ID, REM_SCIENTIA_ATTEMPTS, REM_SCIENTIA_DB, REM_SCIENTIA_METADATA,
    REM_SCIENTIA_PUBLISH, REM_SCIENTIA_SIMULATE, mcp_social_worthiness_enforce,
    mcp_social_worthiness_score_min, no_voxdb_json_envelope, no_voxdb_syndication,
    no_voxdb_tool_string, operator_publisher_config, unified_news_item_from_manifest_row,
    worthiness_score_for_row,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationRouteSimulateParams {
    pub publication_id: String,
}

pub async fn vox_scientia_publication_route_simulate(
    state: &ServerState,
    params: VoxScientiaPublicationRouteSimulateParams,
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
    let item = match unified_news_item_from_manifest_row(&row) {
        Ok(i) => i,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("parse metadata_json: {e}"),
                REM_SCIENTIA_METADATA,
            )
            .to_json();
        }
    };
    let worthiness = worthiness_score_for_row(&row);
    let publisher =
        vox_publisher::Publisher::new(operator_publisher_config(state, true, worthiness));
    match publisher.publish_all(&item).await {
        Ok(r) => ToolResult::ok(r).to_json(),
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("simulate failed: {e}"),
            REM_SCIENTIA_SIMULATE,
        )
        .to_json(),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationPublishParams {
    pub publication_id: String,
    #[serde(default)]
    pub channels: Option<Vec<String>>,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub dry_run: bool,
    /// When true, emit compact JSON in the tool text payload (single line).
    #[serde(default)]
    pub json: bool,
}

pub async fn vox_scientia_publication_publish(
    state: &ServerState,
    params: VoxScientiaPublicationPublishParams,
) -> String {
    let compact = params.json;
    let Some(db) = &state.db else {
        return no_voxdb_syndication(compact);
    };
    let row = match db.get_publication_manifest(&params.publication_id).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json_styled(compact);
        }
    };
    let Some(row) = row else {
        return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json_styled(compact);
    };
    let digest = row.content_sha3_256.clone();
    let mut item = match unified_news_item_from_manifest_row(&row) {
        Ok(i) => i,
        Err(e) => {
            return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
                format!("parse metadata_json: {e}"),
                REM_SCIENTIA_METADATA,
            )
            .to_json_styled(compact);
        }
    };
    if let Some(channels) = params.channels.as_ref() {
        let normalized = vox_publisher::switching::normalize_channels(channels);
        vox_publisher::switching::apply_channel_allowlist(&mut item, normalized.as_slice());
    }
    let dual = match db
        .has_dual_publication_approval_for_digest(&params.publication_id, &digest)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json_styled(compact);
        }
    };
    let gate = vox_publisher::gate::evaluate_publish_gate(
        vox_publisher::gate::publish_gate_inputs_for_mcp(
            params.dry_run,
            state.orchestrator_config.news.dry_run,
            state.orchestrator_config.news.publish_armed,
            true,
            dual,
            &item,
        ),
    );
    if gate.has_blockers() {
        let msg = serde_json::json!({
            "error": "live publish blocked by gate",
            "blocking_reasons": gate.blocking_reasons,
        })
        .to_string();
        return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
            msg,
            REM_SCIENTIA_SIMULATE,
        )
        .to_json_styled(compact);
    }
    let worthiness = worthiness_score_for_row(&row);
    if mcp_social_worthiness_enforce(state)
        && !params.dry_run
        && !state.orchestrator_config.news.dry_run
        && !item.syndication.dry_run
        && gate.live_publish_allowed
        && let Some(score) = worthiness
    {
        let floor = mcp_social_worthiness_score_min(state);
        if score < floor {
            let msg = serde_json::json!({
                "error": "live publish blocked by worthiness floor",
                "worthiness_score": score,
                "floor": floor,
            })
            .to_string();
            return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
                msg,
                REM_SCIENTIA_PUBLISH,
            )
            .to_json_styled(compact);
        }
    }
    let publisher =
        vox_publisher::Publisher::new(operator_publisher_config(state, params.dry_run, worthiness));
    let out = match publisher.publish_all(&item).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<vox_publisher::SyndicationResult>::err_with_remediation(
                format!("publish failed: {e}"),
                REM_SCIENTIA_PUBLISH,
            )
            .to_json_styled(compact);
        }
    };
    if let Ok(out_json) = serde_json::to_string(&out) {
        let _ = db
            .record_publication_attempt(
                &params.publication_id,
                &digest,
                "manual_mcp",
                out_json.as_str(),
            )
            .await;
    }
    if gate.live_publish_allowed {
        if out.all_enabled_channels_succeeded(&item) {
            let _ = db
                .set_publication_state(
                    &params.publication_id,
                    "published",
                    Some(&serde_json::json!({ "channel_group": "manual_mcp" }).to_string()),
                )
                .await;
        } else if out.has_failures() {
            let _ = db
                .set_publication_state(
                    &params.publication_id,
                    "publish_failed",
                    Some(&serde_json::json!({ "channel_group": "manual_mcp" }).to_string()),
                )
                .await;
        }
    }
    ToolResult::ok(out).to_json_styled(compact)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaPublicationRetryFailedParams {
    pub publication_id: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default = "vox_config::serde_defaults::default_true")]
    pub dry_run: bool,
    /// When true, emit compact JSON (including nested publish responses).
    #[serde(default)]
    pub json: bool,
}

pub async fn vox_scientia_publication_retry_failed(
    state: &ServerState,
    params: VoxScientiaPublicationRetryFailedParams,
) -> String {
    let compact = params.json;
    let Some(db) = &state.db else {
        return no_voxdb_json_envelope(compact);
    };
    let row = match db.get_publication_manifest(&params.publication_id).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json_styled(compact);
        }
    };
    let Some(row) = row else {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json_styled(compact);
    };
    let digest = row.content_sha3_256;
    let attempts = match db.list_publication_attempts(&params.publication_id).await {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json_styled(compact);
        }
    };
    if attempts.is_empty() {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "no attempts found".to_string(),
            REM_SCIENTIA_ATTEMPTS,
        )
        .to_json_styled(compact);
    }
    let attempt_refs: Vec<vox_publisher::switching::AttemptOutcome<'_>> = attempts
        .iter()
        .map(|a| vox_publisher::switching::AttemptOutcome {
            content_sha3_256: a.content_sha3_256.as_str(),
            outcome_json: a.outcome_json.as_str(),
        })
        .collect();
    let explicit = params
        .channel
        .as_ref()
        .map(|c| vox_publisher::switching::parse_channels_csv(c.trim()));
    let plan = match vox_publisher::switching::plan_publication_retry_channels(
        attempt_refs.as_slice(),
        digest.as_str(),
        explicit.as_deref(),
    ) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                "no syndication attempt outcome for current manifest digest".to_string(),
                REM_SCIENTIA_ATTEMPTS,
            )
            .to_json_styled(compact);
        }
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("attempt parse: {e}"),
                REM_SCIENTIA_METADATA,
            )
            .to_json_styled(compact);
        }
    };

    if !plan.skipped_success_channels.is_empty() && plan.will_retry_channels.is_empty() {
        return ToolResult::ok(serde_json::json!({
            "publication_id": params.publication_id,
            "retried": false,
            "reason": "channels_already_succeeded_for_digest",
            "skipped_success_channels": plan.skipped_success_channels,
            "blocked_channels": plan.blocked_channels,
        }))
        .to_json_styled(compact);
    }

    if plan.will_retry_channels.is_empty() {
        return ToolResult::ok(serde_json::json!({
            "publication_id": params.publication_id,
            "retried": false,
            "reason": if params.channel.is_some() {
                "no_channels_eligible_for_retry"
            } else {
                "no_failed_channels"
            },
            "skipped_success_channels": plan.skipped_success_channels,
            "blocked_channels": plan.blocked_channels,
        }))
        .to_json_styled(compact);
    }

    vox_scientia_publication_publish(
        state,
        VoxScientiaPublicationPublishParams {
            publication_id: params.publication_id,
            channels: Some(plan.will_retry_channels),
            dry_run: params.dry_run,
            json: params.json,
        },
    )
    .await
}
