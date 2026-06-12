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
        let mut ctx = IdeContext::default();
        ctx.cursor_line = Some(10);
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
