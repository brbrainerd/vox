//! `vox share` — share artifacts (workflows, skills, code) via the Vox marketplace.

use anyhow::{Context, Result};
use vox_pm::{ArtifactEntry, CodeStore};

/// Default store path for the local code store.
const DEFAULT_DB: &str = ".vox/store.db";

/// Get a CodeStore connection (remote if env vars set, else local).
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

fn print_artifact(a: &ArtifactEntry) {
    println!(
        "  {} ({}) v{} by {} — ⬇{} ★{:.1} [{}]",
        a.name, a.artifact_type, a.version, a.author_id, a.downloads, a.avg_rating, a.status
    );
    if let Some(ref desc) = a.description {
        println!("    {}", desc);
    }
}

/// Run the `vox share publish` subcommand.
pub async fn publish(
    artifact_type: &str,
    name: &str,
    hash: &str,
    version: &str,
    description: Option<&str>,
    tags: Option<&str>,
) -> Result<()> {
    let store = connect().await?;
    let id = format!("{name}-{version}");
    store
        .publish_artifact(
            &id,
            artifact_type,
            name,
            description,
            "local-user",
            hash,
            version,
            tags,
            "public",
        )
        .await?;
    println!("✓ Published {name} v{version} as {artifact_type}");
    Ok(())
}

/// Run the `vox share search` subcommand.
pub async fn search(query: &str) -> Result<()> {
    let store = connect().await?;
    let results = store.search_artifacts(query).await?;
    if results.is_empty() {
        println!("No artifacts found for '{query}'");
    } else {
        println!("Found {} artifacts:", results.len());
        for a in &results {
            print_artifact(a);
        }
    }
    Ok(())
}

/// Run the `vox share list` subcommand.
pub async fn list(artifact_type: &str) -> Result<()> {
    let store = connect().await?;
    let results = store.list_artifacts(artifact_type).await?;
    if results.is_empty() {
        println!("No {artifact_type} artifacts found.");
    } else {
        println!("{} {} artifacts:", results.len(), artifact_type);
        for a in &results {
            print_artifact(a);
        }
    }
    Ok(())
}

/// Run the `vox share review` subcommand.
pub async fn review(artifact_id: &str, rating: i64, comment: Option<&str>) -> Result<()> {
    let store = connect().await?;
    store
        .submit_review(artifact_id, "local-user", "approved", comment, Some(rating))
        .await?;
    println!("✓ Reviewed {artifact_id} with rating {rating}/5");
    Ok(())
}
