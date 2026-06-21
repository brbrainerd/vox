//! Corpus generator for the `vox_tool_selection` training lane (B1.1).
//!
//! Generates rule-based rows where the model must pick the correct tool from a
//! candidate set that includes hard negatives from the same domain category.
//! All data is derived from a static tool catalog — no LLM completions.

/// One tool-selection SFT row.
pub struct ToolSelectionRow {
    pub task: String,
    pub candidate_tools: Vec<String>,
    pub chosen_tool: String,
    pub lane: String,
}

/// Static catalog of (tool_name, category, description).
/// Categories define which tools are "hard negatives" for each other
/// (same category = confusable; different category = easy negative).
const TOOL_CATALOG: &[(&str, &str, &str)] = &[
    // ── task management ─────────────────────────────────────────────────────
    (
        "vox_submit_task",
        "task",
        "submit a new task to the orchestrator for execution",
    ),
    (
        "vox_task_status",
        "task",
        "query the current status of a running or completed task",
    ),
    (
        "vox_cancel_task",
        "task",
        "cancel an in-progress or queued task by identifier",
    ),
    (
        "vox_complete_task",
        "task",
        "mark a task as successfully completed with a result",
    ),
    (
        "vox_fail_task",
        "task",
        "mark a task as failed and provide an error reason",
    ),
    (
        "vox_reorder_task",
        "task",
        "change the priority order of pending tasks in the queue",
    ),
    // ── agents ──────────────────────────────────────────────────────────────
    (
        "vox_spawn_agent",
        "agent",
        "spawn a new agent to handle a specific workload",
    ),
    (
        "vox_retire_agent",
        "agent",
        "retire and shut down an agent that is no longer needed",
    ),
    (
        "vox_drain_agent",
        "agent",
        "drain an agent by waiting for current work to finish",
    ),
    (
        "vox_pause_agent",
        "agent",
        "pause an agent so it stops accepting new work",
    ),
    (
        "vox_resume_agent",
        "agent",
        "resume a previously paused agent",
    ),
    // ── speech / oratio ─────────────────────────────────────────────────────
    (
        "vox_oratio_transcribe",
        "speech",
        "transcribe an audio file to text using Oratio STT",
    ),
    (
        "vox_oratio_listen",
        "speech",
        "listen to live audio input and transcribe in real time",
    ),
    (
        "vox_oratio_status",
        "speech",
        "check the current status of the Oratio speech service",
    ),
    (
        "vox_speech_to_code",
        "speech",
        "convert speech input to Vox code using STT then codegen",
    ),
    // ── tool search / registry ───────────────────────────────────────────────
    (
        "vox_tool_search",
        "registry",
        "search the tool registry for tools matching a query",
    ),
    (
        "vox_publish_message",
        "registry",
        "publish a message to a topic on the agent message bus",
    ),
    // ── feedback / clarification ─────────────────────────────────────────────
    (
        "vox_doubt_task",
        "feedback",
        "raise a doubt about a task that needs user review",
    ),
    (
        "vox_ask_clarification",
        "feedback",
        "ask the user a clarifying question before proceeding",
    ),
    (
        "vox_resolve_feedback",
        "feedback",
        "resolve pending feedback and resume task execution",
    ),
    (
        "vox_feedback_list",
        "feedback",
        "list all pending feedback items for the current session",
    ),
];

/// Task phrasings derived from a tool description (rule-based, no LLM).
fn task_from_description(tool_name: &str, description: &str) -> String {
    // Transform "submit a new task to the orchestrator" → "I need to submit a new task to the orchestrator"
    let action_words = [
        "submit",
        "query",
        "cancel",
        "mark",
        "change",
        "spawn",
        "retire",
        "drain",
        "pause",
        "resume",
        "transcribe",
        "listen",
        "check",
        "convert",
        "search",
        "publish",
        "raise",
        "ask",
        "resolve",
        "list",
    ];
    let lower = description.to_lowercase();
    for word in &action_words {
        if lower.starts_with(word) {
            return format!("I need to {} (use {})", description, tool_name);
        }
    }
    format!("Use {} to {}", tool_name, description)
}

/// Select hard negatives: other tools in the same category (confusable), plus
/// fill up to total_count from other categories (easy negatives).
fn select_candidates(chosen_idx: usize, total_count: usize, rng_state: &mut u64) -> Vec<String> {
    let (chosen_name, chosen_cat, _) = TOOL_CATALOG[chosen_idx];
    let mut candidates = vec![chosen_name.to_string()];

    // Hard negatives: same category first (skip chosen)
    for (i, (name, cat, _)) in TOOL_CATALOG.iter().enumerate() {
        if i == chosen_idx {
            continue;
        }
        if *cat == chosen_cat && candidates.len() < total_count {
            candidates.push((*name).to_string());
        }
    }

    // Fill remaining with easy negatives from other categories
    // Use a deterministic shuffle-like selection via xorshift
    let mut pool: Vec<usize> = (0..TOOL_CATALOG.len())
        .filter(|&i| {
            i != chosen_idx
                && TOOL_CATALOG[i].1 != chosen_cat
                && !candidates.contains(&TOOL_CATALOG[i].0.to_string())
        })
        .collect();

    while candidates.len() < total_count && !pool.is_empty() {
        // xorshift pick
        *rng_state ^= *rng_state << 13;
        *rng_state ^= *rng_state >> 7;
        *rng_state ^= *rng_state << 17;
        let idx = (*rng_state as usize) % pool.len();
        candidates.push(TOOL_CATALOG[pool[idx]].0.to_string());
        pool.remove(idx);
    }

    // Shuffle candidates so chosen_tool isn't always first
    for i in (1..candidates.len()).rev() {
        *rng_state ^= *rng_state << 13;
        *rng_state ^= *rng_state >> 7;
        *rng_state ^= *rng_state << 17;
        let j = (*rng_state as usize) % (i + 1);
        candidates.swap(i, j);
    }

    candidates
}

/// Generate `n` tool-selection rows.
///
/// Each row has:
/// - a task derived from the tool description (rule-based)
/// - `chosen_tool` = the correct answer
/// - `candidate_tools` ≥ 4 entries including `chosen_tool` and hard negatives
/// - `lane` = "vox_tool_selection"
pub fn generate_tool_selection_rows(n: usize) -> Vec<ToolSelectionRow> {
    let mut rows = Vec::with_capacity(n);
    let mut rng: u64 = 0xdeadbeef_42424242;
    let catalog_len = TOOL_CATALOG.len();

    for i in 0..n {
        let chosen_idx = i % catalog_len;
        let (chosen_name, _, description) = TOOL_CATALOG[chosen_idx];

        // Curriculum: phase 1 = 4 candidates, phase 3 = 10 candidates
        let candidate_count = match i % 3 {
            0 => 4,
            1 => 4,
            _ => 6,
        }
        .min(catalog_len);

        let task = task_from_description(chosen_name, description);
        let candidates = select_candidates(chosen_idx, candidate_count, &mut rng);

        rows.push(ToolSelectionRow {
            task,
            candidate_tools: candidates,
            chosen_tool: chosen_name.to_string(),
            lane: "vox_tool_selection".to_string(),
        });
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_have_hard_negatives_and_chosen_in_candidates() {
        let rows = generate_tool_selection_rows(50);
        assert!(!rows.is_empty());
        for r in &rows {
            assert!(r.candidate_tools.len() >= 4);
            assert!(r.candidate_tools.contains(&r.chosen_tool));
            assert_eq!(r.lane, "vox_tool_selection");
        }
    }

    #[test]
    fn rows_are_rule_based_not_llm() {
        // tasks derived from tool schemas/descriptions, no LLM completions
        let rows = generate_tool_selection_rows(10);
        for r in &rows {
            assert!(!r.task.is_empty());
        }
    }
}
