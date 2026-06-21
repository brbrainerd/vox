//! Convert captured agent traces (agent_trace_record schema) into SFT rows.
use serde_json::{json, Value};
use std::path::Path;
use std::io::Write;

/// Convert one trace JSON into an SFT row (lane vox_dogfood_agent). Returns
/// None if the trace has no steps (nothing to learn).
pub fn trace_to_sft(trace: &Value) -> Option<Value> {
    let intent = trace.get("intent")?.as_str()?;
    let steps = trace.get("steps")?.as_array()?;
    if steps.is_empty() { return None; }
    let prompt = format!("[vox_agent]\nIntent: {intent}\nEmit the tool-call sequence as JSON.");
    let response = serde_json::to_string(steps).ok()?;
    Some(json!({
        "prompt": prompt,
        "response": response,
        "messages": [
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": response}
        ],
        "category": "agent_trace",
        "lane": "vox_dogfood_agent",
        "origin": "agent",
        "response_mode": "code_only",
        "task_family": "agent_trace"
    }))
}

/// Convert a batch of traces to SFT rows, then fail if the corpus is a
/// monoculture (semantic entropy below `min_diversity`).
pub fn traces_to_sft_gated(traces: &[Value], min_diversity: f64) -> anyhow::Result<Vec<Value>> {
    let rows: Vec<Value> = traces.iter().filter_map(trace_to_sft).collect();
    let responses: Vec<String> = rows.iter()
        .filter_map(|r| r.get("response").and_then(|v| v.as_str()).map(String::from))
        .collect();
    if !responses.is_empty() {
        let report = vox_eval::eval_semantic_entropy(&responses, min_diversity);
        anyhow::ensure!(!report.collapse_warning,
            "agentic trace corpus failed diversity check (mode collapse) — got {:.3}", report.ast_diversity);
    }
    Ok(rows)
}

/// Load agent traces from a JSON/JSONL file, convert them, run them through the diversity gate,
/// and write them to output_path.
pub fn generate_agent_traces_sft_file(
    input_path: &Path,
    output_path: &Path,
    min_diversity: f64,
) -> anyhow::Result<usize> {
    if let Some(p) = output_path.parent() {
        std::fs::create_dir_all(p)?;
    }

    let mut traces = Vec::new();
    if input_path.exists() {
        let content = std::fs::read_to_string(input_path)?;
        // Check if it's JSON array or JSONL
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            if let Some(arr) = val.as_array() {
                traces.extend(arr.clone());
            } else {
                traces.push(val);
            }
        } else {
            // Try JSONL
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<Value>(line) {
                    traces.push(val);
                }
            }
        }
    }

    // No traces (missing/empty input): write an empty file and return 0. The mix
    // source is `optional: true`, so an empty file is safe — we must NOT fabricate
    // synthetic rows tagged as genuine agent traces (silent corpus contamination).
    if traces.is_empty() {
        std::fs::File::create(output_path)?;
        return Ok(0);
    }

    let sft_rows = traces_to_sft_gated(&traces, min_diversity)?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(output_path)?);
    for row in &sft_rows {
        writeln!(file, "{}", serde_json::to_string(row)?)?;
    }
    file.flush()?;

    Ok(sft_rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_steps_yield_none() {
        assert!(trace_to_sft(&json!({"intent":"x","steps":[]})).is_none());
    }

    #[test]
    fn populated_trace_yields_agent_lane_row() {
        let t = json!({"intent":"list files","steps":[{"tool_name":"ls","arguments":{},"result":"a","success":true}]});
        let row = trace_to_sft(&t).unwrap();
        assert_eq!(row["lane"], "vox_dogfood_agent");
        assert_eq!(row["origin"], "agent");
    }

    #[test]
    fn test_diversity_collapse() {
        let single_trace = json!({
            "intent": "list files",
            "steps": [
                { "tool_name": "ls", "arguments": {}, "result": "a", "success": true }
            ]
        });
        let traces = vec![single_trace.clone(); 5];
        let res = traces_to_sft_gated(&traces, 0.90);
        assert!(res.is_err());
    }

    #[test]
    fn missing_input_writes_empty_does_not_fabricate() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("absent.jsonl");
        let output = dir.path().join("agent_traces.jsonl");
        let n = generate_agent_traces_sft_file(&input, &output, 0.40).unwrap();
        assert_eq!(n, 0, "missing input must yield zero rows, not synthetic data");
        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.trim().is_empty(), "output must be empty, not fabricated");
    }
}
