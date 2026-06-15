//! Merge live Socrates + repo sidecar JSON into `metadata_json` before worthiness preflight.
//! Mirrors `vox-cli` `scientia_worthiness_enrich` minus eval-gate **run directory** checks (those stay CLI-only until shared).

use std::path::Path;

use anyhow::Result;

use crate::publication::PublicationManifest;
fn extract_repository_id(manifest: &PublicationManifest) -> Option<String> {
    let raw = manifest.metadata_json.as_deref()?;
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    v.get("repository_id")
        .and_then(|x| x.as_str())
        .map(std::string::ToString::to_string)
}

/// When `metadata_json.scientia_evidence.socrates_aggregate` is missing or empty, fill from `socrates_surface` rows.
pub async fn merge_live_socrates_aggregate(
    manifest: PublicationManifest,
    db: &vox_db::VoxDb,
    repository_id_fallback: Option<&str>,
) -> Result<PublicationManifest> {
    let rid = extract_repository_id(&manifest)
        .or_else(|| repository_id_fallback.map(std::string::ToString::to_string));
    let Some(repository_id) = rid else {
        return Ok(manifest);
    };
    let merged = db
        .merge_scientia_live_socrates_into_metadata_json(
            manifest.metadata_json.as_deref(),
            repository_id.as_str(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut out = manifest;
    out.metadata_json = Some(merged);
    Ok(out)
}

/// Live Socrates rollup, then JSON sidecar hydration (`eval_gate_report_repo_relative`, benchmark pair JSON).
pub async fn enrich_manifest_socrates_and_sidecars(
    manifest: PublicationManifest,
    db: &vox_db::VoxDb,
    repo_root: &Path,
    repository_id_fallback: Option<&str>,
) -> Result<PublicationManifest> {
    let mut m = merge_live_socrates_aggregate(manifest, db, repository_id_fallback).await?;
    if let Some(updated) = crate::scientia_evidence::enrich_metadata_json_with_repo_files(
        m.metadata_json.as_deref(),
        repo_root,
    )? {
        m.metadata_json = Some(updated);
    }
    Ok(m)
}
