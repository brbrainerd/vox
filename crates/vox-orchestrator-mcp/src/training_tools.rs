//! MCP tools: submit Mens / training-style work through the orchestrator (compatibility shim).

use serde::Deserialize;
use serde_json::json;

use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_orchestrator::{TaskCapabilityHints, TaskPriority};

const REM_TRAIN_SUBMIT: &str = "Check orchestrator health, agent queues, and that the Mens training docs match your backend (`docs/src/reference/mens-training.md`).";

/// Arguments for `vox_schola_submit`.
#[derive(Debug, Deserialize)]
pub struct TrainSubmitParams {
    /// Human-readable training goal (stored on the orchestrator task).
    pub description: String,
    /// When set, seeds Socrates / session retrieval from the same context store key as chat.
    #[serde(default)]
    pub session_id: Option<String>,
    /// When true, route toward CUDA-capable agent queues when configured.
    #[serde(default)]
    pub require_cuda: bool,
    /// When true, route toward Metal-capable agent queues when configured.
    #[serde(default)]
    pub require_metal: bool,
    /// Optional minimum VRAM hint (MiB) for routing.
    pub min_vram_mb: Option<u32>,
    /// Optional scheduler pool label for training placement (e.g. `pool=train-gpu`).
    #[serde(default)]
    pub pool_label: Option<String>,
    /// Mark this request as trajectory capture/eval oriented.
    #[serde(default)]
    pub trajectory_capture: bool,
    /// Optional minimum quality score (1-5) expected for trajectory rows.
    #[serde(default)]
    pub min_quality_score: Option<u8>,
    /// Optional tenant identifier for budget attribution.
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// Enqueue a background orchestrator task tagged for training; returns canonical `vox mens train` hint.
pub async fn train_submit(state: &ServerState, params: TrainSubmitParams) -> String {
    let prefer_gpu_compute = params.require_cuda || params.require_metal;
    let caps = TaskCapabilityHints {
        gpu_cuda: params.require_cuda,
        gpu_metal: params.require_metal,
        min_vram_mb: params.min_vram_mb,
        prefer_gpu_compute,
        labels: {
            let mut labels = vec!["workload=mens-train".to_string()];
            if let Some(pool) = params.pool_label.as_deref().map(str::trim)
                && !pool.is_empty()
            {
                labels.push(pool.to_string());
            }
            labels
        },
        ..Default::default()
    };

    let trajectory_note = if params.trajectory_capture {
        format!(
            "trajectory_capture=true quality_floor={}",
            params.min_quality_score.unwrap_or(0)
        )
    } else {
        "trajectory_capture=false".to_string()
    };

    let desc = format!(
        "[Mens train orchestration] {}\ntrajectory: {}\n\nRun locally: `vox mens train --backend qlora --tokenizer hf --device cuda|metal|cpu` (see docs/src/reference/mens-training.md).",
        params.description, trajectory_note
    );

    let orch = &state.orchestrator;
    match orch
        .submit_task_with_agent(
            desc,
            vec![],
            Some(TaskPriority::Background),
            None,
            Some(caps),
            None,
            params.session_id.clone(),
            params.tenant_id.clone(),
        )
        .await
    {
        Ok(task_id) => ToolResult::ok(json!({
            "task_id": task_id.0,
            "hint": "Training execution remains in Mens CLI; this task records intent and routes GPU-capable agents when configured.",
            "canonical_cli": "vox mens train",
            "trajectory_capture": params.trajectory_capture,
            "min_quality_score": params.min_quality_score,
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(format!("{e}"), REM_TRAIN_SUBMIT)
            .to_json(),
    }
}
