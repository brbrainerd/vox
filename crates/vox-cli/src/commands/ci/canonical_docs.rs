//! Documentation authority-map verifier used by `vox ci check-docs-ssot`.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use vox_bounded_fs::read_utf8_path_capped;

const MAP_REL: &str = "contracts/documentation/canonical-map.v1.yaml";
const SCHEMA_REL: &str = "contracts/documentation/canonical-map.v1.schema.json";

#[derive(Debug, Deserialize)]
struct CanonicalMap {
    #[allow(dead_code)]
    schema_version: u32,
    domains: Vec<DomainEntry>,
}

#[derive(Debug, Deserialize)]
struct DomainEntry {
    id: String,
    title: String,
    /// When `B-canon`, `canon_doc` must not live under `docs/src/archive/`.
    #[serde(default)]
    tier: Option<String>,
    canon_doc: String,
    #[serde(default)]
    spec_paths: Vec<String>,
    #[serde(default)]
    generated_docs: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    owning_crate_globs: Vec<String>,
}

pub(crate) fn run(repo_root: &Path) -> Result<()> {
    let map_path = repo_root.join(MAP_REL);
    let map_raw =
        read_utf8_path_capped(&map_path).with_context(|| format!("read {}", map_path.display()))?;
    let schema_path = repo_root.join(SCHEMA_REL);
    let schema_raw = read_utf8_path_capped(&schema_path)
        .with_context(|| format!("read {}", schema_path.display()))?;

    let schema_val: JsonValue =
        serde_json::from_str(&schema_raw).context("parse canonical-map schema JSON")?;
    let instance: JsonValue =
        serde_yaml::from_str(&map_raw).context("parse canonical-map as JSON value")?;
    let validator = vox_jsonschema_util::compile_validator(&schema_val, schema_path.display())
        .context("compile canonical-map schema")?;
    vox_jsonschema_util::validate(&instance, &validator, format!("{MAP_REL} vs {SCHEMA_REL}"))
        .map_err(|e| anyhow!("{e:#}"))?;

    let map: CanonicalMap = serde_yaml::from_str(&map_raw).context("parse canonical-map YAML")?;
    verify_uniqueness(&map)?;
    verify_paths(repo_root, &map)?;
    verify_alias_rules(repo_root, &map)?;
    verify_owning_globs(repo_root, &map)?;

    println!("canonical docs map OK ({} domains)", map.domains.len());
    Ok(())
}

fn verify_uniqueness(map: &CanonicalMap) -> Result<()> {
    let mut ids = HashSet::new();
    let mut canon_docs = HashSet::new();
    for d in &map.domains {
        if !ids.insert(d.id.as_str()) {
            return Err(anyhow!("canonical-map duplicate domain id: {}", d.id));
        }
        if !canon_docs.insert(d.canon_doc.as_str()) {
            return Err(anyhow!(
                "canonical-map duplicate canon_doc path: {}",
                d.canon_doc
            ));
        }
    }
    Ok(())
}

fn verify_paths(repo_root: &Path, map: &CanonicalMap) -> Result<()> {
    for d in &map.domains {
        if d.title.trim().is_empty() {
            return Err(anyhow!("canonical-map domain {} has empty title", d.id));
        }
        if d.tier.as_deref() == Some("B-canon")
            && d.canon_doc
                .replace('\\', "/")
                .starts_with("docs/src/archive/")
        {
            return Err(anyhow!(
                "canonical-map domain {} B-canon canon_doc must not point into docs/src/archive/ (got {})",
                d.id,
                d.canon_doc
            ));
        }
        ensure_file(repo_root, &d.canon_doc, &d.id, "canon_doc")?;
        let canon_text = read_utf8_path_capped(&repo_root.join(&d.canon_doc))
            .with_context(|| format!("read canon_doc {}", d.canon_doc))?;
        if !canon_text.contains("\ncategory:") {
            return Err(anyhow!(
                "canonical-map domain {} canon_doc {} missing frontmatter category",
                d.id,
                d.canon_doc
            ));
        }
        for p in &d.spec_paths {
            ensure_file(repo_root, p, &d.id, "spec_paths")?;
        }
        for p in &d.generated_docs {
            ensure_file(repo_root, p, &d.id, "generated_docs")?;
        }
        for p in &d.aliases {
            ensure_file(repo_root, p, &d.id, "aliases")?;
        }
    }
    Ok(())
}

fn ensure_file(repo_root: &Path, rel: &str, domain: &str, field: &str) -> Result<()> {
    let p = repo_root.join(rel);
    if !p.is_file() {
        return Err(anyhow!(
            "canonical-map domain {domain} {field} missing file: {}",
            p.display()
        ));
    }
    Ok(())
}

fn verify_alias_rules(repo_root: &Path, map: &CanonicalMap) -> Result<()> {
    for d in &map.domains {
        let canon_file = Path::new(&d.canon_doc)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        for alias in &d.aliases {
            let text = read_utf8_path_capped(&repo_root.join(alias))
                .with_context(|| format!("read alias {}", alias))?;
            let links_to_canon = text.contains(&d.canon_doc)
                || (!canon_file.is_empty() && text.contains(canon_file));
            let legacy_status =
                text.contains("\nstatus: legacy") || text.contains("\nstatus: \"legacy\"");
            if !links_to_canon && !legacy_status {
                return Err(anyhow!(
                    "canonical-map alias {} for domain {} must either link to canon_doc {} or set status: legacy",
                    alias,
                    d.id,
                    d.canon_doc
                ));
            }
        }
    }
    Ok(())
}

fn verify_owning_globs(repo_root: &Path, map: &CanonicalMap) -> Result<()> {
    let files = collect_files(repo_root)?;
    for d in &map.domains {
        for g in &d.owning_crate_globs {
            let pat = glob::Pattern::new(g).with_context(|| {
                format!(
                    "canonical-map domain {} invalid owning_crate_globs pattern: {}",
                    d.id, g
                )
            })?;
            let mut any = false;
            for f in &files {
                if pat.matches_path(f) {
                    any = true;
                    break;
                }
            }
            if !any {
                return Err(anyhow!(
                    "canonical-map domain {} owning_glob matched no repo files: {}",
                    d.id,
                    g
                ));
            }
        }
    }
    Ok(())
}

fn collect_files(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![repo_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
            let entry = entry?;
            let p = entry.path();
            let t = entry.file_type()?;
            if t.is_dir() {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name == ".git" || name == "target" || name == "book" {
                    continue;
                }
                stack.push(p);
            } else if t.is_file() {
                let rel = p.strip_prefix(repo_root).unwrap_or(&p).to_path_buf();
                out.push(rel);
            }
        }
    }
    Ok(out)
}
