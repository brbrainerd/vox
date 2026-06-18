use lcov_parser::{LCOVParser, LCOVRecord, ParsedResult};
use serde_json::{Value, json};

pub fn ingest_lcov_reachability(graph: &Value, lcov_content: &str) -> Result<Value, String> {
    let mut updated = graph.clone();
    let mut execution_counts = std::collections::HashMap::new();

    let mut parser = LCOVParser::new(lcov_content.as_bytes());
    loop {
        match parser.parse_next() {
            ParsedResult::Ok(record, _) => {
                if let LCOVRecord::FunctionData(count, fn_name) = record {
                    execution_counts.insert(fn_name, count as u64);
                }
            }
            ParsedResult::Eof => break,
            ParsedResult::Err(e) => {
                return Err(format!("LCOV parse error: {:?}", e));
            }
        }
    }

    if let Some(nodes) = updated.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                let count = execution_counts.get(id).copied().unwrap_or(0);
                node.as_object_mut()
                    .unwrap()
                    .insert("execution_count".to_string(), json!(count));
            }
        }
    }

    Ok(updated)
}
