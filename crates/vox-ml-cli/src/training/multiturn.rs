//! Multi-turn conversation templates.

/// Returns true if the given Vox construct type supports decorators.
/// Only `fn` and `type` declarations accept decorator prefixes in Vox.
fn construct_accepts_decorators(construct: &str) -> bool {
    matches!(construct, "function" | "fn" | "method")
}

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
///
/// # Decorator placement safety
/// `@deprecated` and `@traced` are only valid on `fn` and `type` declarations in Vox.
/// For other constructs (import, component, actor, state_machine, etc.) we use
/// non-decorator refinements: inline documentation comments and stability markers.
pub fn generate_multiturn_pairs(
    construct: &str,
    name: &str,
    base_instruction: &str,
    code: &str,
    schema_version: &str,
    source: &str,
) -> Vec<serde_json::Value> {
    let mut pairs = Vec::new();
    let supports_decorators =
        construct_accepts_decorators(construct) && !code.trim_start().starts_with('@');

    // Generate 3 turns of refinements with programmatically generated refined code.
    for index in 0..3 {
        let (follow_up, refined_code) = match index {
            0 => (
                format!("Add a docstring explaining the purpose of the `{name}` {construct}"),
                format!("/// The `{name}` {construct} is documented here.\n{code}"),
            ),
            1 => {
                if supports_decorators {
                    (
                        format!(
                            "Mark the `{name}` {construct} as deprecated using the `@deprecated` decorator"
                        ),
                        format!("@deprecated\n{code}"),
                    )
                } else {
                    // Non-fn/type constructs cannot carry decorator prefixes in Vox.
                    // Use a documentation comment refinement instead.
                    (
                        format!("Add a TODO comment noting the `{name}` {construct} needs review"),
                        format!(
                            "// TODO: Review the `{name}` {construct} for correctness.\n{code}"
                        ),
                    )
                }
            }
            _ => {
                if supports_decorators {
                    (
                        format!("Add a tracing decorator `@traced` to the `{name}` {construct}"),
                        format!("@traced\n{code}"),
                    )
                } else {
                    // Stability marker comment -- safe for any construct type.
                    (
                        format!("Mark the `{name}` {construct} as stable with a stability comment"),
                        format!("// STABLE: `{name}` {construct} is production-ready.\n{code}"),
                    )
                }
            }
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

    #[test]
    fn test_non_fn_constructs_do_not_get_decorators() {
        let base_code = "import std.mobile";
        let pairs = generate_multiturn_pairs(
            "import",
            "std.mobile",
            "import std.mobile",
            base_code,
            "vox_dogfood_v1",
            "test_file.vox",
        );

        assert_eq!(pairs.len(), 3);
        for pair in &pairs {
            let response = pair["response"].as_str().unwrap();
            // Decorators like @traced and @deprecated must NOT appear on import refinements
            assert!(
                !response.starts_with("@deprecated"),
                "import construct must not be decorated with @deprecated"
            );
            assert!(
                !response.starts_with("@traced"),
                "import construct must not be decorated with @traced"
            );
        }
    }

    #[test]
    fn test_type_constructs_do_not_get_decorators() {
        let base_code = "type UserProfile {\n    id: int\n}";
        let pairs = generate_multiturn_pairs(
            "type",
            "UserProfile",
            "define type UserProfile",
            base_code,
            "vox_dogfood_v1",
            "test_file.vox",
        );

        assert_eq!(pairs.len(), 3);
        for pair in &pairs {
            let response = pair["response"].as_str().unwrap();
            assert!(
                !response.starts_with("@deprecated"),
                "type construct must not be decorated with @deprecated"
            );
            assert!(
                !response.starts_with("@traced"),
                "type construct must not be decorated with @traced"
            );
        }
    }

    #[test]
    fn test_already_decorated_constructs_do_not_get_decorators() {
        let base_code = "@query fn my_query() to str {\n    return \"hello\";\n}";
        let pairs = generate_multiturn_pairs(
            "fn",
            "my_query",
            "write my_query",
            base_code,
            "vox_dogfood_v1",
            "test_file.vox",
        );

        assert_eq!(pairs.len(), 3);
        for pair in &pairs {
            let response = pair["response"].as_str().unwrap();
            assert!(
                !response.starts_with("@deprecated"),
                "already decorated construct must not be prepended with @deprecated"
            );
            assert!(
                !response.starts_with("@traced"),
                "already decorated construct must not be prepended with @traced"
            );
        }
    }
}
