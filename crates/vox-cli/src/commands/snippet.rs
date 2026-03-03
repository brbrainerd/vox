//! `vox snippet` — save, search, and manage code snippets.

use anyhow::{Context, Result};
use vox_pm::{CodeStore, SnippetEntry};

const DEFAULT_DB: &str = ".vox/store.db";

async fn connect() -> Result<CodeStore> {
    if let (Ok(url), Ok(token)) = (
        std::env::var("VOX_TURSO_URL"),
        std::env::var("VOX_TURSO_TOKEN"),
    ) {
        CodeStore::open_remote(&url, &token)
            .await
            .context("Failed to connect to remote store")
    } else {
        std::fs::create_dir_all(".vox").ok();
        CodeStore::open(DEFAULT_DB)
            .await
            .context("Failed to open local store")
    }
}

fn print_snippet(s: &SnippetEntry) {
    println!("  [{}] {} ({})", s.id, s.title, s.language);
    if let Some(ref desc) = s.description {
        println!("    {}", desc);
    }
    for (i, line) in s.code.lines().take(3).enumerate() {
        println!("    {}│ {}", i + 1, line);
    }
    if s.code.lines().count() > 3 {
        println!("    ... ({} more lines)", s.code.lines().count() - 3);
    }
}

/// Save a code snippet from a file.
pub async fn save(
    file: &std::path::Path,
    title: &str,
    description: Option<&str>,
    tags: Option<&str>,
) -> Result<()> {
    let code = std::fs::read_to_string(file)?;
    let lang = file.extension().and_then(|e| e.to_str()).unwrap_or("vox");

    let store = connect().await?;
    let id = store
        .save_snippet(
            lang,
            title,
            &code,
            description,
            tags,
            Some("local-user"),
            Some(&file.display().to_string()),
            None,
        )
        .await?;
    println!("✓ Saved snippet #{id}: {title}");
    Ok(())
}

/// Search code snippets.
pub async fn search(query: &str) -> Result<()> {
    let store = connect().await?;
    let results = store.search_snippets(query, None).await?;
    if results.is_empty() {
        println!("No snippets found for '{query}'");
    } else {
        println!("Found {} snippets:", results.len());
        for s in &results {
            print_snippet(s);
        }
    }
    Ok(())
}

/// Export snippets as JSON (for RLHF/RAG pipelines).
pub async fn export(limit: i64) -> Result<()> {
    let store = connect().await?;
    let results = store.search_snippets("", None).await?;
    let limited: Vec<_> = results.into_iter().take(limit as usize).collect();
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "snippets": limited.iter().map(|s| serde_json::json!({
            "id": s.id,
            "language": s.language,
            "title": s.title,
            "code": s.code,
            "description": s.description,
            "tags": s.tags,
        })).collect::<Vec<_>>()
    }))?;
    println!("{json}");
    Ok(())
}
