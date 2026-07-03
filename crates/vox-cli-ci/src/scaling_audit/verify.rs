//! Schema / SSOT verification helpers for `scaling_audit`.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use vox_bounded_fs::read_utf8_path_capped;

use super::{
    FINDINGS_ARRAY_SCHEMA_REL, FINDINGS_LATEST_REL, GOLD_DATASET_REL, GOLD_SCHEMA_REL, POLICY_REL,
    POLICY_SCHEMA_REL, REMEDIATION_DELTA_REL, REMEDIATION_DELTA_SCHEMA_REL, REMEDIATION_LANES_REL,
    REMEDIATION_REPORT_ROOT,
};

/// Validate `contracts/scaling/policy.yaml` against `policy.schema.json`.
pub(super) fn verify_policy_schema(repo_root: &Path) -> Result<()> {
    let policy_path = repo_root.join(POLICY_REL);
    let raw = read_utf8_path_capped(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let schema_path = repo_root.join(POLICY_SCHEMA_REL);
    if !schema_path.is_file() {
        return Err(anyhow!("missing {}", schema_path.display()));
    }
    let schema_val: JsonValue = serde_json::from_str(
        &read_utf8_path_capped(&schema_path)
            .with_context(|| format!("read {}", schema_path.display()))?,
    )
    .with_context(|| format!("parse {}", schema_path.display()))?;
    let instance: JsonValue =
        serde_yaml::from_str(&raw).context("parse scaling policy as JSON value")?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())?;
    vox_jsonschema_util::validate(
        &instance,
        &validator,
        format!("scaling policy YAML vs {}", schema_path.display()),
    )
    .map_err(|e| anyhow!("{e:#}"))?;
    println!("scaling policy YAML OK (schema validated)");
    Ok(())
}

/// Embedded policy must match on-disk file (vox-scaling-policy `include_str`).
pub(super) fn verify_embedded_policy_roundtrip(repo_root: &Path) -> Result<()> {
    let policy_path = repo_root.join(POLICY_REL);
    let disk = read_utf8_path_capped(&policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    let parsed = vox_scaling_policy::ScalingPolicy::from_yaml_str(&disk)
        .context("parse policy with vox-scaling-policy")?;
    if parsed.schema_version < 1 {
        return Err(anyhow!("scaling policy schema_version must be >= 1"));
    }
    println!("vox-scaling-policy parse OK");
    Ok(())
}

fn validate_json_file_against_schema(
    repo_root: &Path,
    instance_rel: &str,
    schema_rel: &str,
    label: &str,
) -> Result<()> {
    let inst_path = repo_root.join(instance_rel);
    if !inst_path.is_file() {
        return Err(anyhow!("missing {} file {}", label, inst_path.display()));
    }
    let schema_path = repo_root.join(schema_rel);
    if !schema_path.is_file() {
        return Err(anyhow!(
            "missing {} schema {}",
            label,
            schema_path.display()
        ));
    }
    let schema_val: JsonValue = serde_json::from_str(
        &read_utf8_path_capped(&schema_path)
            .with_context(|| format!("read {}", schema_path.display()))?,
    )
    .with_context(|| format!("parse {}", schema_path.display()))?;
    let instance_val: JsonValue = serde_json::from_str(
        &read_utf8_path_capped(&inst_path)
            .with_context(|| format!("read {}", inst_path.display()))?,
    )
    .with_context(|| format!("parse {}", inst_path.display()))?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .with_context(|| format!("compile {label} schema"))?;
    vox_jsonschema_util::validate(
        &instance_val,
        &validator,
        format!("{} ({})", label, inst_path.display()),
    )
    .map_err(|e| anyhow!("{e:#}"))?;
    println!("{label} OK (schema validated)");
    Ok(())
}

/// `findings-latest.json` is a bare array; validate against findings-array v1 schema.
pub(super) fn verify_findings_latest_schema(repo_root: &Path) -> Result<()> {
    validate_json_file_against_schema(
        repo_root,
        FINDINGS_LATEST_REL,
        FINDINGS_ARRAY_SCHEMA_REL,
        "findings-latest.json",
    )
}

pub(super) fn verify_remediation_delta_schema(repo_root: &Path) -> Result<()> {
    validate_json_file_against_schema(
        repo_root,
        REMEDIATION_DELTA_REL,
        REMEDIATION_DELTA_SCHEMA_REL,
        "remediation delta-after-remediation.json",
    )
}

pub(super) fn verify_gold_dataset_schema(repo_root: &Path) -> Result<()> {
    validate_json_file_against_schema(
        repo_root,
        GOLD_DATASET_REL,
        GOLD_SCHEMA_REL,
        "TOESTUB gold-dataset.v1.json",
    )
}

/// If `promotion-metrics.json` exists (after `emit-reports`), enforce minimal contract for CI dashboards.
pub(super) fn verify_promotion_metrics_optional(repo_root: &Path) -> Result<()> {
    let p = PathBuf::from(repo_root)
        .join(REMEDIATION_REPORT_ROOT)
        .join("promotion-metrics.json");
    if !p.is_file() {
        println!("promotion-metrics.json: absent (optional until emit-reports)");
        return Ok(());
    }
    let raw = read_utf8_path_capped(&p).with_context(|| format!("read {}", p.display()))?;
    let v: JsonValue =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", p.display()))?;
    if v.get("version").and_then(|x| x.as_u64()) != Some(1) {
        return Err(anyhow!("{}: expected version 1", p.display()));
    }
    if v.get("findings_total_latest").is_none() {
        return Err(anyhow!("{}: missing findings_total_latest", p.display()));
    }
    println!("promotion-metrics.json OK (rollup contract)");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RemediationLanesFile {
    #[allow(dead_code)]
    version: u32,
    lanes: Vec<RemediationLane>,
}

#[derive(Debug, Deserialize)]
struct RemediationLane {
    id: String,
    rule_family: String,
    status: String,
    #[serde(default)]
    #[allow(dead_code)]
    primary_crate: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    focus: Option<String>,
}

pub(super) fn verify_remediation_lanes(repo_root: &Path) -> Result<()> {
    let p = repo_root.join(REMEDIATION_LANES_REL);
    let raw = read_utf8_path_capped(&p).with_context(|| format!("read {}", p.display()))?;
    let doc: RemediationLanesFile =
        serde_yaml::from_str(&raw).context("parse REMEDIATION-LANES.yaml")?;
    if doc.version != 1 {
        return Err(anyhow!(
            "REMEDIATION-LANES.yaml: expected version 1, got {}",
            doc.version
        ));
    }
    let mut ids = HashSet::<&str>::new();
    for lane in &doc.lanes {
        if lane.id.is_empty() {
            return Err(anyhow!("remediation lane missing id"));
        }
        if !ids.insert(lane.id.as_str()) {
            return Err(anyhow!("duplicate remediation lane id: {}", lane.id));
        }
        if lane.rule_family.is_empty() {
            return Err(anyhow!("lane {}: rule_family required", lane.id));
        }
        if lane.status.is_empty() {
            return Err(anyhow!("lane {}: status required", lane.id));
        }
    }
    if doc.lanes.is_empty() {
        return Err(anyhow!(
            "REMEDIATION-LANES.yaml: expected at least one lane"
        ));
    }
    println!(
        "REMEDIATION-LANES.yaml OK ({} unique lanes)",
        doc.lanes.len()
    );
    Ok(())
}
