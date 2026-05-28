//! `vox memory search` — prints retrieval diagnostics JSON.

use std::sync::Arc;

use vox_db::{DbConfig, VoxDb};
use vox_repository::discover_repository_or_fallback;
use vox_search::{
    RetrievalTriggerMode, SearchPolicy, SearchRuntimeContext, run_search_with_verification,
};

pub async fn run(query_parts: Vec<String>, limit: usize) -> anyhow::Result<()> {
    let query = query_parts.join(" ").trim().to_string();
    if query.is_empty() {
        anyhow::bail!("query must not be empty");
    }

    let cwd = std::env::current_dir()?;
    let repo_ctx = discover_repository_or_fallback(&cwd);
    let repo_root = repo_ctx.root;

    let mem = vox_orchestrator::MemoryConfig::default();
    let log_dir = cwd.join(&mem.log_dir);
    let memory_md_path = cwd.join(&mem.memory_md_path);

    let db: Option<Arc<VoxDb>> = match DbConfig::resolve_canonical() {
        Ok(cfg) => VoxDb::connect(cfg).await.ok().map(Arc::new),
        Err(_) => None,
    };

    let ctx = SearchRuntimeContext::new(repo_root, db, log_dir, memory_md_path);
    let policy = SearchPolicy::from_env();

    let (_exec, diagnostics, _plan) = run_search_with_verification(
        &ctx,
        &query,
        RetrievalTriggerMode::ExplicitToolQuery,
        limit,
        &policy,
        None,
        None,
    )
    .await
    .map_err(anyhow::Error::msg)?;

    println!("{}", serde_json::to_string_pretty(&diagnostics)?);
    Ok(())
}
