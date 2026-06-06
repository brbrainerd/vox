use super::arxiv::{
    arxiv_assist_main_tex, arxiv_operator_handoff_value, pack_arxiv_staging_tar_gz,
};
use super::{ScholarlyVenue, StagingExportError};
use crate::citation_cff::render_citation_cff;
use crate::crossref_metadata::crossref_work_export_json;
use crate::publication::PublicationManifest;
use crate::zenodo_metadata;
use sha3::{Digest, Sha3_256};
use std::fs;
use std::path::Path;

pub fn write_scholarly_staging(
    manifest: &PublicationManifest,
    venue: ScholarlyVenue,
    out_dir: &Path,
) -> Result<Vec<String>, StagingExportError> {
    fs::create_dir_all(out_dir)?;
    let mut written: Vec<String> = Vec::new();

    let body_path = out_dir.join("body.md");
    fs::write(&body_path, manifest.body_markdown.as_bytes())?;
    written.push("body.md".to_string());

    let cff = render_citation_cff(manifest)?;
    fs::write(out_dir.join("CITATION.cff"), cff)?;
    written.push("CITATION.cff".to_string());

    let crossref = crossref_work_export_json(manifest);
    let crossref_s = serde_json::to_string_pretty(&crossref)?;
    fs::write(out_dir.join("crossref_work.json"), crossref_s)?;
    written.push("crossref_work.json".to_string());

    if let Some(raw) = manifest.citations_json.as_deref() {
        let t = raw.trim();
        if !t.is_empty() {
            fs::write(out_dir.join("citations.json"), raw.as_bytes())?;
            written.push("citations.json".to_string());
        }
    }

    if matches!(venue, ScholarlyVenue::Zenodo) {
        let zj = zenodo_metadata::zenodo_json_pretty(manifest)?;
        fs::write(out_dir.join("zenodo.json"), zj)?;
        written.push("zenodo.json".to_string());
    }

    if matches!(venue, ScholarlyVenue::ArxivAssist) {
        let main_tex_body = arxiv_assist_main_tex(manifest);
        fs::write(out_dir.join("main.tex"), main_tex_body)?;
        written.push("main.tex".to_string());
        let ah = serde_json::to_string_pretty(&arxiv_operator_handoff_value(manifest))?;
        fs::write(out_dir.join("arxiv_handoff.json"), ah)?;
        written.push("arxiv_handoff.json".to_string());
        let tar_path = out_dir.join("arxiv_bundle.tar.gz");
        pack_arxiv_staging_tar_gz(out_dir, &tar_path)?;
        written.push("arxiv_bundle.tar.gz".to_string());
    }

    write_staging_checksum_manifest(out_dir, &written)?;
    written.push("staging_checksums.json".to_string());

    Ok(written)
}

pub fn write_staging_checksum_manifest(
    out_dir: &Path,
    relpaths: &[String],
) -> Result<(), StagingExportError> {
    let mut sha_map = serde_json::Map::new();
    for rel in relpaths {
        if rel == "staging_checksums.json" {
            continue;
        }
        let p = out_dir.join(rel);
        if !p.is_file() {
            continue;
        }
        let bytes = fs::read(&p)?;
        let digest = Sha3_256::digest(&bytes);
        let hex = hex::encode(digest);
        sha_map.insert(rel.clone(), serde_json::json!(hex));
    }
    let doc = serde_json::json!({
        "schema_version": 1_i32,
        "sha3_256": sha_map,
    });
    fs::write(
        out_dir.join("staging_checksums.json"),
        serde_json::to_string_pretty(&doc)?,
    )?;
    Ok(())
}
