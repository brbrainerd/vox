//! Synthetic training-pair generator for Vox MCP tools, A2A messaging, skills,
//! orchestrator commands, durable workflows, and agent constructs.
//!
//! All data is sourced from compile-time Rust constants — never from external files
//! or hardcoded magic values. Adding a new MCP tool to the registry automatically
//! generates training pairs for it on the next `vox corpus generate` run.
//!
//! ## Usage
//! ```no_run
//! use vox_corpus::synthetic_gen::{SyntheticGenConfig, generate_all};
//! let cfg = SyntheticGenConfig::default();
//! generate_all(&cfg, std::path::Path::new("mens/data/synthetic.jsonl")).unwrap();
//! ```

// ─── Compile-time source tables ──────────────────────────────────────────────

pub use vox_mcp_registry::{ORCHESTRATOR_TOOLS, SKILL_TOOLS};

include!(concat!(env!("OUT_DIR"), "/dynamic_registry.rs"));

pub(crate) fn example_agent_pairs() -> Vec<(String, String)> {
    vec![
        ("agent_1".into(), "agent_2".into()),
        ("compiler_agent".into(), "review_agent".into()),
        ("orchestrator".into(), "worker_1".into()),
        ("planner".into(), "executor".into()),
        ("frontend_agent".into(), "backend_agent".into()),
    ]
}

/// Dynamically generated example tasks from the live orchestrator registry.
pub(crate) fn example_tasks() -> Vec<String> {
    vox_mcp_registry::TOOL_REGISTRY
        .iter()
        .filter(|t| vox_mcp_registry::ORCHESTRATOR_TOOLS.contains(&t.name))
        .map(|t| {
            format!(
                "use the {} tool to {}",
                t.name,
                t.description.to_lowercase()
            )
        })
        .collect()
}

// ─── Config ───────────────────────────────────────────────────────────────────

/// Configuration for the synthetic corpus generator.
#[derive(Debug, Clone)]
pub struct SyntheticGenConfig {
    /// Minimum phrasings per tool call (diversity floor).
    pub min_phrasings_per_tool: usize,
    /// Minimum A2A pairs per message type.
    pub min_pairs_per_a2a_type: usize,
    /// RNG seed. `0` uses deterministic seeding from tool name hash.
    pub seed: u64,
    /// Whether to emit tool_trace format rows.
    pub emit_tool_traces: bool,
    /// Whether to emit A2A trace rows.
    pub emit_a2a_traces: bool,
    /// Whether to emit workflow trace rows.
    pub emit_workflow_traces: bool,
    /// Whether to emit orchestrator command rows.
    pub emit_orchestrator_rows: bool,
    /// Whether to emit skill management rows.
    pub emit_skill_rows: bool,
    /// Whether to emit agent documentation / agent usage rows.
    pub emit_agent_rows: bool,
    /// Whether to emit CLI command usage rows.
    pub emit_cli_rows: bool,
    /// Whether to emit shell script generation rows.
    pub emit_script_rows: bool,
    /// Whether to emit organic Vox code generation pairs.
    pub emit_organic_vox: bool,
    /// Whether to emit Chain-of-Thought (CoT) organic Vox code generation pairs (Reasoning-First).
    pub emit_cot_organic_vox: bool,
    /// Whether to run the augmentation engine (typos, synonyms, case) after generation.
    /// This 3× multiplies effective corpus size with robust variants.
    pub augment_after_generate: bool,
    /// Whether to emit routing decision pairs (Gap 1: orchestrator-as-model training).
    pub emit_routing_decisions: bool,
    /// Whether to emit expanded negative preference pairs (Gap 2).
    pub emit_negative_expanded: bool,
    /// Whether to emit error→recovery pairs (Gap 6).
    pub emit_error_recovery: bool,
    /// Whether to emit multi-agent conversation pairs (Gap 8).
    pub emit_multi_agent_convos: bool,
    /// Whether to emit telemetry interpretation pairs (Gap 10).
    pub emit_telemetry_pairs: bool,
    /// Whether to emit KCH Anti-conflation negative DPO pairs (Wave 4).
    pub emit_kch_anticonflation: bool,
}

impl Default for SyntheticGenConfig {
    fn default() -> Self {
        Self {
            min_phrasings_per_tool: 6,
            min_pairs_per_a2a_type: 4,
            seed: 0,
            emit_tool_traces: true,
            emit_a2a_traces: true,
            emit_workflow_traces: true,
            emit_orchestrator_rows: true,
            emit_skill_rows: true,
            emit_agent_rows: true,
            emit_cli_rows: true,
            emit_script_rows: true,
            emit_organic_vox: true,
            emit_cot_organic_vox: true,
            augment_after_generate: true,
            emit_routing_decisions: true,
            emit_negative_expanded: true,
            emit_error_recovery: true,
            emit_multi_agent_convos: true,
            emit_telemetry_pairs: true,
            emit_kch_anticonflation: true,
        }
    }
}

// ─── Shared JSONL emitters (used by submodule `include!` bodies) ───────────────

use std::io::Write;
use std::path::Path;

use anyhow::Context;
use serde_json::{Value, json};

pub(crate) fn emit_line(
    out: &mut impl Write,
    prompt: &str,
    response: &Value,
    category: &str,
    record_type: &str,
) -> anyhow::Result<()> {
    let difficulty = crate::training::construct_difficulty(category, record_type);
    let resp_str = response.to_string();
    let row = json!({
        "prompt": prompt,
        "response": resp_str,
        "messages": [
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": resp_str}
        ],
        "category": category,
        "record_type": record_type,
        "schema_version": "vox_dogfood_v1",
        "difficulty": difficulty,
    });
    writeln!(out, "{}", serde_json::to_string(&row)?)?;
    Ok(())
}

pub(crate) fn emit_tool_pair(
    out: &mut impl Write,
    tool_name: &str,
    description: &str,
    prompt: &str,
    args: Value,
    category: &str,
    record_type: &str,
) -> anyhow::Result<()> {
    let response = json!({
        "tool": tool_name,
        "description": description,
        "arguments": args,
    });
    emit_line(out, prompt, &response, category, record_type)
}

pub(crate) mod rng;
pub(crate) use rng::name_hash;
mod a2a_pairs;
mod agent_pairs;
mod cli_pairs;
mod error_recovery_pairs;
pub mod kch_anticonflation;
mod multi_agent_pairs;
mod negative_pairs;
mod orchestrator_pairs;
mod routing_pairs;
mod script_pairs;
mod telemetry_pairs;
mod templates;
pub(crate) mod tool_pairs;
pub mod transplant_pairs;
mod web_pairs;
mod workflow_pairs;

pub use agent_pairs::generate_agent_lifecycle_pairs;
pub use error_recovery_pairs::generate_error_recovery_pairs;
pub use multi_agent_pairs::generate_multi_agent_conversation_pairs;
pub use routing_pairs::{
    generate_negative_preference_pairs_expanded, generate_routing_decision_pairs,
    generate_tool_chain_pairs,
};
pub use telemetry_pairs::generate_telemetry_interpretation_pairs;

include!("bodies/_generate_all_mod.inc");

#[cfg(test)]
mod tests;
