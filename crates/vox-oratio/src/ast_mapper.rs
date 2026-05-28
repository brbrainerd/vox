//! AST mapping for navigating to specific nodes without dictating exact characters.

use serde::{Deserialize, Serialize};

/// An AST-aware target that Vox Oratio can map to in the IDE.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstTarget {
    /// The type of node (e.g. "function", "struct", "impl").
    pub node_kind: String,
    /// Extracted symbol name, if identified.
    pub symbol_name: Option<String>,
    /// Whether to target the node currently at the cursor.
    pub at_cursor: bool,
}

/// Simple heuristic to extract AST targets from transcript (Phase 1C MVP).
#[must_use]
pub fn map_to_ast_target(
    transcript: &str,
    context: Option<&crate::routing::IdeContext>,
) -> Option<AstTarget> {
    let lower = transcript.to_ascii_lowercase();

    // Contextual targeting for "this function", "this struct", etc.
    if lower.contains("this") {
        let kind = if lower.contains("function") || lower.contains("fn") {
            Some("function")
        } else if lower.contains("struct") {
            Some("struct")
        } else if lower.contains("type") {
            Some("type")
        } else if lower.contains("module") || lower.contains("file") {
            Some("module")
        } else {
            None
        };

        if let Some(k) = kind {
            let symbol_name = if let Some(ctx) = context {
                ctx.symbol_stack.first().cloned()
            } else {
                None
            };
            return Some(AstTarget {
                node_kind: k.to_string(),
                symbol_name,
                at_cursor: context.and_then(|c| c.cursor_line).is_some(),
            });
        }
    }

    let parts: Vec<&str> = lower.split_whitespace().collect();

    for (i, p) in parts.iter().enumerate() {
        if *p == "function" || *p == "fn" || *p == "struct" || *p == "type" {
            let kind = if *p == "fn" {
                "function".to_string()
            } else {
                p.to_string()
            };
            let mut name = None;
            let mut j = i + 1;
            // Skip connector words like "called", "named", "for"
            if j < parts.len() && matches!(parts[j], "called" | "named" | "for") {
                j += 1;
            }
            if j < parts.len() {
                let clean = parts[j].trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if !clean.is_empty() {
                    name = Some(clean.to_string());
                }
            }
            return Some(AstTarget {
                node_kind: kind,
                symbol_name: name,
                at_cursor: false,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::IdeContext;

    #[test]
    fn test_map_function_by_name() {
        let target = map_to_ast_target("create a function called hello", None).unwrap();
        assert_eq!(target.node_kind, "function");
        assert_eq!(target.symbol_name.as_deref(), Some("hello"));
        assert!(!target.at_cursor);
    }

    #[test]
    fn test_map_this_function_with_context() {
        let ctx = IdeContext {
            cursor_line: Some(10),
            ..IdeContext::default()
        };
        let target = map_to_ast_target("edit this function", Some(&ctx)).unwrap();
        assert_eq!(target.node_kind, "function");
        assert!(target.symbol_name.is_none());
        assert!(target.at_cursor);
    }

    #[test]
    fn test_map_this_function_without_context() {
        let target = map_to_ast_target("edit this function", None).unwrap();
        assert_eq!(target.node_kind, "function");
        assert!(!target.at_cursor);
    }
}
