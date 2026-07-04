//! JSON Schema validation for SCIENTIA finding-candidate and novelty-evidence ledger contracts.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::Value as JsonValue;

use vox_bounded_fs::read_utf8_path_capped;

/// Repo-relative path to finding candidate v1 JSON Schema.
pub const FINDING_CANDIDATE_SCHEMA_REL: &str =
    "contracts/scientia/finding-candidate.v1.schema.json";
/// Repo-relative path to novelty evidence bundle v1 JSON Schema.
pub const NOVELTY_BUNDLE_SCHEMA_REL: &str =
    "contracts/scientia/novelty-evidence-bundle.v1.schema.json";

fn validate_json_against_schema(
    repo_root: &Path,
    schema_rel: &str,
    data_path: &Path,
    label: &str,
) -> Result<()> {
    let schema_path = repo_root.join(schema_rel);
    let schema_src = read_utf8_path_capped(&schema_path)
        .with_context(|| format!("read {}", schema_path.display()))?;
    let data_src = read_utf8_path_capped(data_path)
        .with_context(|| format!("read {}", data_path.display()))?;

    let schema_val: JsonValue = serde_json::from_str(&schema_src).with_context(|| {
        format!(
            "parse JSON Schema {}",
            schema_path
                .strip_prefix(repo_root)
                .unwrap_or(&schema_path)
                .display()
        )
    })?;
    let data_val: JsonValue = serde_json::from_str(&data_src)
        .with_context(|| format!("parse JSON {}", data_path.display()))?;

    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .with_context(|| format!("compile JSON Schema {}", schema_path.display()))?;
    vox_jsonschema_util::validate(
        &data_val,
        &validator,
        format!("{label}: {} vs {}", data_path.display(), schema_rel),
    )
    .map_err(|e| anyhow!("{e:#}"))?;

    println!(
        "OK: {} validates against {}",
        data_path.display(),
        schema_rel
    );
    Ok(())
}

/// Validate a finding-candidate JSON file against `finding-candidate.v1.schema.json`.
pub fn validate_finding_candidate_file(repo_root: &Path, json_path: &Path) -> Result<()> {
    validate_json_against_schema(
        repo_root,
        FINDING_CANDIDATE_SCHEMA_REL,
        json_path,
        "finding-candidate",
    )
}

/// Validate a novelty-evidence-bundle JSON file against `novelty-evidence-bundle.v1.schema.json`.
pub fn validate_novelty_bundle_file(repo_root: &Path, json_path: &Path) -> Result<()> {
    validate_json_against_schema(
        repo_root,
        NOVELTY_BUNDLE_SCHEMA_REL,
        json_path,
        "novelty-evidence-bundle",
    )
}

/// Default example paths (repo-relative) exercised by `vox ci scientia-novelty-ledger-contracts`.
pub fn example_finding_candidate_path(repo_root: &Path) -> PathBuf {
    repo_root.join("contracts/reports/scientia-finding-candidate.example.v1.json")
}

pub fn example_novelty_bundle_path(repo_root: &Path) -> PathBuf {
    repo_root.join("contracts/reports/scientia-novelty-evidence-bundle.example.v1.json")
}
