//! Deterministic risk score (0–100) from tool name + args heuristics.

use serde_json::Value;

use super::mutation_classifier::mutation_kind_for_tool;

/// Higher means more dangerous; used by [`super::guardrail_kernel`].
pub fn tool_risk_score(tool: &str, args: &Value) -> u8 {
    let mut score: u32 = match mutation_kind_for_tool(tool) {
        "read_only" => 5,
        "local_mutation" => 35,
        "external_side_effect" => 60,
        _ => 25,
    };

    let args_str = args.to_string().to_ascii_lowercase();
    if args_str.contains("rm -rf")
        || args_str.contains("format-volume")
        || args_str.contains("remove-item -recurse")
    {
        score = score.saturating_add(40);
    }
    if args_str.contains("http://") || args_str.contains("https://") {
        score = score.saturating_add(10);
    }

    score.min(100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn destructive_pattern_boosts_score() {
        let s = tool_risk_score(
            "vox_run_shell",
            &json!({ "command": "rm -rf /", "user_approval": true }),
        );
        assert!(s >= 90);
    }
}
