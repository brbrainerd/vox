//! Shared helpers for normalizing LLM tool outputs (markdown fences, token preservation checks).

use std::path::Path;

use serde_json::Value;

/// Strip a leading ```json … ``` or generic ``` fence from model output.
#[must_use]
pub(crate) fn strip_json_codeblock_fence(s: &str) -> String {
    let block = s.trim();
    if let Some(rest) = block.strip_prefix("```json") {
        let mut inner = rest.trim_start_matches(['\n', '\r']).trim();
        if let Some(pos) = inner.rfind("```") {
            inner = inner[..pos].trim();
        }
        return inner.to_string();
    }
    if block.starts_with("```") {
        let rest = block.strip_prefix("```").unwrap_or(block).trim();
        let inner = rest.strip_suffix("```").unwrap_or(rest).trim();
        return inner.to_string();
    }
    block.to_string()
}

/// Strip ```vox or plain ``` fences around generated `.vox` source.
#[must_use]
#[cfg(test)]
pub(crate) fn strip_vox_codegen_fence(s: &str) -> String {
    vox_compiler::generated_vox::strip_vox_codeblock_fence(s)
}

#[allow(dead_code)]
pub(crate) fn validate_llm_surface(original: &str, corrected: &str) -> bool {
    if corrected.is_empty() {
        return false;
    }
    let max_len = original.len().saturating_mul(5).clamp(1024, 32_768);
    if corrected.len() > max_len {
        return false;
    }
    for marker in ["::", "--"] {
        let oc = original.matches(marker).count();
        let cc = corrected.matches(marker).count();
        if cc + 2 < oc {
            return false;
        }
    }
    let om = original.matches('/').count() + original.matches('\\').count();
    let cm = corrected.matches('/').count() + corrected.matches('\\').count();
    if om > 0 && cm + 2 < om {
        return false;
    }
    true
}

/// Ensure flag-like, path-like, and module-path tokens from the deterministic transcript appear in the LLM output.
#[allow(dead_code)]
pub(crate) fn protected_tokens_preserved(original: &str, corrected: &str) -> bool {
    for raw in original.split_whitespace() {
        let flag_like = raw.starts_with("--");
        let path_like = raw.contains('/') || raw.contains('\\');
        let mod_like = raw.contains("::");
        if flag_like || path_like || mod_like {
            if !corrected.contains(raw) {
                let redacted = if path_like {
                    Path::new(raw.trim_matches(|c| c == '`' || c == '"'))
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("(path)")
                } else {
                    raw
                };
                tracing::debug!(
                    target: "vox_mcp_oratio",
                    stage = "llm_pass",
                    missing_token_redacted = redacted,
                    "protected token not preserved in LLM correction"
                );
                return false;
            }
        }
    }
    true
}

#[allow(dead_code)]
pub(crate) fn llm_changes_well_formed(v: &Value) -> bool {
    match v.get("changes") {
        None => true,
        Some(Value::Array(arr)) => arr.iter().all(|item| {
            item.get("before").and_then(|x| x.as_str()).is_some()
                && item.get("after").and_then(|x| x.as_str()).is_some()
        }),
        Some(_) => false,
    }
}

#[allow(dead_code)]
pub(crate) fn llm_confidence_field_ok(v: &Value) -> bool {
    match v.get("confidence") {
        None => true,
        Some(c) => c.as_f64().is_some_and(|x| (0.0..=1.0).contains(&x)),
    }
}

#[cfg(test)]
mod tests {
    use super::strip_vox_codegen_fence;

    #[test]
    fn strips_vox_fence() {
        let input = "```vox\nactor Ping { state n: i32 }\n```";
        assert_eq!(
            strip_vox_codegen_fence(input),
            "actor Ping { state n: i32 }"
        );
    }

    #[test]
    fn leaves_plain_code_unchanged() {
        let input = "actor Pong { state n: i32 }";
        assert_eq!(strip_vox_codegen_fence(input), input);
    }

    #[test]
    fn strips_mixed_prose_and_fence() {
        let input = "Here is code:\n```vox\nactor A { state n: i32 }\n```";
        assert_eq!(
            strip_vox_codegen_fence(input),
            "Here is code:\n```vox\nactor A { state n: i32 }\n```"
        );
    }

    #[test]
    fn handles_malformed_fence_block() {
        let input = "```vox\nactor Broken {";
        assert_eq!(strip_vox_codegen_fence(input), input);
    }
}
