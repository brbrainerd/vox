//! Multi-turn conversation templates.

/// Return follow-up instruction templates for a given construct.
/// These are used after a base instruction pair to simulate multi-turn refinement.
/// `{name}` is replaced with the extracted identifier.
pub fn followup_templates(construct: &str) -> &[&str] {
    let _ = construct;
    &[
        "Add a docstring explaining the purpose of the `{name}` {construct}",
        "Mark the `{name}` {construct} as deprecated using the `@deprecated` decorator",
        "Add a tracing decorator `@traced` to the `{name}` {construct}",
    ]
}

/// Build multi-turn conversation pairs from a base (instruction, code) pair.
/// Returns a Vec of (follow_up_prompt, refined_code) where refined_code is modified
/// by prepending docstrings or decorators so the model learns to refine code.
pub fn generate_multiturn_pairs(
    construct: &str,
    name: &str,
    base_instruction: &str,
    code: &str,
    schema_version: &str,
    source: &str,
) -> Vec<serde_json::Value> {
    let mut pairs = Vec::new();

    // Generate 3 turns of refinements with programmatically generated refined code.
    for index in 0..3 {
        let (follow_up, refined_code) = match index {
            0 => (
                format!("Add a docstring explaining the purpose of the `{name}` {construct}"),
                format!("/// The `{name}` {construct} is documented here.\n{code}"),
            ),
            1 => (
                format!(
                    "Mark the `{name}` {construct} as deprecated using the `@deprecated` decorator"
                ),
                format!("@deprecated\n{code}"),
            ),
            _ => (
                format!("Add a tracing decorator `@traced` to the `{name}` {construct}"),
                format!("@traced\n{code}"),
            ),
        };

        // Multi-turn format: include the previous exchange as context in the prompt
        let prompt = format!(
            "Previous instruction: {base_instruction}\nPrevious code:\n```vox\n{code}\n```\n\nFollow-up: {follow_up}"
        );
        pairs.push(serde_json::json!({
            "prompt": prompt,
            "response": refined_code,
            "instruction": follow_up,
            "output": refined_code,
            "category": construct,
            "difficulty": crate::training::construct_difficulty(construct),
            "source": source,
            "rating": 4,
            "turn": 2,
            "schema_version": schema_version,
        }));
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_multiturn_pairs_refines_code() {
        let base_code = "fn my_fn() to str {\n    return \"hello\";\n}";
        let pairs = generate_multiturn_pairs(
            "function",
            "my_fn",
            "write my_fn",
            base_code,
            "vox_dogfood_v1",
            "test_file.vox",
        );

        assert_eq!(pairs.len(), 3);
        for pair in &pairs {
            let response = pair["response"].as_str().unwrap();
            let output = pair["output"].as_str().unwrap();

            // Assert that the refined code is NOT equal to the original code
            assert_ne!(
                response, base_code,
                "Refined code should be modified, not identical to base code"
            );
            assert_ne!(output, base_code, "Refined code output should be modified");
        }
    }
}
