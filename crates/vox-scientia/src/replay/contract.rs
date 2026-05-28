//! `MainEntity` extraction helpers.
//!
//! The replay runner consumes [`crate::ro_crate::MainEntity`]; this
//! module wraps the JSON-loading path so callers don't have to know the
//! RO-Crate `@graph` shape.

use serde_json::Value;
use crate::ro_crate::MainEntity;

/// Parse a `MainEntity` from raw `ro-crate-metadata.json` bytes.
///
/// Looks for a graph node with `@id == "#mainEntity"` and reconstructs the
/// `MainEntity` from its `vox:*` predicates. Returns `Ok(None)` when the
/// JSON is valid but no mainEntity node exists (i.e., the artifact is not
/// replay-eligible).
pub fn parse_main_entity_from_json(bytes: &[u8]) -> Result<Option<MainEntity>, ParseError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| ParseError::Json(e.to_string()))?;
    let graph = root
        .get("@graph")
        .and_then(Value::as_array)
        .ok_or_else(|| ParseError::Schema("missing @graph array".into()))?;
    let node = match graph.iter().find(|n| n["@id"] == "#mainEntity") {
        Some(n) => n,
        None => return Ok(None),
    };
    let entry_point = node
        .get("vox:entryPoint")
        .and_then(Value::as_str)
        .ok_or_else(|| ParseError::Schema("mainEntity.vox:entryPoint missing".into()))?
        .to_string();
    let env_pin = node
        .get("vox:envPin")
        .and_then(Value::as_str)
        .ok_or_else(|| ParseError::Schema("mainEntity.vox:envPin missing".into()))?
        .to_string();
    let timeout_seconds = node
        .get("vox:timeoutSeconds")
        .and_then(Value::as_u64)
        .ok_or_else(|| ParseError::Schema("mainEntity.vox:timeoutSeconds missing".into()))?
        as u32;
    let budget = node
        .get("vox:resourceBudget")
        .ok_or_else(|| ParseError::Schema("mainEntity.vox:resourceBudget missing".into()))?;
    let max_stdout_bytes = budget
        .get("maxStdoutBytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| ParseError::Schema("resourceBudget.maxStdoutBytes missing".into()))?;
    let max_stderr_bytes = budget
        .get("maxStderrBytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| ParseError::Schema("resourceBudget.maxStderrBytes missing".into()))?;
    let outputs = node
        .get("vox:expectedOutputs")
        .and_then(Value::as_array)
        .ok_or_else(|| ParseError::Schema("mainEntity.vox:expectedOutputs missing".into()))?;
    let mut expected_output_paths = Vec::with_capacity(outputs.len());
    let mut expected_output_hashes_hex = Vec::with_capacity(outputs.len());
    for (idx, out) in outputs.iter().enumerate() {
        let path = out
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ParseError::Schema(format!("expectedOutputs[{idx}].path missing")))?
            .to_string();
        let hash = out
            .get("sha3_256_hex")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ParseError::Schema(format!("expectedOutputs[{idx}].sha3_256_hex missing"))
            })?
            .to_string();
        expected_output_paths.push(path);
        expected_output_hashes_hex.push(hash);
    }
    // Parse optional `vox:figures` array of figure provenance entries.
    let figures = node
        .get("vox:figures")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .enumerate()
                .map(|(idx, f)| {
                    let path = f
                        .get("path")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ParseError::Schema(format!("figures[{idx}].path missing"))
                        })?
                        .to_string();
                    let sha = f
                        .get("sha3_256_hex")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ParseError::Schema(format!("figures[{idx}].sha3_256_hex missing"))
                        })?
                        .to_string();
                    let script = f
                        .get("vox:sourceScript")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            ParseError::Schema(format!("figures[{idx}].vox:sourceScript missing"))
                        })?
                        .to_string();
                    let rendered_at_ms = f
                        .get("vox:renderedAtMs")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| {
                            ParseError::Schema(format!("figures[{idx}].vox:renderedAtMs missing"))
                        })?;
                    let caption_hint = f
                        .get("vox:captionHint")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    Ok::<_, ParseError>(crate::ro_crate::FigureProvenance {
                        path,
                        sha3_256_hex: sha,
                        source_script: script,
                        rendered_at_ms,
                        caption_hint,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(Some(MainEntity {
        entry_point,
        expected_output_paths,
        expected_output_hashes_hex,
        env_pin,
        timeout_seconds,
        max_stdout_bytes,
        max_stderr_bytes,
        figures,
    }))
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid ro-crate-metadata.json: {0}")]
    Json(String),
    #[error("ro-crate-metadata.json schema: {0}")]
    Schema(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ro_crate::{build_ro_crate_json, RoCrateMetadata};

    fn sample_with_main_entity() -> RoCrateMetadata {
        RoCrateMetadata {
            name: "demo".into(),
            description: "demo".into(),
            doi: None,
            author_orcid: None,
            author_ror: None,
            license_spdx: "MIT".into(),
            published_at: 0,
            keywords: vec![],
            main_entity: Some(MainEntity {
                entry_point: "echo ok > out.txt".into(),
                expected_output_paths: vec!["out.txt".into()],
                expected_output_hashes_hex: vec!["abc".into()],
                env_pin: "lockfile:0".into(),
                timeout_seconds: 30,
                max_stdout_bytes: 1024,
                max_stderr_bytes: 1024,
                figures: vec![],
            }),
        }
    }

    #[test]
    fn round_trips_through_ro_crate_json() {
        let meta = sample_with_main_entity();
        let json = build_ro_crate_json(&meta);
        let bytes = serde_json::to_vec(&json).unwrap();
        let parsed = parse_main_entity_from_json(&bytes).unwrap();
        let me = parsed.expect("mainEntity present");
        assert_eq!(me.entry_point, "echo ok > out.txt");
        assert_eq!(me.expected_output_paths, vec!["out.txt".to_string()]);
        assert_eq!(me.timeout_seconds, 30);
        assert_eq!(me.max_stdout_bytes, 1024);
    }

    #[test]
    fn absent_main_entity_returns_none() {
        let mut meta = sample_with_main_entity();
        meta.main_entity = None;
        let json = build_ro_crate_json(&meta);
        let bytes = serde_json::to_vec(&json).unwrap();
        let parsed = parse_main_entity_from_json(&bytes).unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn malformed_json_yields_json_error() {
        let res = parse_main_entity_from_json(b"{not json");
        assert!(matches!(res, Err(ParseError::Json(_))));
    }

    #[test]
    fn missing_graph_yields_schema_error() {
        let res = parse_main_entity_from_json(br#"{"@context": []}"#);
        assert!(matches!(res, Err(ParseError::Schema(_))));
    }
}
