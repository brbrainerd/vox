use serde_json::{Value, json};

pub fn ingest_lcov_reachability(graph: &Value, lcov_content: &str) -> Result<Value, String> {
    let mut updated = graph.clone();
    let mut execution_counts = std::collections::HashMap::new();

    // Simple line parsing for FNDA (function execution counts)
    for line in lcov_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("FNDA:") {
            let parts: Vec<&str> = trimmed[5..].split(',').collect();
            if parts.len() == 2 {
                if let Ok(count) = parts[0].parse::<u64>() {
                    let fn_name = parts[1].to_string();
                    execution_counts.insert(fn_name, count);
                }
            }
        }
    }

    if let Some(nodes) = updated.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                let count = execution_counts.get(id).copied().unwrap_or(0);
                node.as_object_mut().unwrap().insert(
                    "execution_count".to_string(),
                    json!(count),
                );
            }
        }
    }

    Ok(updated)
}
