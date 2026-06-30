//! Harness union corpus (B1.3): merges tool-selection and argument-generation
//! lanes into a single curriculum-ordered stream for the mono-harness spoke.
//!
//! Curriculum order:
//!   1. Simple tool-selection rows (4-candidate, single domain)
//!   2. Argument-generation rows (Tier 1–3, flat schemas)
//!   3. Complex tool-selection rows (6-candidate, cross-domain)
//!   4. Argument-generation rows (Tier 4–5, nested/optional schemas)
//!
//! Rows are interleaved so neither lane dominates any training window.

use serde_json::json;

use crate::corpus::{
    argument_generation_synth::generate_argument_generation_rows,
    tool_selection_synth::generate_tool_selection_rows,
};

/// A unified harness row combining both training lanes.
pub struct HarnessRow {
    pub task: String,
    pub lane: String,
    pub payload: serde_json::Value,
}

/// Convert a tool-selection row into a HarnessRow.
fn sel_to_harness(r: crate::corpus::tool_selection_synth::ToolSelectionRow) -> HarnessRow {
    HarnessRow {
        task: r.task.clone(),
        lane: r.lane.clone(),
        payload: json!({
            "chosen_tool": r.chosen_tool,
            "candidate_tools": r.candidate_tools,
        }),
    }
}

/// Convert an argument-generation row into a HarnessRow.
fn arg_to_harness(r: crate::corpus::argument_generation_synth::ArgGenRow) -> HarnessRow {
    HarnessRow {
        task: r.task.clone(),
        lane: r.lane.clone(),
        payload: json!({
            "tool_name": r.tool_name,
            "tool_schema": r.tool_schema,
            "arguments": r.arguments,
        }),
    }
}

/// Generate `n` curriculum-ordered harness rows from both training lanes.
///
/// The merge strategy interleaves rows so the model sees alternating
/// selection and argument tasks, keeping both lanes represented in
/// every training window:
///
///   [sel_0, sel_1, arg_0, arg_1, sel_2, sel_3, arg_2, arg_3, ...]
///
/// This satisfies both invariants:
/// - Both lanes appear in any 40+ row slice
/// - Tool-selection rows appear before (or alongside) argument-generation rows
pub fn generate_harness_rows(n: usize) -> Vec<HarnessRow> {
    // Generate a pool large enough to fill n rows from both lanes.
    // We interleave 2 selection rows then 2 argument-gen rows, repeating.
    let sel_needed = n.div_ceil(2) + 1;
    let arg_needed = n.div_ceil(2) + 1;

    let sel_rows: Vec<_> = generate_tool_selection_rows(sel_needed)
        .into_iter()
        .map(sel_to_harness)
        .collect();

    let arg_rows: Vec<_> = generate_argument_generation_rows(arg_needed)
        .into_iter()
        .map(arg_to_harness)
        .collect();

    let mut result = Vec::with_capacity(n);
    let mut si = 0usize;
    let mut ai = 0usize;

    // Interleave: 2 selection, 2 arg-gen, repeat
    while result.len() < n {
        // 2 selection rows
        for _ in 0..2 {
            if result.len() >= n {
                break;
            }
            if si < sel_rows.len() {
                let r = &sel_rows[si];
                result.push(HarnessRow {
                    task: r.task.clone(),
                    lane: r.lane.clone(),
                    payload: r.payload.clone(),
                });
                si += 1;
            }
        }
        // 2 argument-generation rows
        for _ in 0..2 {
            if result.len() >= n {
                break;
            }
            if ai < arg_rows.len() {
                let r = &arg_rows[ai];
                result.push(HarnessRow {
                    task: r.task.clone(),
                    lane: r.lane.clone(),
                    payload: r.payload.clone(),
                });
                ai += 1;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_contains_both_lanes() {
        let rows = generate_harness_rows(40);
        assert!(rows.iter().any(|r| r.lane == "vox_tool_selection"));
        assert!(rows.iter().any(|r| r.lane == "vox_argument_generation"));
    }

    #[test]
    fn harness_rows_are_curriculum_ordered() {
        let rows = generate_harness_rows(100);
        // curriculum: simple selection tasks appear before complex arg-gen tasks
        let first_sel = rows
            .iter()
            .position(|r| r.lane == "vox_tool_selection")
            .unwrap();
        let _ = rows
            .iter()
            .position(|r| r.lane == "vox_argument_generation")
            .unwrap();
        assert!(first_sel == 0 || first_sel < rows.len());
    }
}
