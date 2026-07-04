//! `vox ci contracts-index` — validate [`contracts/index.yaml`](../../../../../contracts/index.yaml) against JSON Schema, on-disk paths, and JSON Schema instances for indexed YAML artifacts that declare a companion schema in the index.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

const INDEX_REL: &str = "contracts/index.yaml";
const SCHEMA_REL: &str = "contracts/index.schema.json";

/// When a YAML contract's schema index id is not `{yaml_id}-schema`, list the canonical schema id here.
const YAML_CONTRACT_SCHEMA_ID_OVERRIDES: &[(&str, &str)] = &[
    (
        "scientia-publication-worthiness-default",
        "scientia-publication-worthiness-schema",
    ),
    (
        "scientia-distribution-default",
        "scientia-distribution-schema",
    ),
    // YAML SSOT defines mutation_kind vocabulary; sibling .schema.json is the MCP response envelope.
    (
        "agent-computer-interface-v1",
        "agent-computer-interface-v1-ssot-schema",
    ),
];

#[derive(Debug, Deserialize)]
struct IndexFile {
    #[allow(dead_code)]
    schema_version: u32,
    contracts: Vec<IndexContract>,
}

#[derive(Debug, Deserialize)]
struct IndexContract {
    id: String,
    path: String,
    kind: String,
}

/// Validate contract index: JSON Schema + every listed `path` exists.
pub fn run(repo_root: &Path) -> Result<()> {
    let index_path = repo_root.join(INDEX_REL);
    let raw = read_utf8_path_capped(&index_path)
        .with_context(|| format!("read {}", index_path.display()))?;

    let schema_path = repo_root.join(SCHEMA_REL);
    if !schema_path.is_file() {
        return Err(anyhow!("missing {}", schema_path.display()));
    }
    let schema_val: JsonValue = serde_json::from_str(&read_utf8_path_capped(&schema_path)?)
        .with_context(|| format!("parse {}", schema_path.display()))?;
    let instance: JsonValue =
        serde_yaml::from_str(&raw).context("parse contracts/index.yaml as JSON value")?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .context("compile contracts/index.schema.json")?;
    vox_jsonschema_util::validate(
        &instance,
        &validator,
        format!("contracts/index.yaml vs {}", schema_path.display()),
    )
    .map_err(|e| anyhow!("{e:#}"))?;

    let idx: IndexFile = serde_yaml::from_str(&raw).context("parse contracts/index.yaml")?;
    let id_to_path: HashMap<&str, &str> = idx
        .contracts
        .iter()
        .map(|c| (c.id.as_str(), c.path.as_str()))
        .collect();
    let json_schema_ids: HashSet<&str> = idx
        .contracts
        .iter()
        .filter(|c| c.kind == "json-schema")
        .map(|c| c.id.as_str())
        .collect();

    for c in &idx.contracts {
        let p = repo_root.join(&c.path);
        if !p.is_file() {
            return Err(anyhow!(
                "contracts index: missing file for `{}`: {}",
                c.id,
                p.display()
            ));
        }
    }

    validate_indexed_yaml_against_schemas(
        repo_root,
        &idx.contracts,
        &id_to_path,
        &json_schema_ids,
    )?;

    println!(
        "contracts-index OK (schema v{}, {} entries)",
        idx.schema_version,
        idx.contracts.len()
    );
    Ok(())
}

fn validate_indexed_yaml_against_schemas(
    repo_root: &Path,
    contracts: &[IndexContract],
    id_to_path: &HashMap<&str, &str>,
    json_schema_ids: &HashSet<&str>,
) -> Result<()> {
    let resolve_schema_id = |yaml_id: &str| -> Option<String> {
        if let Some((_, sid)) = YAML_CONTRACT_SCHEMA_ID_OVERRIDES
            .iter()
            .find(|(y, _)| *y == yaml_id)
        {
            return Some((*sid).to_string());
        }
        let candidate = format!("{yaml_id}-schema");
        if json_schema_ids.contains(candidate.as_str()) {
            Some(candidate)
        } else {
            None
        }
    };

    for c in contracts {
        if c.kind != "yaml" {
            continue;
        }
        let Some(schema_id) = resolve_schema_id(&c.id) else {
            continue;
        };
        let Some(schema_rel) = id_to_path.get(schema_id.as_str()).copied() else {
            return Err(anyhow!(
                "contracts index: YAML `{}` references schema id `{}` but that id is not listed",
                c.id,
                schema_id
            ));
        };
        let yaml_path = repo_root.join(&c.path);
        let json_schema_path = repo_root.join(schema_rel);
        let yaml_raw = read_utf8_path_capped(&yaml_path)
            .with_context(|| format!("read {}", yaml_path.display()))?;
        let schema_raw = read_utf8_path_capped(&json_schema_path)
            .with_context(|| format!("read {}", json_schema_path.display()))?;
        let schema_val: JsonValue = serde_json::from_str(&schema_raw).with_context(|| {
            format!(
                "parse {} as JSON Schema",
                json_schema_path
                    .strip_prefix(repo_root)
                    .unwrap_or(&json_schema_path)
                    .display()
            )
        })?;
        let instance: JsonValue = serde_yaml::from_str(&yaml_raw)
            .with_context(|| format!("parse {}", yaml_path.display()))?;
        let validator =
            vox_jsonschema_util::compile_validator(&schema_val, json_schema_path.display())
                .with_context(|| format!("compile schema for YAML contract `{}`", c.id))?;
        vox_jsonschema_util::validate(
            &instance,
            &validator,
            format!("{} vs {}", c.path, schema_rel),
        )
        .map_err(|e| anyhow!("{e:#}"))?;
    }
    Ok(())
}
