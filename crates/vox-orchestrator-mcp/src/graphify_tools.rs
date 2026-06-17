//! Graphify corpus tools (`vox_graphify_status`, `vox_graphify_search`).

use chrono::Utc;
use serde::Deserialize;
use std::fs;

use crate::git_exec::{GitExec, GitExecError};
use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_config::graphify::{
    CorpusStatus, GraphifyError, assess_corpus_status, lexical_search_graph, load_graphify_corpora,
};

const REM_GRAPHIFY: &str =
    "Ensure `contracts/retrieval/graphify-corpora.v1.yaml` exists and graph paths are readable.";

const DEFAULT_SEARCH_LIMIT: u32 = 10;

#[derive(Debug, Deserialize)]
pub struct GraphifyStatusParams {
    /// Corpus id; omit to report all corpora in the registry.
    pub corpus: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GraphifySearchParams {
    /// Corpus id from the registry; omit to use `default_corpus_id`.
    pub corpus: Option<String>,
    pub query: String,
    pub limit: Option<u32>,
}

fn corpus_by_id<'a>(
    reg: &'a vox_config::graphify::GraphifyCorporaRegistry,
    id: &str,
) -> Result<&'a vox_config::graphify::GraphifyCorpus, GraphifyError> {
    reg.corpora
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| GraphifyError::UnknownCorpus(id.to_string()))
}

fn selected_corpora<'a>(
    reg: &'a vox_config::graphify::GraphifyCorporaRegistry,
    corpus: &Option<String>,
) -> Result<Vec<&'a vox_config::graphify::GraphifyCorpus>, GraphifyError> {
    match corpus {
        Some(id) => Ok(vec![corpus_by_id(reg, id)?]),
        None => Ok(reg.corpora.iter().collect()),
    }
}

fn resolve_search_corpus<'a>(
    reg: &'a vox_config::graphify::GraphifyCorporaRegistry,
    corpus: &Option<String>,
) -> Result<(&'a vox_config::graphify::GraphifyCorpus, String), GraphifyError> {
    match corpus {
        Some(id) => {
            let c = corpus_by_id(reg, id)?;
            Ok((c, id.clone()))
        }
        None => {
            let id = reg.default_corpus_id.clone();
            let c = corpus_by_id(reg, &id)?;
            Ok((c, id))
        }
    }
}

fn knowledge_id(corpus_id: &str, node_id: &str) -> String {
    format!("graphify:{corpus_id}:node:{node_id}")
}

async fn resolve_head_sha(state: &ServerState) -> Option<String> {
    let cwd = state
        .repository
        .git_root
        .clone()
        .unwrap_or_else(|| state.repository.root.clone());
    let exec = GitExec::new(cwd);
    match exec.run(&["rev-parse", "HEAD"]).await {
        Ok(o) => {
            let sha = o.stdout.trim();
            if sha.is_empty() {
                None
            } else {
                Some(sha.to_string())
            }
        }
        Err(GitExecError::Banned(_))
        | Err(GitExecError::NonZero { .. })
        | Err(GitExecError::Spawn { .. }) => None,
    }
}

fn assess_all(
    repo_root: &std::path::Path,
    reg: &vox_config::graphify::GraphifyCorporaRegistry,
    corpus: &Option<String>,
    head_sha: Option<&str>,
) -> Result<Vec<CorpusStatus>, GraphifyError> {
    let now = Utc::now();
    let ttl = reg.ttl_days_default;
    selected_corpora(reg, corpus)?
        .into_iter()
        .map(|c| Ok(assess_corpus_status(repo_root, c, head_sha, now, ttl)))
        .collect()
}

/// `vox_graphify_status`: read-only freshness report for graphify corpora.
pub async fn graphify_status(state: &ServerState, params: GraphifyStatusParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let head = resolve_head_sha(state).await;
    let statuses = match assess_all(repo_root, &reg, &params.corpus, head.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let payload = serde_json::json!({
        "head_git_sha": head,
        "default_corpus_id": reg.default_corpus_id,
        "corpora": statuses,
    });
    ToolResult::ok(payload).to_json()
}

/// `vox_graphify_search`: lexical search over an on-disk graphify corpus graph.
pub async fn graphify_search(state: &ServerState, params: GraphifySearchParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let graph_path = repo_root.join(&corpus.graph_path);
    let raw = match fs::read_to_string(&graph_path) {
        Ok(s) => s,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("read {}: {e}", graph_path.display()),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let graph: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                format!("parse {}: {e}", graph_path.display()),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let limit = params.limit.unwrap_or(DEFAULT_SEARCH_LIMIT).max(1);
    let hits = lexical_search_graph(&graph, &corpus_id, &params.query, limit as usize);
    let payload_hits: Vec<serde_json::Value> = hits
        .into_iter()
        .map(|h| {
            serde_json::json!({
                "node_id": h.node_id,
                "label": h.label,
                "score": h.score,
                "knowledge_id": knowledge_id(&corpus_id, &h.node_id),
            })
        })
        .collect();
    let payload = serde_json::json!({
        "corpus_id": corpus_id,
        "hits": payload_hits,
    });
    ToolResult::ok(payload).to_json()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use vox_orchestrator::{
        AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
    };
    use vox_repository::{RepoCapabilities, RepositoryContext};
    use vox_skills::new_registry_arc;

    fn write_registry(repo: &Path) {
        let dir = repo.join("contracts/retrieval");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graphify-corpora.v1.yaml"),
            include_str!("../../../contracts/retrieval/graphify-corpora.v1.yaml"),
        )
        .unwrap();
    }

    fn write_sample_graph(repo: &Path) {
        let dir = repo.join("graphify-out");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("graph.json"),
            r#"{"nodes":[{"id":"auth","label":"authentication module","type":"module"}],"links":[]}"#,
        )
        .unwrap();
    }

    fn test_state_for_repo(root: std::path::PathBuf) -> ServerState {
        let cfg = OrchestratorConfig::for_testing();
        let orch_cfg = cfg.clone();
        let groups = AffinityGroupRegistry::new(vec![]);
        let session_cfg = SessionConfig {
            persist: false,
            sessions_dir: std::env::temp_dir().join("vox-mcp-graphify-tools-test-sessions"),
            ..SessionConfig::default()
        };
        let session_manager = SessionManager::new(session_cfg).expect("session manager");
        let repository = RepositoryContext {
            root,
            git_root: None,
            repository_id: "graphify-tools-test".into(),
            origin_url: None,
            capabilities: RepoCapabilities {
                vox_project: false,
                cargo_workspace: false,
                cargo_package: false,
                node_workspace: false,
                python_project: false,
                go_module: false,
                git: false,
            },
            has_vox_agents_dir: false,
            vox_toml: None,
        };
        ServerState::test_stub(
            cfg,
            repository,
            Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
            Arc::new(Mutex::new(session_manager)),
            new_registry_arc(),
        )
    }

    #[test]
    fn unknown_corpus_returns_error_from_assess_all() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        let err = assess_all(tmp.path(), &reg, &Some("missing".into()), None).unwrap_err();
        assert!(matches!(err, GraphifyError::UnknownCorpus(_)));
    }

    #[tokio::test]
    async fn graphify_search_returns_matching_hit() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        write_sample_graph(tmp.path());
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = graphify_search(
            &state,
            GraphifySearchParams {
                corpus: Some("repo-code-graph".into()),
                query: "authentication".into(),
                limit: None,
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
        let data = parsed.get("data").expect("data");
        assert_eq!(
            data.get("corpus_id"),
            Some(&serde_json::json!("repo-code-graph"))
        );
        let hits = data
            .get("hits")
            .and_then(|h| h.as_array())
            .expect("hits array");
        assert!(!hits.is_empty(), "expected at least one hit: {json}");
        let first = &hits[0];
        assert_eq!(first.get("node_id"), Some(&serde_json::json!("auth")));
        assert_eq!(
            first.get("knowledge_id"),
            Some(&serde_json::json!("graphify:repo-code-graph:node:auth"))
        );
        assert!(
            first
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .contains("authentication")
        );
    }
}
