//! Structured suppressions loaded from JSON matching `contracts/toestub/suppression.v1.schema.json`.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use serde_json::Value;

use crate::rules::Finding;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SuppressionFile {
    #[allow(dead_code)]
    version: u32,
    suppressions: Vec<SuppressionEntry>,
}

#[derive(Debug, Deserialize)]
struct SuppressionEntry {
    rule_id_prefix: String,
    #[serde(default)]
    path_glob: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    #[allow(dead_code)]
    reason: String,
    #[allow(dead_code)]
    owner: String,
    #[serde(default)]
    #[allow(dead_code)]
    expires_after: Option<String>,
}

struct Compiled {
    rule_prefix: String,
    path_matcher: GlobMatcher,
    line: Option<usize>,
}

pub struct SuppressionStore {
    entries: Vec<Compiled>,
}

fn find_repo_root_holding_suppression_schema(mut base: &Path) -> Option<PathBuf> {
    if base.is_file() {
        base = base.parent()?;
    }
    for anc in base.ancestors() {
        if anc
            .join("contracts/toestub/suppression.v1.schema.json")
            .is_file()
        {
            return Some(anc.to_path_buf());
        }
    }
    None
}

fn load_suppression_schema_value(suppression_path: &Path) -> anyhow::Result<(Value, PathBuf)> {
    let root = find_repo_root_holding_suppression_schema(suppression_path).ok_or_else(|| {
        anyhow::anyhow!(
            "cannot locate contracts/toestub/suppression.v1.schema.json by walking up from {}",
            suppression_path.display()
        )
    })?;
    let schema_path = root.join("contracts/toestub/suppression.v1.schema.json");
    let schema_raw = vox_bounded_fs::read_utf8_path_capped(&schema_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", schema_path.display()))?;
    let schema_val: Value = serde_json::from_str(&schema_raw)
        .map_err(|e| anyhow::anyhow!("parse suppression schema: {e}"))?;
    Ok((schema_val, schema_path))
}

fn validate_suppression_json_instance(raw: &str, suppression_path: &Path) -> anyhow::Result<()> {
    let (schema_val, schema_path) = load_suppression_schema_value(suppression_path)?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .map_err(|e| anyhow::anyhow!("{e:#}"))?;
    let instance: Value =
        serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("parse suppressions JSON: {e}"))?;
    vox_jsonschema_util::validate(
        &instance,
        &validator,
        format!("suppressions vs {}", schema_path.display()),
    )
    .map_err(|e| anyhow::anyhow!("{e:#}"))?;
    Ok(())
}

/// Fail-closed validation for `contracts/toestub/suppression.v1.schema.json` (JSON parse),
/// both suppression ledger files against the schema, and full [`SuppressionStore`] load for the real ledger.
///
/// `repo_root` must be the workspace root (directory containing `contracts/`).
pub fn validate_toestub_suppression_contracts(repo_root: &Path) -> anyhow::Result<()> {
    let schema_path = repo_root.join("contracts/toestub/suppression.v1.schema.json");
    let _schema: Value = serde_json::from_str(
        &vox_bounded_fs::read_utf8_path_capped(&schema_path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", schema_path.display()))?,
    )
    .map_err(|e| anyhow::anyhow!("contracts/toestub/suppression.v1.schema.json: {e}"))?;

    let ledger = repo_root.join("contracts/toestub/suppressions.v1.json");
    let example = repo_root.join("contracts/toestub/suppressions.v1.example.json");

    for path in [&ledger, &example] {
        let raw = vox_bounded_fs::read_utf8_path_capped(path)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
        validate_suppression_json_instance(&raw, path)?;
    }

    SuppressionStore::load_optional(Some(&ledger))?;
    Ok(())
}

impl SuppressionStore {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Parse JSON from disk; returns empty store if path missing.
    pub fn load_optional(path: Option<&Path>) -> anyhow::Result<Self> {
        let Some(path) = path else {
            return Ok(Self::empty());
        };
        if !path.is_file() {
            return Ok(Self::empty());
        }
        let raw = vox_bounded_fs::read_utf8_path_capped(path)
            .map_err(|e| anyhow::anyhow!("read suppressions {}: {e}", path.display()))?;
        validate_suppression_json_instance(&raw, path)?;
        let doc: SuppressionFile = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parse suppressions JSON: {e}"))?;
        if doc.version != 1 {
            anyhow::bail!("suppressions: expected version 1");
        }
        let mut entries = Vec::with_capacity(doc.suppressions.len());
        for s in doc.suppressions {
            let Some(glob_str) = s
                .path_glob
                .as_ref()
                .map(|g| g.trim())
                .filter(|g| !g.is_empty())
            else {
                anyhow::bail!(
                    "suppression for `{}` must set non-empty `path_glob` (no blanket rule-wide suppressions)",
                    s.rule_id_prefix
                );
            };
            let g = Glob::new(glob_str)
                .map_err(|e| anyhow::anyhow!("bad path_glob {glob_str}: {e}"))?;
            let path_matcher = g.compile_matcher();
            entries.push(Compiled {
                rule_prefix: s.rule_id_prefix,
                path_matcher,
                line: s.line.map(|n| n as usize),
            });
        }
        Ok(Self { entries })
    }

    pub fn suppresses(&self, finding: &Finding) -> bool {
        let rel = finding.file.to_string_lossy().replace('\\', "/");
        for e in &self.entries {
            if !finding.rule_id.starts_with(&e.rule_prefix) {
                continue;
            }
            if !e.path_matcher.is_match(Path::new(rel.as_str())) {
                continue;
            }
            if let Some(ln) = e.line
                && finding.line != ln
            {
                continue;
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toestub_suppression_contracts_validate_from_workspace_root() {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = loop {
            if dir.join("contracts/toestub/suppressions.v1.json").is_file() {
                break dir;
            }
            assert!(
                dir.pop(),
                "could not find contracts/toestub/suppressions.v1.json above {}",
                env!("CARGO_MANIFEST_DIR")
            );
        };
        validate_toestub_suppression_contracts(&repo_root).unwrap();
    }
}
