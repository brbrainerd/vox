//! Publication manifest and syndication helpers for `vox db publication-*`.

mod archive_code;
pub use archive_code::*;
pub(crate) mod embedder;
mod helpers;
mod ingest;

pub use ingest::*;
mod preflight;
pub use preflight::*;
mod scholarly;
pub use scholarly::*;
mod discovery;
pub use discovery::*;
mod internal;
pub(crate) use internal::*;
mod route;
pub use route::*;
mod decision;
pub use decision::*;
mod media;
pub use media::*;
mod remote_jobs;
pub use remote_jobs::*;
mod prepare;
pub use prepare::*;

use crate::commands::db_cli::{ArxivHandoffStageCli, ScholarlyVenueCli};
use anyhow::Result;
use std::time::Instant;
use vox_bounded_fs::read_utf8_path_capped;

use helpers::{
    build_scientia_evidence_context, read_scientific_metadata_json, repository_id_for_prepare,
    source_ref_string,
};

/// Simulate per-channel routing/policy outcomes for one prepared publication id.
///
/// When `json` is true, prints one line of compact JSON (stable key order from `serde_json`).
pub async fn publication_route_simulate(publication_id: &str, json: bool) -> Result<()> {
    let db = vox_db::VoxDb::connect_default().await?;
    let result = publication_route_simulate_with_db(&db, publication_id).await?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    Ok(())
}
