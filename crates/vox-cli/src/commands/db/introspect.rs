//! Live app-plane SQL schema introspection (`vox db introspect`).

use anyhow::Result;
use vox_sql::AnySqlBackend;

pub async fn introspect(url: Option<&str>, compact: bool) -> Result<()> {
    let backend = if let Some(url) = url {
        AnySqlBackend::connect_from_url(url).await?
    } else {
        AnySqlBackend::connect_from_app_env().await?
    };
    let schema = backend.introspect_schema().await?;
    if compact {
        println!("{}", serde_json::to_string(&schema)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&schema)?);
    }
    Ok(())
}
