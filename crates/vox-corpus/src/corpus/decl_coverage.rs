//! Decl-kind histogram over `examples/golden/**/*.vox` for Mens / CI visibility (`ast-extract`).

use std::path::Path;

#[cfg(feature = "ast-extract")]
use anyhow::Context as _;
#[cfg(feature = "ast-extract")]
use serde::Deserialize;

/// Walk `examples/golden` under `repo_root`, parse each `.vox`, and count AST decl kinds
/// (same strings as the private `crate::corpus::extract_vox::part_ast` helper).
#[cfg(feature = "ast-extract")]
pub fn golden_decl_histogram(repo_root: &Path) -> anyhow::Result<serde_json::Value> {
    use crate::corpus::extract_vox::part_ast;
    use std::collections::BTreeMap;

    let golden = repo_root.join("examples/golden");
    if !golden.is_dir() {
        anyhow::bail!("missing golden dir {}", golden.display());
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files_ok = 0usize;
    let mut files_parse_fail = 0usize;
    walk_golden(&golden, &mut |src: &str| {
        if let Some(blocks) = part_ast::extract_decl_blocks_ast(src) {
            files_ok += 1;
            for (kind, _, _) in blocks {
                *counts.entry(kind).or_insert(0) += 1;
            }
        } else if !src.trim().is_empty() {
            files_parse_fail += 1;
        }
    })?;
    Ok(serde_json::json!({
        "golden_dir": golden.to_string_lossy(),
        "files_parse_ok": files_ok,
        "files_parse_fail": files_parse_fail,
        "decl_kind_counts": counts,
    }))
}

#[cfg(feature = "ast-extract")]
fn walk_golden(dir: &Path, f: &mut dyn FnMut(&str)) -> anyhow::Result<()> {
    for ent in std::fs::read_dir(dir)? {
        let ent = ent?;
        let p = ent.path();
        if p.is_dir() {
            walk_golden(&p, f)?;
        } else if p.extension().and_then(|e| e.to_str()) == Some("vox") {
            let src = std::fs::read_to_string(&p)?;
            f(&src);
        }
    }
    Ok(())
}

#[cfg(not(feature = "ast-extract"))]
pub fn golden_decl_histogram(_repo_root: &Path) -> anyhow::Result<serde_json::Value> {
    anyhow::bail!("decl_coverage requires vox-corpus `ast-extract` feature")
}

/// YAML contract `contracts/mens/golden_decl_expectations.yaml`: each kind must have count ≥ 1
/// in [`golden_decl_histogram`]'s `decl_kind_counts`.
#[cfg(feature = "ast-extract")]
#[derive(Debug, Deserialize)]
pub struct GoldenDeclExpectations {
    /// Declaration kind strings (same vocabulary as `part_ast::decl_kind_and_name`).
    pub required_decl_kinds: Vec<String>,
}

#[cfg(feature = "ast-extract")]
pub fn assert_golden_decl_expectations(
    repo_root: &Path,
    report: &serde_json::Value,
) -> anyhow::Result<()> {
    let path = repo_root.join("contracts/mens/golden_decl_expectations.yaml");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let exp: GoldenDeclExpectations =
        serde_yaml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    let counts = report
        .get("decl_kind_counts")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("report missing decl_kind_counts object"))?;
    let mut missing = Vec::new();
    for kind in &exp.required_decl_kinds {
        let n = counts.get(kind).and_then(|v| v.as_u64()).unwrap_or(0);
        if n < 1 {
            missing.push(kind.as_str());
        }
    }
    if !missing.is_empty() {
        anyhow::bail!(
            "corpus decl coverage: required decl kinds missing or zero in goldens: {:?}\n\
             Update examples/golden, parser/extractor labels, or contracts/mens/golden_decl_expectations.yaml",
            missing
        );
    }
    Ok(())
}
