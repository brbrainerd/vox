//! Inputs required to run retrieval without MCP-specific handles.

use std::path::PathBuf;
use std::sync::Arc;

use vox_db::VoxDb;

/// Cross-platform retrieval runtime inputs (orchestrator, MCP, CLI).
#[derive(Clone)]
pub struct SearchRuntimeContext {
    /// Repository root used for inventory / path lexical matches.
    pub repo_root: PathBuf,
    /// Optional [`vox_db::VoxDb`] handle (knowledge graph, chunk hybrid, embeddings).
    pub db: Option<Arc<VoxDb>>,
    /// Directory scanned for markdown memory files (daily logs tree).
    pub memory_log_dir: PathBuf,
    /// Long-term memory markdown file path.
    pub memory_md_path: PathBuf,
    /// Optional MCP/orchestrator trace id for logging and outbound HTTP (e.g. Qdrant).
    pub trace_id: Option<String>,
}

impl std::fmt::Debug for SearchRuntimeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchRuntimeContext")
            .field("repo_root", &self.repo_root)
            .field("db_attached", &self.db.is_some())
            .field("memory_log_dir", &self.memory_log_dir)
            .field("memory_md_path", &self.memory_md_path)
            .field("has_trace_id", &self.trace_id.is_some())
            .finish()
    }
}

impl SearchRuntimeContext {
    /// New context; callers should canonicalize `repo_root` when possible.
    pub fn new(
        repo_root: PathBuf,
        db: Option<Arc<VoxDb>>,
        memory_log_dir: PathBuf,
        memory_md_path: PathBuf,
    ) -> Self {
        Self {
            repo_root,
            db,
            memory_log_dir,
            memory_md_path,
            trace_id: None,
        }
    }

    /// Attach a trace id for [`crate::execution::execute_search_plan`] telemetry and sidecar headers.
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: Option<String>) -> Self {
        self.trace_id = trace_id;
        self
    }
}

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;
    use std::path::PathBuf;

    fn ctx_no_db() -> SearchRuntimeContext {
        SearchRuntimeContext::new(
            PathBuf::from("/repo"),
            None,
            PathBuf::from("/mem"),
            PathBuf::from("/mem/lt.md"),
        )
    }

    #[test]
    fn debug_masks_db_as_bool() {
        let ctx = ctx_no_db();
        let s = format!("{ctx:?}");
        assert!(
            s.contains("db_attached: false"),
            "debug should expose db_attached bool; got: {s}"
        );
        assert!(
            !s.contains("VoxDb"),
            "debug must not leak VoxDb internals; got: {s}"
        );
    }

    #[test]
    fn debug_masks_trace_id_as_bool() {
        let ctx = ctx_no_db().with_trace_id(Some("secret-trace-123".to_string()));
        let s = format!("{ctx:?}");
        assert!(
            s.contains("has_trace_id: true"),
            "debug should show has_trace_id=true; got: {s}"
        );
        assert!(
            !s.contains("secret-trace-123"),
            "debug must not leak trace id value; got: {s}"
        );
    }

    #[test]
    fn with_trace_id_none_clears_trace() {
        let ctx = ctx_no_db()
            .with_trace_id(Some("abc".to_string()))
            .with_trace_id(None);
        assert!(ctx.trace_id.is_none());
        let s = format!("{ctx:?}");
        assert!(s.contains("has_trace_id: false"), "got: {s}");
    }

    #[test]
    fn with_trace_id_sets_value() {
        let ctx = ctx_no_db().with_trace_id(Some("trace-xyz".to_string()));
        assert_eq!(ctx.trace_id.as_deref(), Some("trace-xyz"));
    }
}
