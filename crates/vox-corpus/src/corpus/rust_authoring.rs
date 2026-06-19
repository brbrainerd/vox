//! Generate (instruction -> idiomatic Rust) SFT pairs from workspace .rs files.

use serde_json::json;

/// Build one SFT pair JSON from a function name + its source. Lane: vox_rust_authoring.
pub fn make_authoring_pair(fn_name: &str, rust_src: &str) -> serde_json::Value {
    let instruction = format!("Write an idiomatic Rust function named `{fn_name}`.");
    json!({
        "prompt": instruction,
        "response": format!("```rust\n{rust_src}\n```"),
        "messages": [
            {"role": "user", "content": instruction},
            {"role": "assistant", "content": format!("```rust\n{rust_src}\n```")}
        ],
        "category": "rust_authoring",
        "lane": "vox_rust_authoring",
        "origin": "human",
        "response_mode": "code_only",
        "task_family": "rust_authoring"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pair_has_lane_and_assistant_turn() {
        let p = make_authoring_pair("add", "fn add(a: i32, b: i32) -> i32 { a + b }");
        assert_eq!(p["lane"], "vox_rust_authoring");
        let msgs = p["messages"].as_array().unwrap();
        assert_eq!(msgs.last().unwrap()["role"], "assistant");
    }
}
